# fetter

<a href="https://github.com/fetter-io/fetter-rs/actions/workflows/ci.yml">
    <img style="display: inline!important" src="https://img.shields.io/github/actions/workflow/status/fetter-io/fetter-rs/ci.yml?branch=default&label=CI&logo=Github"></img>
</a>
<a href="https://crates.io/crates/fetter">
    <img src="https://img.shields.io/crates/v/fetter?label=crates.io&logo=rust"></img>
</a>
<a href="https://pypi.org/project/fetter/">
    <img src="https://img.shields.io/pypi/v/fetter?label=PyPI&logo=pypi"></img>
</a>
<a href="https://crates.io/crates/fetter">
    <img src="https://img.shields.io/crates/d/fetter?label=Downloads&logo=rust"></img>
</a>

## System-wide Python package discovery, validation, and allow-listing.


The `fetter` command-line tool scans and validates Python packages across virtual environments or entire systems, ensuring packages conform to specified requirements or lock files. It identifies unapproved or vulnerable packages, supports continuous integration through 'pre-commit', and offers excellent performance thanks to a multi-threaded Rust implementation.


* 🔎 System Scanning: Finds Python packages across system environments.
* ⚖️ Package Validation: Checks installed packages against requirements.txt, pyproject.toml, or lock files sourced locally, via URLs, or via `git` repositories.
* 🛡️ Vulnerability Audit: Scans packages for security vulnerabilites in the Open Source Vulnerability database.
* ⚙️ CI Integration: Validate and audit with `pre-commit` [hooks](#Using-fetter-with-pre-commit).
* 🚀 Fast: Multi-threaded Rust implementation.
* 🪢 Bound Requirements: Derive lock-file-like bound requirements from observed system packages.
* 🧹 Search and Purge: Find and remove packages across environments.
* 🧩 Flexible Output: Display results in terminal or export to delimited files.


## Installing `fetter`

While available as a pure Rust binary ([crates](https://crates.io/crates/fetter)), `fetter` is easily installed via a Python package ([pypi](https://pypi.org/project/fetter)):

```shell
$ pip install fetter
$ fetter --help
```

Alternatively, as `fetter` can operate on multiple virtual environments, installation via [`pipx`](https://pipx.pypa.io) might be desirable:

```shell
$ pipx install fetter
$ fetter --version
```

## Using `fetter` from the command line

For complete command-line documentation, see [CLI Documentation](#Command-Line-Interface-Documentation).


## Using `fetter` with pre-commit

Two `fetter` commands can be run via [pre-commit](https://pre-commit.com/) hooks for continuous integration of Python package controls.


### Running `fetter validate` with `pre-commit`.


The `fetter validate` command permits validating that the actually installed Python packages in the current environment are what are defined to be installed, as specified by a requirements.txt file, a pyproject.toml file, or a lock file such as one produced by `uv`.

The `fetter validate` command takes a required argument, `--bound`, to specify that path or URL to the file to be used to define the bound requirements. The optional `--superset` argument permits packages not defined in the bound requirements to be present. The optional `--subset` argument permits not all packages in the bound requirements to be present.

To run `fetter validate` with `pre-commit`, add the following to your `.pre-commit-config.yaml`.


```yaml
repos:
- repo: https://github.com/fetter-io/fetter-rs
  rev: v0.13.1
  hooks:
    - id: fetter-validate
      args: [--bound, {FILE}, --superset, --subset]

```


### Running `fetter audit` with `pre-commit`.

The `fetter audit` command will check for cybersecurity vulnerabilities issued for all installed Python packages in the current environment. Vulnerabilities are searched for in the Open Source Vulnerability (OSV) database.

To run `fetter audit` with `pre-commit`, add the following to your `.pre-commit-config.yaml`. Note that, as searching vulnerabilities can take time, this hook is likely better deployed as a `pre-push` rather than a `pre-commit` hook.

```yaml
repos:
- repo: https://github.com/fetter-io/fetter-rs
  rev: v0.13.1
  hooks:
    - id: fetter-audit
```


## Command Line Interface Documentation


## What is New in Fetter

### 0.13.0

All subcommands now have their output sub-subcommands set to `display` by default.

The `validate` and `audit` subcommands now return a non-zero exit code when items are found.

The CLI now exits for unsupported platforms.


### 0.12.0

Extended `validate` and `audit` commands to return a non-zero error code if `display` prints records.


### 0.11.0

Implemented variable-width and colored terminal displays.

Implemented terminal spinner for long-running commands.

Added `purge-invalid` and `purge-pattern` commands.

Split `unpack` command into `unpack-count` and `unpack-files`.

Added support to specify `--bound` with a git repository.


### 0.10.0

Added `--user-site` flag to force inclusion of user site packages; otherwise, user site packages are only included if `ENABLE_USER_SITE` is set.

Reimplemented display and delimited table outputs to use a generic trait implementation.


### 0.9.0

Support `--requirement` in requirements files.


### 0.8.0

Package and DepSpec comparisons now remove user.

Package and DepSpec comparisons now accept matching either on requested_revision or commit_id.

URLs are now shown in DepSpec displays.

Delimited file output no longer pads spaces.


### 0.7.0

Validate display now shows paths properly.

Updated validate json output to terminate line and flush buffer.


### 0.6.0

Package and dependency keys are case insensitive.

Improved URL validation between dependency and package by removing user components.

Improved validation JSON output to provided labelled objects.

Improved valiation output to show sorted missing packages.

Renamed validation explain values.

Implemented support for nested requirements.txt.


### 0.5.0

Implemented search command with basic wildcard matching.

Implemented `Arc`-wrapped `PathBuf` for sharable site paths.

Added explanation column to validation results.

Added support for both `--subset` and `--superset` validations.

Implemented `ValidationDigest` for simplified JSON serialization.

Added `JSON` CLI output option for validation results.






