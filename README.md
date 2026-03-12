# GitHub Org Repo Clone (GORC)

This is simple utility for cloning all of the repositories in a GitHub
organization. By default, it will clone any repositories that aren't already
cloned to the specified path and fetch updates for the ones that do exist.

Gorc attempts to pull GitHub credential information from the environment. First
from the `gh` utility if it exists, then from an environment variable named
`GITHUB_TOKEN` then from `GITHUB_PAT` in order of descending priority.

## Usage

Usage: gorc [OPTIONS] --path <PATH> --org <ORG>

Options:
  -p, --path <PATH>
          Path to the directory where all repositories will be cloned or fetched

  -o, --org <ORG>
          GitHub organization to clone

      --http
          Use HTTP instead of SSH

  -j, --jj
          Use Jujutsu as the VCS

  -q, --quiet
          No output

  -v, --verbose
          Verbose output

      --nofetch
          Do not fetch updates to remote repositories, only clone new ones

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

