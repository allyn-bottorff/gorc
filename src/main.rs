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

use std::env;
use std::process::Command;
use std::fs;
use clap::Parser;
use anyhow::Result;
use serde::Deserialize;
use ureq;
use futures;

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

#[derive(Debug, Deserialize)]
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


fn main() -> Result<()> {
    let cli_flags = CliFlags::parse();


    let token =
        get_github_token(&cli_flags).expect("Unable to get GitHub token from the environment.");

    let config = Config::new_from_flags(&cli_flags);

    let repos = match get_org_repositories(&config, &token).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to get org repositories: {}\n", e);
            panic!();
        }
    };


    // Create the requested path if it doesn't exist
    fs::create_dir_all(&config.path).unwrap();



    let mut tasks = vec![];
    for repo in &repos {
        let task = clone_one_repo(&config, &repo);
        tasks.push(task);
    }

    futures::future::join_all(tasks).await;

    dbg!(cli_flags);
    dbg!(config);
    dbg!(token);
    dbg!(repos);
    println!("gorc!");
    Ok(())
}

// Get all the repositories allowed by the token
async fn get_org_repositories(config: &Config, token: &str) -> Result<Vec<GHRepo>>{
    let url_base = format!("https://api.github.com/orgs/{}/repos",config.org);
    let repositories = ureq::get(url_base)
        .query("per_page","100")
        .header("User-Agent", "gorc")
        .header("Authorization", token)
        .call()?
        .body_mut()
        .read_json::<Vec<GHRepo>>()?;


    // TODO(alb): Handle request pagination

    // dbg!(&resp_text);
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

/// Clone a single repository using either Git or JJ depending on the configuration
async fn clone_one_repo(config: &Config, repo: &GHRepo) -> Result<tokio::process::Child, std::io::Error> {

    let url = match config.transport {
    Transport::HTTP => &repo.clone_url,
    Transport::SSH => &repo.ssh_url,
    };

    let path = fs::canonicalize(&config.path).unwrap();

    println!("Cloning:     {}", &repo.name);
    let result = match config.vcs {
        Vcs::Git => {
            tokio::process::Command::new("git")
                .current_dir(path)
                .arg("clone")
                .arg(&url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        },
        Vcs::JJ => {
            tokio::process::Command::new("jj")
                .current_dir(path)
                .arg("git")
                .arg("clone")
                .arg("--colocate")
                .arg(&url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        }
    };
    println!("Complete:    {}", &repo.name);

    return result

}
