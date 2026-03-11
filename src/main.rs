// Copyright 2026 Allyn L. Bottorff
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio;
use ureq;
use futures;
use futures::StreamExt;


/// GitHub Org Repository Clone (GORC)
///
/// A simple tool to clone and sync all of the repositories from a single GitHub organization
/// This tool will attempt to find an authentication token for GitHub from the following sources,
/// in order of decreasing precedence:
/// `gh auth token` -> `GITHUB_TOKEN` -> `GITHUB_PAT`
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct CliFlags {
    ///Path to the directory where all repositories will be cloned or fetched.
    #[arg(short, long)]
    path: String,
    ///GitHub organization to clone.
    #[arg(short, long)]
    org: String,
    ///Use HTTP instead of SSH.
    #[arg(long)]
    http: bool,
    ///Use Jujutsu as the VCS.
    #[arg(short, long)]
    jj: bool,
    ///No output
    #[arg(short, long, default_value_t = false, conflicts_with = "verbose")]
    quiet: bool,
    ///Verbose output
    #[arg(short, long, default_value_t = false, conflicts_with = "quiet")]
    verbose: bool,
    ///Do not fetch updates to remote repositories, only clone new ones.
    #[arg(long, default_value_t = false)]
    nofetch: bool,
}

/// Determines whether to perform Git operations over HTTP or SSH
#[derive(Debug)]
enum Transport {
    HTTP,
    SSH,
}

/// Determines whether to use native Git repositories or git-backed Jujutsu repositories. In the JJ
/// case, the --colocate flag is used to ensure Git compatibility.
#[derive(Debug)]
enum Vcs {
    Git,
    JJ,
}

/// Set the amount of output to the console during normal operations
#[derive(Debug)]
enum Verbosity {
    Quiet,   // Output nothing
    Normal,  // Normal status and progress output
    Verbose, // Error and debug information in addition to normal output
}

#[derive(Debug, Deserialize, Clone)]
struct GHRepo {
    /// Name of the repository according to GitHub
    name: String,
    /// Git protocol url
    git_url: String,
    /// SSH clone url
    ssh_url: String,
    /// HTTP clone url
    clone_url: String,
}

