

<!--
# `fetter`: A Command-line tool for system-wide Python package allow-listing and vulnerability scanning
# Scan Your Entire System for Python Packages with Vulnerabilities
-->

# System-Wide Python Package Control: Enforce Allow Lists, Find Vulnerabilities, and More


A Python developer's system is likely littered with numerous virtual environments and hundreds of packages. Many of these virtual environments might be abandoned, holding out-of-date packages with known security vulnerabilities.

Even within a single virtual environment, installed packages can drift from the project's specified requirements. A developer might mistakenly install a package in the wrong virtual environment, or install a new package that inadvertently forces an upgrade to another package. When installed packages deviate from vetted requirements, unexpected behaviors can result, or worse, malware can be installed.

The `fetter` command-line application searches an entire system (or targeted virtual environments) for Python packages. Once found, those packages can be validated against a requirements or lock file, or searched for security vulnerabilities. Deployed as a `pre-commit` hook, these checks can be conducted on commit or push and integrated into continuous integration workflows. Going further, with `fetter` teams can enforce environment or system-wide package control. Just as cybersecurity tools such as Airlock Digital offer application allow listing, `fetter` can be used for Python package allow listing.

Beyond core validation operations, `fetter` permits searching for particular installed packages, deriving new requirements from observed packages across multiple environments, and unpacking and purging installed package content.

Similar to `ruff` and `uv`, `fetter` is implemented in efficient, multi-threaded Rust, taking maximum advantage of multi-core machines. Compared to implementing requirements validation with the `packaging` library, `fetter` can be twice as fast; compared to scanning for vulnerabilities in the OSV database with `pip-audit`, `fetter` can be ten times faster.

## Installing `fetter`

