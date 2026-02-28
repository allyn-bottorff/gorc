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

use clap::Parser;
use std::env;
use std::process::Command;
use std::path::Path;

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
    #[arg(short, long, default_value_t = false)]
    quiet: bool,
    ///Verbose output
    #[arg(short, long, default_value_t = false)]
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
    Quiet, // Output nothing
    Normal, // Normal status and progress output
    Verbose, // Error and debug information in addition to normal output
}

/// Parsed configuration with CLI flags parsed into some ergonomic types
#[derive(Debug)]
struct Config {
    org: String,
    transport: Transport,
    verbosity: Verbosity,
    vcs: Vcs, 
    nofetch: bool,
    path: Path,
}
impl Config {
    fn new_from_cli(flags: CliFlags) -> Config {

    }
}

fn main() {
    let cli_flags = CliFlags::parse();

    println!("gorc!");

    let token = get_github_token(&cli_flags).expect("Unable to get GitHub token from the environment.");

    dbg!(cli_flags);
    dbg!(token);
}




fn get_org_repositories(cli_flags: &CliFlags, token: &str) {


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
