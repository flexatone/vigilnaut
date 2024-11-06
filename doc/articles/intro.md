

<!--
# `fetter`: A Command-line tool for system-wide Python package allow-listing and vulnerability scanning
# Scan Your Entire System for Python Packages with Vulnerabilities
-->

# System-Wide Python Package Control: Search, Allow List, and Find Vulnerabilities,


A Python developer's system is likely littered with numerous virtual environments and hundreds of packages. Many of these virtual environments might be abandoned, holding out-of-date packages with known security vulnerabilities.

Even within a single virtual environment, installed packages can drift from the project's specified requirements. A developer might mistakenly install a package in the wrong virtual environment, or install a new package that inadvertently forces an upgrade to another package. When installed packages deviate from vetted requirements, unexpected behaviors can result, or worse, malware can be installed.

The `fetter` command-line application searches an entire system (or targeted virtual environments) for Python packages. Once found, those packages can be validated against a requirements or lock file, or searched for security vulnerabilities. Deployed as a `pre-commit` hook, these checks can be conducted on commit or push and integrated into continuous integration workflows. Going further, with `fetter` teams can enforce environment or system-wide package control. Just as cybersecurity tools such as Airlock Digital offer application allow listing, `fetter` can be used for Python package allow listing.

Beyond core validation operations, `fetter` permits searching for particular installed packages, deriving new requirements from observed packages across multiple environments, and unpacking and purging installed package content.

Similar to `ruff` and `uv`, `fetter` is implemented in efficient, multi-threaded Rust, taking maximum advantage of multi-core machines. Compared to implementing requirements validation with the `packaging` library, `fetter` can be twice as fast; compared to scanning for vulnerabilities in the OSV database with `pip-audit`, `fetter` can be ten times faster.

## Installing `fetter`

While available as pure Rust binary (crates), `fetter` is easily installed via a Python package wrapper:

```
$ pip install fetter
$ fetter --help
```

## Scanning Systems and Environments

By default, `fetter` will search for all packages in all `site_package` directories discoverable from all Python executables found in the system or user virtual environements.

```
$ fetter search
```

To limit searching `site_packages` associated with specific Python executables, the `--exe` (or `-e`) argument can be supplied with relative or absolute paths. For example, to only search site-packages of the currently active Python, `-e python3` can be provided.

```
$ fetter -e python3 search
```




## Usage With `pre-commit`


## Delimited File Output