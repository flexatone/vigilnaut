use std::process;
// use std::str::FromStr;

use crate::validation_report::ValidationFlags;
use clap::{Parser, Subcommand, ValueEnum};
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crate::dep_manifest::DepManifest;
use crate::scan_fs::Anchor;
use crate::scan_fs::ScanFS;
use crate::spin::print_banner;
use crate::spin::spin;
use crate::table::Tableable;
use crate::ureq_client::UreqClientLive;
use crate::util::logger;
use crate::util::path_normalize;
use crate::util::DURATION_0;

//------------------------------------------------------------------------------
// utility enums

#[derive(Copy, Clone, ValueEnum)]
enum CliAnchor {
    Lower,
    Upper,
    Both,
}
impl From<CliAnchor> for Anchor {
    fn from(cli_anchor: CliAnchor) -> Self {
        match cli_anchor {
            CliAnchor::Lower => Anchor::Lower,
            CliAnchor::Upper => Anchor::Upper,
            CliAnchor::Both => Anchor::Both,
        }
    }
}

//------------------------------------------------------------------------------

const ERROR_EXIT_CODE: i32 = 3;
const TITLE: &str = "fetter: System-wide Python package discovery and validation";

const AFTER_HELP: &str = "\
Examples:
  fetter scan
  fetter scan write -o /tmp/pkgscan.txt --delimiter '|'

  fetter search --pattern pip*

  fetter count

  fetter --exe python3 derive -a lower write -o /tmp/bound_requirements.txt

  fetter validate --bound /tmp/bound_requirements.txt
  fetter -e python3 validate --bound /tmp/bound_requirements.txt
  fetter -e python3 validate --superset --bound git@github.com:fetter-io/bound-test.git

  fetter audit
  fetter -e python3 -e /usr/bin/python audit write -o /tmp/audit.txt  -d '|'

  fetter -e python3 unpack-count
  fetter unpack-count -p pip*

  fetter -e /usr/bin/python purge-pattern -p numpy*

  fetter purge-invalid --bound /tmp/bound_requirements.txt
";

