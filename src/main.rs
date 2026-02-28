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

/// GitHub Org Repository Clone (GORC)
///
/// A simple tool to clone and sync all of the repositories from a single GitHub organization
/// This tool will attempt to find an authentication token for GitHub from the following sources,
/// stopping after the first valid source found:
/// `gh auth token` -> `GITHUB_TOKEN` -> `GITHUB_PAT`
#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
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

fn main() {
    let _cli = Cli::parse();

    println!("gorc!")
}

fn get_github_token() -> Option<String> {
    let token_env_var: Option<String> = match env::var("GITHUB_TOKEN") {
        Ok(v) => Some(v),
        Err(_e) => match env::var("GITHUB_PAT") {
            Ok(v) => Some(v),
            Err(_e) => None,
        },
    };

    return token_env_var;
}