While available as a pure Rust binary ([crates](https://crates.io/crates/fetter)), `fetter` is easily installed via a Python package ([pypi](https://pypi.org/project/fetter)):

```shell
$ pip install fetter
$ fetter --help
```

Alternatively, as `fetter` can operate on multiple virtual environments, installation via [`pipx`](https://pipx.pypa.io) might be appropriate:

```shell
$ pipx install fetter
$ fetter --version
```

## Scanning Systems and Environments

By default, `fetter` will search for all packages in all `site-package` directories discoverable from all Python executables found in the system or user virtual environments. Depending on your system, this command might take several seconds.

```shell
$ fetter scan
```

The `fetter scan` command finds all installed packages. Observed across an entire system, the results can be surprising. For example, I happen to have eight different versions of the `zipp` package scattered among seventeen virtual environments. (For a concise display, virtual environment names are abbreviated.)

```shell
Package      Site
zipp-3.7.0   ~/.env-rt/lib/python3.8/site-packages
             ~/.env-ag/lib/python3.8/site-packages
             ~/.env-qf/lib/python3.8/site-packages
             ~/.env-qa/lib/python3.8/site-packages
zipp-3.8.0   ~/.env-aw/lib/python3.8/site-packages
zipp-3.15.0  ~/.env-gp/lib/python3.8/site-packages
             ~/.env-po/lib/python3.10/site-packages
             ~/.env-yp/lib/python3.8/site-packages
zipp-3.16.0  ~/.env-fb/lib/python3.11/site-packages
zipp-3.16.2  ~/.env-sf/lib/python3.11/site-packages
             ~/.env-te/lib/python3.8/site-packages
             ~/.env-hy/lib/python3.8/site-packages
zipp-3.17.0  ~/.env-sq/lib/python3.12/site-packages
zipp-3.18.1  ~/.env-tp/lib/python3.11/site-packages
             ~/.env-uv/lib/python3.11/site-packages
             ~/.env-wp/lib/python3.8/site-packages
zipp-3.20.2  ~/.env-tl/lib/python3.11/site-packages
```


To limit scanning to `site-packages` directories associated with a specific Python executable, the `--exe` (or `-e`) argument can be supplied with relative or absolute paths. To demonstrate this, we can first build a virtual environment from the following requirements.txt:

```
jinja2==3.1.3
zipp==3.18.1
requests==2.32.3
```

Then, after making that environment active, we can scan the `site-packages` directory of this active Python by providing the argument `-e python3`. As `fetter` reports on all installed packages, we see not only explicit requirements but all dependencies of those requirements, as well as `pip` itself:

```shell
$ fetter -e python3 scan
Package                   Site
certifi-2024.8.30         ~/.env-wp/lib/python3.8/site-packages
charset_normalizer-3.4.0  ~/.env-wp/lib/python3.8/site-packages
idna-3.10                 ~/.env-wp/lib/python3.8/site-packages
jinja2-3.1.3              ~/.env-wp/lib/python3.8/site-packages
markupsafe-2.1.5          ~/.env-wp/lib/python3.8/site-packages
pip-21.1.1                ~/.env-wp/lib/python3.8/site-packages
requests-2.32.3           ~/.env-wp/lib/python3.8/site-packages
setuptools-56.0.0         ~/.env-wp/lib/python3.8/site-packages
urllib3-2.2.3             ~/.env-wp/lib/python3.8/site-packages
zipp-3.18.1               ~/.env-wp/lib/python3.8/site-packages
```

## Validating Installed Packages

Once we can discover all installed packages, we can validate them against a list of expected packages. That list, or "bound requirements", can be a requirements.txt file, a pyproject.toml file, or a lock file created by `uv` or other tool.

For example, to validate that the installed packages match the packages specified in requirements.txt, we can use the `fetter validate` command, again targeting our active Python with `-e python3` and providing "requirements.txt" to the `--bound` argument.

```shell
$ fetter -e python3 validate --bound requirements.txt
Package                   Dependency  Explain     Sites
certifi-2024.8.30                     Unrequired  ~/.env-wp/lib/python3.8/site-packages
charset_normalizer-3.4.0              Unrequired  ~/.env-wp/lib/python3.8/site-packages
idna-3.10                             Unrequired  ~/.env-wp/lib/python3.8/site-packages
markupsafe-2.1.5                      Unrequired  ~/.env-wp/lib/python3.8/site-packages
pip-21.1.1                            Unrequired  ~/.env-wp/lib/python3.8/site-packages
setuptools-56.0.0                     Unrequired  ~/.env-wp/lib/python3.8/site-packages
urllib3-2.2.3                         Unrequired  ~/.env-wp/lib/python3.8/site-packages
```

As configured, validation fails with numerous "Unrequired" records because packages are installed that are not defined in the requirements.txt file. As this is a common scenario, the `--superset` command can be provided to accept packages that are not defined in the bound requirements.

```shell
$ fetter -e python3 validate --bound requirements.txt --superset
```

If we happen to update a package in a way that is not within the specification of the bound requirements, `fetter` will report these as "Misdefined" records. In the example below, we update `zipp` to version 3.20.2 and re-run validation:


```shell
$ fetter -e python3 validate --bound requirements.txt --superset
Package      Dependency    Explain     Sites
zipp-3.20.2  zipp==3.18.1  Misdefined  ~/.env-wp/lib/python3.8/site-packages
```

If we remove the the `zipp` package entirely, `fetter` identifies this as a "Missing" record.

```shell
$ fetter -e python3 validate --bound requirements.txt --superset
Package  Dependency    Explain  Sites
         zipp==3.18.1  Missing
```

If we want to peermit the absence of specified packages, the `--subset` flag can be used:

```
fetter -e python3 validate --bound requirements.txt --superset --subset
```

For maximal control, bound requirements can be a lock file, which is expected to fully specify all packages and their dependencies. To help derive a bound requirements file from a system or virtual environment, the `fetter derive` command can be used. Bound requirements can be stored on the local file system, fetched from a URL, or pulled from a `git` repository.

Validating installed packages can provdie an important check that a developer environment conforms to the projects expectations. Going further, a system-wide definition of bound requirements can implement Python Package allow listing. Deploying `fetter validate` as a `git` hook with `pre-commit` is an effective way to do this: all that is necessary is to specify the `fetter-rs` repo, the `fetter-validate` hook, and any additional configuration in you ".pre-commit-config.yaml" file.


```yaml
repos:
- repo: https://github.com/fetter-io/fetter-rs
  rev: v0.13.1
  hooks:
    - id: fetter-validate
      args: [--bound, requirements.txt, --superset]

```


## Searching for Package Vulnerabilities

In addition to validating that installed packages conform to specified versions, `fetter` can check if installed packages have known security vulnerabilities defined in the Open Source Vulnerability (OSV) database. When using the `fetter audit` command, details are provided for every vulnerability associated with the particular package and version.

```shell
$ fetter -e python3 audit
Package            Vulnerabilities      Attribute  Value
jinja2-3.1.3       GHSA-h75v-3vvj-5mfj  URL        https://osv.dev/vulnerability/GHSA-h75v-3vvj-5mfj
                                        Summary    Jinja vulnerable to HTML attribute injection when passing ...
                                        Reference  https://nvd.nist.gov/vuln/detail/CVE-2024-34064
                                        Severity   CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:L/I:L/A:N
pip-21.1.1         GHSA-mq26-g339-26xf  URL        https://osv.dev/vulnerability/GHSA-mq26-g339-26xf
                                        Summary    Command Injection in pip when used with Mercurial
                                        Reference  https://nvd.nist.gov/vuln/detail/CVE-2023-5752
                                        Severity   CVSS:4.0/AV:L/AC:L/AT:N/PR:L/UI:N/VC:N/VI:H/VA:N/SC:N/SI:N/SA
                   PYSEC-2023-228       URL        https://osv.dev/vulnerability/PYSEC-2023-228
                                        Reference  https://mail.python.org/archives/list/security-announce@py...
                                        Severity   CVSS:3.1/AV:L/AC:L/PR:L/UI:N/S:U/C:N/I:L/A:N
setuptools-56.0.0  GHSA-cx63-2mw6-8hw5  URL        https://osv.dev/vulnerability/GHSA-cx63-2mw6-8hw5
                                        Summary    setuptools vulnerable to Command Injection via package URL
                                        Reference  https://nvd.nist.gov/vuln/detail/CVE-2024-6345
                                        Severity   CVSS:4.0/AV:N/AC:L/AT:P/PR:N/UI:A/VC:H/VI:H/VA:H/SC:N/SI:N/SA
                   GHSA-r9hx-vwmv-q579  URL        https://osv.dev/vulnerability/GHSA-r9hx-vwmv-q579
                                        Summary    pypa/setuptools vulnerable to Regular Expression Denial of...
                                        Reference  https://nvd.nist.gov/vuln/detail/CVE-2022-40897
                                        Severity   CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H
                   PYSEC-2022-43012     URL        https://osv.dev/vulnerability/PYSEC-2022-43012
                                        Reference  https://github.com/pypa/setuptools/blob/fe8a98e696241487ba...
zipp-3.18.1        GHSA-jfmj-5v4g-7637  URL        https://osv.dev/vulnerability/GHSA-jfmj-5v4g-7637
                                        Summary    zipp Denial of Service vulnerability
                                        Reference  https://nvd.nist.gov/vuln/detail/CVE-2024-5569
                                        Severity   CVSS:4.0/AV:L/AC:L/AT:N/PR:N/UI:N/VC:N/VI:N/VA:H/SC:N/SI:N/SA
```

While tools such as `pip-audit` can provide similar audit's, `fetter` offers very significant performance advantages.

Just as with `fetter validate`, this operation can be configured to run as a `git` hook. While perhaps not necessary to run on every commit, running this check pre-push might be desirable. As before, all that is necessary is to add the hook in your ".pre-commit-config.yaml" file.

```yaml
repos:
- repo: https://github.com/fetter-io/fetter-rs
  rev: v0.13.1
  hooks:
    - id: fetter-audit
```

## Other Utilities

The `fetter` CLI exposes a number of additional utilities to explore system-wide Python package information. For example, to get metrics on discovered executiables, site package directories, and packages, aggregate counts are available with `fetter count`:

```shell
fetter count

             Count
Executables  67
Sites        45
Packages     1420
```

To discover all versions of NumPy installed on my system, I can use `fetter search` with a glob-style pattern.

```shell
$ fetter search -p numpy-*
Package       Site
numpy-1.18.5  ~/.env-ag/lib/python3.8/site-packages
numpy-1.19.5  ~/.env-qa/lib/python3.8/site-packages
numpy-1.22.0  ~/.env-qf/lib/python3.8/site-packages
numpy-1.22.2  ~/.env310/lib/python3.10/site-packages
numpy-1.22.4  ~/.env-te/lib/python3.8/site-packages
numpy-1.23.5  ~/.env-hy/lib/python3.8/site-packages
              ~/.env-tn/lib/python3.9/site-packages
              ~/.env-gp/lib/python3.8/site-packages
              ~/.env-yp/lib/python3.8/site-packages
              ~/.env-tl/lib/python3.11/site-packages
              ~/.env-np/lib/python3.10/site-packages
numpy-1.24.2  ~/.env-er/lib/python3.11/site-packages
              ~/.env-aw/lib/python3.8/site-packages
              ~/.env-am/lib/python3.8/site-packages
numpy-1.24.3  ~/.env-tl/lib/python3.11/site-packages
              ~/.env-ak/lib/python3.8/site-packages
              ~/.env-uv/lib/python3.11/site-packages
numpy-1.24.4  ~/.env-rt/lib/python3.8/site-packages
numpy-1.25.1  ~/.env-sf/lib/python3.11/site-packages
numpy-1.26.0  ~/.env-fb/lib/python3.11/site-packages
numpy-1.26.2  ~/.env-tt/lib/python3.12/site-packages
              ~/.env-rr/lib/python3.12/site-packages
numpy-1.26.4  ~/.env-sg/lib/python3.11/site-packages
              ~/.env-df/lib/python3.12/site-packages
numpy-2.0.0   ~/.env-tt/lib/python3.12/site-packages
              ~/.env-sq/lib/python3.12/site-packages
numpy-2.1.2   ~/.env-lt/lib/python3.11/site-packages
```

Observing 15 different versions in 27 virtual environment might encourage me to clean up some of these old packages. Using `fetter unpack-count`, we can view see how many files are associated with the installation.

```shell
fetter unpack-count -p numpy-1.18.5
Package       Site                                   Files  Dirs
numpy-1.18.5  ~/.env-ag/lib/python3.8/site-packages  855    2
```

Using `fetter purge-pattern`, we can remove all the files associated with that Package, equivalent to uninstalling that package, but possible to be executed across one or all virtual environments on a system:

```shell
fetter purge-pattern -p numpy-1.18.5
```


## Delimited File Output

All of the previous examples have used the default `fetter` behavior to print output to the terminal. Alternatively, all commands offer alternative output as delimited text files, suitable for reading in other applications or further processing. To write the output of the `fetter search` command to a pipe-delimited file we simply add additional arguments:

'''
$ fetter search -p numpy-* write -o /tmp/out.txt -d "|"
$ cat /tmp/out.txt
Package|Site
numpy-1.19.5|~/.env-qa/lib/python3.8/site-packages
numpy-1.22.0|~/.env-qf/lib/python3.8/site-packages
numpy-1.22.2|~/.env310/lib/python3.10/site-packages
numpy-1.22.4|~/.env-te/lib/python3.8/site-packages
numpy-1.23.5|~/.env-np/lib/python3.10/site-packages
numpy-1.23.5|~/.env-yp/lib/python3.8/site-packages
numpy-1.23.5|~/.env-gp/lib/python3.8/site-packages
numpy-1.23.5|~/.env-hy/lib/python3.8/site-packages
numpy-1.23.5|~/.env-tn/lib/python3.9/site-packages
numpy-1.23.5|~/.env-tl/lib/python3.11/site-packages
numpy-1.24.2|~/.env-am/lib/python3.8/site-packages
numpy-1.24.2|~/.env-er/lib/python3.11/site-packages
numpy-1.24.2|~/.env-aw/lib/python3.8/site-packages
numpy-1.24.3|~/.env-tl/lib/python3.11/site-packages
numpy-1.24.3|~/.env-ak/lib/python3.8/site-packages
numpy-1.24.3|~/.env-uv/lib/python3.11/site-packages
numpy-1.24.4|~/.env-rt/lib/python3.8/site-packages
numpy-1.25.1|~/.env-sf/lib/python3.11/site-packages
numpy-1.26.0|~/.env-fb/lib/python3.11/site-packages
numpy-1.26.2|~/.env-rr/lib/python3.12/site-packages
numpy-1.26.2|~/.env-tt/lib/python3.12/site-packages
numpy-1.26.4|~/.env-df/lib/python3.12/site-packages
numpy-1.26.4|~/.env-sg/lib/python3.11/site-packages
numpy-2.0.0|~/.env-tt/lib/python3.12/site-packages
numpy-2.0.0|~/.env-sq/lib/python3.12/site-packages
numpy-2.1.2|~/.env-lt/lib/python3.11/site-packages
'''

## Conclusion

Should I be concerned that I have eight different versions of `zipp`, or fifteen different versioons of `numpy` on system? Maybe: I might return to these environments and execute code for which new vulnerabilities have been discovered, potentially putting my system at risk of exploit or malware.

It would be better to enforce environment or system-wide controls on what packages can be installed,