#[derive(clap::Parser)]
#[command(version, about, long_about = TITLE, after_help = AFTER_HELP)]
struct Cli {
    /// Zero or more executable paths to derive site package locations. If not provided, all discoverable executables will be used.
    #[arg(
        short,
        long,
        value_name = "EXECUTABLES",
        required = false,
        default_value = "*"
    )]
    exe: Vec<PathBuf>,

    /// Create or use a cache that expires after the provided number of seconds. A duration of zero will disable caching.
    #[arg(long, short, required = false, default_value = "40")]
    cache_duration: u64,

    /// Disable logging and terminal animation.
    #[arg(long, short)]
    quiet: bool, // TODO: recast to reference spinners / animation

    /// Enable logging.
    #[arg(long, short)]
    log: bool,

    /// Print header and version information.
    #[arg(long, short, required = false)]
    banner: Option<String>,

    /// Force inclusion of the user site-packages, even if it is not activated. If not set, user site packages will only be included if the interpreter has been configured to use it.
    #[arg(long, required = false)]
    user_site: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan environment to report on installed packages.
    Scan {
        #[command(subcommand)]
        subcommands: Option<ScanSubcommand>,
    },
    /// Search environment to report on installed packages.
    Search {
        /// Provide a glob-like pattern to match packages.
        #[arg(short, long)]
        pattern: String,

        #[arg(long)]
        case: bool,

        #[command(subcommand)]
        subcommands: Option<SearchSubcommand>,
    },
    /// Count discovered executables, sites, and packages.
    Count {
        #[command(subcommand)]
        subcommands: Option<CountSubcommand>,
    },
    /// Derive new requirements from discovered packages.
    Derive {
        // Select the nature of the bound in the derived requirements.
        #[arg(short, long, value_enum)]
        anchor: CliAnchor,

        #[command(subcommand)]
        subcommands: Option<DeriveSubcommand>,
    },
    /// Validate if packages conform to a validation target.
    Validate {
        /// File path or URL from which to read bound requirements.
        #[arg(short, long, value_name = "FILE")]
        bound: PathBuf,

        /// Names of additional optional (extra) dependency groups.
        #[arg(long, value_name = "OPTIONS")]
        bound_options: Option<Vec<String>>,

        /// If the subset flag is set, the observed packages can be a subset of the bound requirements.
        #[arg(long)]
        subset: bool,

        /// If the superset flag is set, the observed packages can be a superset of the bound requirements.
        #[arg(long)]
        superset: bool,

        #[command(subcommand)]
        subcommands: Option<ValidateSubcommand>,
    },
    /// Install in site-packages automatic validation checks on every Python run.
    SiteInstall {
        /// File path or URL from which to read bound requirements.
        #[arg(short, long, value_name = "FILE")]
        bound: PathBuf,

        /// Names of additional optional (extra) dependency groups.
        #[arg(long, value_name = "OPTIONS")]
        bound_options: Option<Vec<String>>,

        /// If the subset flag is set, the observed packages can be a subset of the bound requirements.
        #[arg(long)]
        subset: bool,

        /// If the superset flag is set, the observed packages can be a superset of the bound requirements.
        #[arg(long)]
        superset: bool,

        #[command(subcommand)]
        subcommands: Option<SiteInstallSubcommand>,
    },
    /// Uninstall from site-packages automatic validation checks on every Python run.
    SiteUninstall,
    /// Search for package security vulnerabilities via the OSV DB.
    Audit {
        /// Provide a glob-like pattern to select packages.
        #[arg(short, long, default_value = "*")]
        pattern: String,

        /// Enable case-sensitive pattern matching.
        #[arg(long)]
        case: bool,

        #[command(subcommand)]
        subcommands: Option<AuditSubcommand>,
    },
    /// Discover counts of all installed packages artifacts.
    UnpackCount {
        /// Provide a glob-like pattern to select packages.
        #[arg(short, long, default_value = "*")]
        pattern: String,

        /// Enable case-sensitive pattern matching.
        #[arg(long)]
        case: bool,

        #[command(subcommand)]
        subcommands: Option<UnpackCountSubcommand>,
    },
    /// Discover file names of all installed package artifacts.
    UnpackFiles {
        /// Provide a glob-like pattern to select packages.
        #[arg(short, long, default_value = "*")]
        pattern: String,

        /// Enable case-sensitive pattern matching.
        #[arg(long)]
        case: bool,

        #[command(subcommand)]
        subcommands: Option<UnpackFilesSubcommand>,
    },
    /// Purge packages that match a search pattern.
    PurgePattern {
        /// Provide a glob-like pattern to select packages.
        #[arg(short, long, default_value = "*")]
        pattern: Option<String>,

        /// Enable case-sensitive pattern matching.
        #[arg(long)]
        case: bool,
    },
    /// Purge packages that are invalid based on dependency specification.
    PurgeInvalid {
        /// File path or URL from which to read bound requirements.
        #[arg(short, long, value_name = "FILE")]
        bound: PathBuf,

        /// Names of additional optional dependency groups.
        #[arg(long, value_name = "OPTIONS")]
        bound_options: Option<Vec<String>>,

        /// If the subset flag is set, the observed packages can be a subset of the bound requirements.
        #[arg(long)]
        subset: bool,

        /// If the superset flag is set, the observed packages can be a superset of the bound requirements.
        #[arg(long)]
        superset: bool,
    },
}