/// Parsed configuration with CLI flags converted into some ergonomic types
#[derive(Debug)]
struct Config {
    org: String,
    transport: Transport,
    verbosity: Verbosity,
    vcs: Vcs,
    nofetch: bool,
    path: String,
}
impl Config {
    fn new_from_flags(flags: &CliFlags) -> Config {
        let transport = match flags.http {
            true => Transport::HTTP,
            false => Transport::SSH,
        };
        let verbosity = match flags.quiet {
            true => Verbosity::Quiet,
            false => match flags.verbose {
                true => Verbosity::Verbose,
                false => Verbosity::Normal,
            },
        };
        let vcs = match flags.jj {
            true => Vcs::JJ,
            false => Vcs::Git,
        };

        Config {
            org: flags.org.trim().into(),
            transport,
            verbosity,
            vcs,
            nofetch: flags.nofetch,
            path: flags.path.clone().trim().into(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {

    let cli_flags = CliFlags::parse();

    let token =
        get_github_token(&cli_flags).expect("Unable to get GitHub token from the environment.");

    // Create the static CONFIG struct that can be freely referenced everywhere
    // Abort if this doesn't succeed.
    let config = Config::new_from_flags(&cli_flags);

    // Create a reference for the config for this scope that's a little more ergonomic. If this
    // can't be accessed, abort.

    let repos = match get_org_repositories(&config, &token) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to get org repositories: {}\n", e);
            panic!();
        }
    };

    // Create the requested path if it doesn't exist. Abort if this cannot be created.
    fs::create_dir_all(&config.path)?;

    // If the base path can't be canonicalized after we've guaranteed its creation, then something
    // is very wrong and we should bail out.
    let base_path = fs::canonicalize(&config.path)?;

    // let mut join_set = tokio::task::JoinSet::new();

    match config.nofetch {
        true => {
            futures::stream::iter(repos)
                .filter(|repo| no_existing_repo(&base_path, repo.name.clone()))
                .map(|repo| clone_one_repo(&config, repo))
                .buffer_unordered(6)
                .for_each(|result| async {
                    match result {
                        Ok(_) => {},
                        Err(e) => {println!("{}", e);}
                    }
                }).await;
        },
        false => {
            futures::stream::iter(repos)
                .map(|repo| clone_or_fetch_wrapper(&config, &base_path, repo))
                .buffer_unordered(6)
                .for_each(|result| async {
                    match result {
                        Ok(_) => {},
                        Err(e) => {println!("{}", e);}
                    }
                }).await;
        }
    }
    // futures::stream::iter(repos)
    //     .map(|repo| clone_or_fetch_wrapper(&config, &base_path, repo))
    //     .buffer_unordered(6)
    //     .for_each(|result| async {
    //     match result {
    //         Ok(_) => {},
    //         Err(e) => {println!("{}", e);}
    //     }
    // }).await;




    // for repo in repos {
    //     match fs::exists(base_path.join(&repo.name)) {
    //         Ok(exists) => match exists {
    //             true => {
    //                 if !config.nofetch {
    //                     join_set.spawn(fetch_one_repo(repo.clone()));
    //                 }
    //             }
    //             false => {
    //                 join_set.spawn(clone_one_repo(repo.clone()));
    //             }
    //         },
    //         Err(e) => {
    //             eprintln!("Unable to handle path ending at '{}': {}", &repo.name, e);
    //         }
    //     }
    // }
    //
    // join_set.join_all().await;
    //
    

    Ok(())
}


// Helper function to determine if a repo already exists by name
async fn no_existing_repo(base_path: &PathBuf, name: String) -> bool {

    match fs::exists(base_path.join(name)).ok() {
        Some(exists) => match exists {
            true => false,
            false => true,
        },
        None => true,
    }
}

fn get_org_repositories(config: &Config, token: &str) -> Result<Vec<GHRepo>> {
    let url_base = format!(
        "https://api.github.com/orgs/{}/repos",
        config.org);

    let repositories = ureq::get(url_base)
        .query("per_page","100")
        .header("User-Agent", "gorc")
        .header("Authorization", token)
        .call()?
        .body_mut()
        .read_json::<Vec<GHRepo>>()?;

    // TODO(alb): Handle request pagination

    Ok(repositories)
}

/// Get GitHub token from the environment. Early return on successfully finding a token
fn get_github_token(cli_flags: &CliFlags) -> Option<String> {
    // Attempt to get the GitHub token from the gh cli tool
    let output = Command::new("gh").args(["auth", "token"]).output();
    match output {
        Ok(token) => {
            match String::from_utf8(token.stdout) {
                Ok(token) => {
                    //TODO(alb): Validate the token
                    if token != "" {
                        let token: String = token.trim().into();
                        return Some(token);
                    }
                }
                Err(e) => {
                    if cli_flags.verbose {
                        eprintln!("Error parsing gh auth token output: {e}");
                    }
                }
            }
        }
        Err(e) => {
            if cli_flags.verbose {
                eprintln!("Error executing gh auth token: {e}");
            }
        }
    }

    // Attempt to get the token from the GITHUB_TOKEN env var
    match env::var("GITHUB_TOKEN") {
        Ok(token) => {
            //TODO(alb): Improve the token validation
            let token: String = token.trim().into();
            if token != "" {
                return Some(token);
            }
        }
        Err(e) => {
            if cli_flags.verbose {
                eprintln!("Error reading GITHUB_TOKEN env var: {e}")
            }
        }
    }

    // Attempt to get the token from the GITHUB_PAT env var
    match env::var("GITHUB_PAT") {
        Ok(token) => {
            //TODO(alb): Improve the token validation
            let token: String = token.trim().into();
            if token != "" {
                return Some(token);
            }
        }
        Err(e) => {
            if cli_flags.verbose {
                eprintln!("Error reading GITHUB_PAT env var: {e}")
            }
        }
    }
    None
}

async fn clone_or_fetch_wrapper(config: &Config, base_path: &PathBuf, repo: GHRepo) -> Result<std::process::ExitStatus, std::io::Error> {

    match fs::exists(base_path.join(&repo.name))? {
        true => fetch_one_repo(config, repo).await,
        false => clone_one_repo(config, repo).await,
    }
}

/// Clone a single repository using either Git or JJ depending on the configuration
async fn clone_one_repo(config: &Config, repo: GHRepo) -> Result<std::process::ExitStatus, std::io::Error> {
    let url = match config.transport {
        Transport::HTTP => &repo.clone_url,
        Transport::SSH => &repo.ssh_url,
    };

    let path = fs::canonicalize(&config.path).unwrap();

    match config.verbosity {
        Verbosity::Quiet => {}
        Verbosity::Normal => {
            println!("Cloning:     {}", &repo.name);
        }
        Verbosity::Verbose => {
            println!("Cloning:     {}", &repo.name);
        }
    }
    let result = match config.vcs {
        Vcs::Git => {
            tokio::process::Command::new("git")
                .current_dir(path)
                .arg("clone")
                .arg(&url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?
                .wait()
                .await
        }
        Vcs::JJ => {
            tokio::process::Command::new("jj")
                .current_dir(path)
                .arg("git")
                .arg("clone")
                .arg("--colocate")
                .arg(&url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?
                .wait()
                .await
        }
    };
    match config.verbosity {
        Verbosity::Quiet => {}
        Verbosity::Normal => {
            println!("Complete:    {}", &repo.name);
        }
        Verbosity::Verbose => {
            println!("Complete:    {}", &repo.name);
        }
    }

    return result;
}

/// Fetch a single repo using either Git or JJ depending on the configuration
async fn fetch_one_repo(config: &Config, repo: GHRepo) -> Result<std::process::ExitStatus, std::io::Error> {
    let url = match config.transport {
        Transport::HTTP => &repo.clone_url,
        Transport::SSH => &repo.ssh_url,
    };

    let path = fs::canonicalize(&config.path).unwrap();

    match config.verbosity {
        Verbosity::Quiet => {}
        Verbosity::Normal => {
            println!("Fetching:    {}", &repo.name);
        }
        Verbosity::Verbose => {
            println!("Fetching:    {}", &repo.name);
        }
    }

    let result = match config.vcs {
        Vcs::Git => {
            tokio::process::Command::new("git")
                .current_dir(path)
                .arg("fetch")
                .arg(&url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?
                .wait()
                .await
        }
        Vcs::JJ => {
            tokio::process::Command::new("jj")
                .current_dir(path)
                .arg("git")
                .arg("fetch")
                .arg(&url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?
                .wait()
                .await
        }
    };
    match config.verbosity {
        Verbosity::Quiet => {}
        Verbosity::Normal => {
            println!("Complete:    {}", &repo.name);
        }
        Verbosity::Verbose => {
            println!("Complete:    {}", &repo.name);
        }
    }

    return result;
}