impl fmt::Display for Commands {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let op_str = match self {
            Commands::Scan { .. } => "scan",
            Commands::Search { .. } => "search",
            Commands::Count { .. } => "count",
            Commands::Derive { .. } => "derive",
            Commands::Validate { .. } => "validate",
            Commands::SiteInstall { .. } => "site-install",
            Commands::SiteUninstall { .. } => "site-uninstall",
            Commands::Audit { .. } => "audit",
            Commands::UnpackCount { .. } => "unpack-count",
            Commands::UnpackFiles { .. } => "unpack-files",
            Commands::PurgePattern { .. } => "purge-pattern",
            Commands::PurgeInvalid { .. } => "purge-invalid",
        };
        write!(f, "{}", op_str)
    }
}

//------------------------------------------------------------------------------
#[derive(Subcommand)]
enum ScanSubcommand {
    /// Display scan in the terminal.
    Display,
    /// Write a scan report to a file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long, default_value = ",")]
        delimiter: char,
    },
}

#[derive(Subcommand)]
enum SearchSubcommand {
    /// Display search int the terminal.
    Display,
    /// Write a search report to a file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long, default_value = ",")]
        delimiter: char,
    },
}

#[derive(Subcommand)]
enum CountSubcommand {
    /// Display scan in the terminal.
    Display,
    /// Write a report to a delimited file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long, default_value = ",")]
        delimiter: char,
    },
}

#[derive(Subcommand)]
enum DeriveSubcommand {
    /// Display derive in the terminal.
    Display,
    /// Write a derive report to a file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum ValidateSubcommand {
    /// Display validation in the terminal.
    Display,
    /// Print a Json representation of validation results.
    Json,
    /// Write a validation report to a file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long, default_value = ",")]
        delimiter: char,
    },
    /// Return an exit code, 0 on success, 3 (by default) on error.
    Exit {
        #[arg(short, long, default_value = "3")]
        code: i32,
    },
}

#[derive(Subcommand)]
enum SiteInstallSubcommand {
    /// Print a Json representation of validation results.
    Warn,
    /// Return an exit code, 0 on success, 3 (by default) on error.
    Exit {
        #[arg(short, long, default_value = "3")]
        code: i32,
    },
}

#[derive(Subcommand)]
enum AuditSubcommand {
    /// Display audit results in the terminal.
    Display,
    /// Write audit results to a delimited file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long, default_value = ",")]
        delimiter: char,
    },
}

#[derive(Subcommand)]
enum UnpackCountSubcommand {
    /// Display installed artifacts in the terminal.
    Display,
    /// Write installed artifacts to a delimited file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long, default_value = ",")]
        delimiter: char,
    },
}

#[derive(Subcommand)]
enum UnpackFilesSubcommand {
    /// Display installed artifacts in the terminal.
    Display,
    /// Write installed artifacts to a delimited file.
    Write {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long, default_value = ",")]
        delimiter: char,
    },
}

//------------------------------------------------------------------------------
// Utility constructors specialized fro CLI contexts

// Provided `exe_paths` are not normalize.
fn get_scan(
    exe_paths: &Vec<PathBuf>, // could be a ref
    force_usite: bool,
    animate: bool,
    cache_dur: Duration,
    log: bool,
) -> Result<ScanFS, Box<dyn std::error::Error>> {
    ScanFS::from_cache(exe_paths, force_usite, cache_dur, log).or_else(|err| {
        if log {
            logger!(module_path!(), "Could not load from cache: {:?}", err);
        }
        // full load
        let active = Arc::new(AtomicBool::new(true));
        if animate {
            spin(active.clone(), "scanning".to_string());
        }
        let sfsl = ScanFS::from_exes(exe_paths, force_usite, log)?;

        if cache_dur > DURATION_0 {
            sfsl.to_cache(cache_dur, log)?;
        }
        if animate {
            active.store(false, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(100));
        }
        Ok(sfsl)
    })
}

// Given a Path, load a DepManifest. This might branch by extension to handle pyproject.toml and other formats.
fn get_dep_manifest(
    bound: &PathBuf,
    bound_options: Option<&Vec<String>>,
) -> Result<DepManifest, Box<dyn std::error::Error>> {
    if bound.to_str().is_some_and(|s| s.ends_with(".git")) {
        // if bound.to_str().map_or(false, |s| s.ends_with(".git")) {
        DepManifest::from_git_repo(bound, bound_options)
    } else if bound
        .to_str()
        .is_some_and(|s| s.ends_with("pyproject.toml"))
    {
        DepManifest::from_pyproject_file(bound, bound_options)
    } else if bound.to_str().is_some_and(|s| s.starts_with("http")) {
        // might have URL based requirements or pyproject
        DepManifest::from_url(&UreqClientLive, bound, bound_options)
    } else {
        // assume all text files are requirements-style
        let fp = path_normalize(bound).unwrap_or_else(|_| bound.clone());
        DepManifest::from_requirements_file(&fp)
    }
}

//------------------------------------------------------------------------------
pub fn run_cli<I, T>(args: I) -> Result<(), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    if env::consts::OS != "macos" && env::consts::OS != "linux" {
        return Err("No support for this platform. To request support, visit https://github.com/fetter-io/fetter-rs/issues/66".into());
    }
    let cli = Cli::parse_from(args);
    if cli.command.is_none() {
        return Err("No command provided. For more information, try '--help'.".into());
    }
    let log = cli.log;
    let quiet = cli.quiet;

    if cli.banner.is_some() {
        print_banner(cli.banner);
    }

    // doa fresh scan or load a cached scan
    let sfs = get_scan(
        &cli.exe,
        cli.user_site,
        !quiet,
        Duration::from_secs(cli.cache_duration),
        log,
    )?;

    match &cli.command {
        Some(Commands::Scan { subcommands }) => match subcommands {
            Some(ScanSubcommand::Write { output, delimiter }) => {
                let sr = sfs.to_scan_report();
                let _ = sr.to_file(output, *delimiter);
            }
            Some(ScanSubcommand::Display) | None => {
                let sr = sfs.to_scan_report();
                let _ = sr.to_stdout();
            }
        },
        Some(Commands::Search {
            subcommands,
            pattern,
            case,
        }) => match subcommands {
            Some(SearchSubcommand::Write { output, delimiter }) => {
                let sr = sfs.to_search_report(pattern, !case);
                let _ = sr.to_file(output, *delimiter);
            }
            Some(SearchSubcommand::Display) | None => {
                // default
                let sr = sfs.to_search_report(pattern, !case);
                let _ = sr.to_stdout();
            }
        },
        Some(Commands::Count { subcommands }) => match subcommands {
            Some(CountSubcommand::Write { output, delimiter }) => {
                let cr = sfs.to_count_report();
                let _ = cr.to_file(output, *delimiter);
            }
            Some(CountSubcommand::Display) | None => {
                // default
                let cr = sfs.to_count_report();
                let _ = cr.to_stdout();
            }
        },
        Some(Commands::Derive {
            subcommands,
            anchor,
        }) => match subcommands {
            Some(DeriveSubcommand::Write { output }) => {
                let dm = sfs.to_dep_manifest((*anchor).into())?;
                let dmr = dm.to_dep_manifest_report();
                let _ = dmr.to_file(output, ' ');
            }
            Some(DeriveSubcommand::Display) | None => {
                // default
                let dm = sfs.to_dep_manifest((*anchor).into())?;
                let dmr = dm.to_dep_manifest_report();
                let _ = dmr.to_stdout();
            }
        },
        Some(Commands::Validate {
            bound,
            bound_options,
            subset,
            superset,
            subcommands,
        }) => {
            let dm = get_dep_manifest(bound, bound_options.as_ref())?;
            let permit_superset = *superset;
            let permit_subset = *subset;
            let vr = sfs.to_validation_report(
                dm,
                ValidationFlags {
                    permit_superset,
                    permit_subset,
                },
            );
            match subcommands {
                Some(ValidateSubcommand::Json) => {
                    println!("{}", serde_json::to_string(&vr.to_validation_digest())?);
                }
                Some(ValidateSubcommand::Write { output, delimiter }) => {
                    let _ = vr.to_file(output, *delimiter);
                }
                Some(ValidateSubcommand::Exit { code }) => {
                    process::exit(if vr.len() > 0 { *code } else { 0 });
                }
                Some(ValidateSubcommand::Display) | None => {
                    // default
                    let _ = vr.to_stdout()?;
                    process::exit(if vr.len() > 0 { ERROR_EXIT_CODE } else { 0 });
                }
            }
        }
        Some(Commands::SiteInstall {
            bound,
            bound_options,
            subset,
            superset,
            subcommands,
        }) => {
            let vf = ValidationFlags {
                permit_superset: *superset,
                permit_subset: *subset,
            };
            let exit_else_warn: Option<i32> = match subcommands {
                Some(SiteInstallSubcommand::Warn) | None => None,
                Some(SiteInstallSubcommand::Exit { code }) => Some(*code),
            };
            sfs.site_validate_install(bound, bound_options, &vf, exit_else_warn, log)?;
        }
        Some(Commands::SiteUninstall {}) => {
            sfs.site_validate_uninstall(log)?;
        }
        Some(Commands::Audit {
            subcommands,
            pattern,
            case,
        }) => {
            // network look makes this potentially slow
            let active = Arc::new(AtomicBool::new(true));
            if !quiet {
                spin(active.clone(), "vulnerability searching".to_string());
            }
            let ar = sfs.to_audit_report(pattern, !case);
            if !quiet {
                active.store(false, Ordering::Relaxed);
                thread::sleep(Duration::from_millis(100));
            }
            match subcommands {
                Some(AuditSubcommand::Write { output, delimiter }) => {
                    let _ = ar.to_file(output, *delimiter);
                } // NOTE: might add Json and Exit
                Some(AuditSubcommand::Display) | None => {
                    // default
                    let _ = ar.to_stdout();
                    process::exit(if ar.len() > 0 { ERROR_EXIT_CODE } else { 0 });
                }
            }
        }
        Some(Commands::UnpackCount {
            subcommands,
            pattern,
            case,
        }) => {
            let count = true;
            let ir = sfs.to_unpack_report(pattern, !case, count);
            match subcommands {
                Some(UnpackCountSubcommand::Write { output, delimiter }) => {
                    let _ = ir.to_file(output, *delimiter);
                }
                Some(UnpackCountSubcommand::Display) | None => {
                    // default
                    let _ = ir.to_stdout();
                }
            }
        }
        Some(Commands::UnpackFiles {
            subcommands,
            pattern,
            case,
        }) => {
            let count = false;
            let ir = sfs.to_unpack_report(pattern, !case, count);
            match subcommands {
                Some(UnpackFilesSubcommand::Write { output, delimiter }) => {
                    let _ = ir.to_file(output, *delimiter);
                }
                Some(UnpackFilesSubcommand::Display) | None => {
                    // default
                    let _ = ir.to_stdout();
                }
            }
        }
        Some(Commands::PurgePattern { pattern, case }) => {
            let _ = sfs.to_purge_pattern(pattern, !case, log);
        }
        Some(Commands::PurgeInvalid {
            bound,
            bound_options,
            subset,
            superset,
        }) => {
            let dm = get_dep_manifest(bound, bound_options.as_ref())?;
            let permit_superset = *superset;
            let permit_subset = *subset;
            let _ = sfs.to_purge_invalid(
                dm,
                ValidationFlags {
                    permit_superset,
                    permit_subset,
                },
                log,
            );
        }
        None => {}
    }
    Ok(())
}

//-----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    // use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_run_cli_a() {
        let _args = vec![OsString::from("fetter"), OsString::from("-h")];
        // run_cli(args); // print to stdout
    }
}
