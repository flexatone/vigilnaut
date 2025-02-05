use crate::path_shared::PathShared;
use crate::util::logger;
use crate::validation_report::ValidationFlags;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

// const FETTER_BIN: &str = "target/release/fetter"; // for testing
const FETTER_BIN: &str = "fetter";

/// Produce the command line argument to reproduce a validation command.
fn get_validation_command(
    executable: &Path, // only accept one
    bound: &Path,
    bound_options: Option<Vec<String>>,
    vf: &ValidationFlags,
) -> String {
    let bo = bound_options.as_ref().map_or(String::new(), |vec| {
        format!(" --bound_options {}", vec.join(" "))
    });
    format!(
        "{} -e {} validate --bound {}{}{}{}",
        FETTER_BIN,
        executable.display(),
        bound.display(),
        bo,
        if vf.permit_subset { " --subset" } else { "" },
        if vf.permit_superset {
            " --superset"
        } else {
            ""
        },
    )
}

fn get_validation_subprocess(
    executable: &Path,
    bound: &Path,
    bound_options: Option<Vec<String>>,
    vf: &ValidationFlags,
    exit_else_warn: Option<i32>,
    cwd_option: Option<PathBuf>,
) -> String {
    let cmd = get_validation_command(executable, bound, bound_options, vf);
    // NOTE: exit_else_warn is only handled here to achieve a true exit; raising an exception with `check=True` will not abort the process
    let eew = exit_else_warn.map_or(String::new(), |i| {
        format!(
            "import sys\nif r.returncode != 0: sys.exit({}) # fetter validation failed",
            i
        )
    });
    let cwd = cwd_option.map_or(String::new(), |p| {
        format!(", cwd='{}'", p.display())
    });
    format!(
        "from subprocess import run\nr = run('{}'.split(' '){})\n{}",
        cmd, cwd, eew
    )
}

const FN_LAUNCHER_PTH: &str = "fetter_launcher.pth";
const FN_VALIDATE_PY: &str = "fetter_validate.py";

pub(crate) fn install_validation(
    executable: &Path,
    bound: &Path,
    bound_options: Option<Vec<String>>,
    vf: &ValidationFlags,
    exit_else_warn: Option<i32>,
    site: &PathShared,
    cwd_option: Option<PathBuf>,
    log: bool,
) -> io::Result<()> {
    let code =
        get_validation_subprocess(executable, bound, bound_options, vf, exit_else_warn, cwd_option);
    let fp_validate = site.join(FN_VALIDATE_PY);
    if log {
        logger!(module_path!(), "Writing: {}", fp_validate.display());
    }
    let mut file = File::create(&fp_validate)?;
    writeln!(file, "{}", code)?;

    let fp_launcher = site.join(FN_LAUNCHER_PTH);
    if log {
        logger!(module_path!(), "Writing: {}", fp_launcher.display());
    }
    let mut file = File::create(&fp_launcher)?;
    writeln!(file, "import fetter_validate\n")?;

    Ok(())
}

pub(crate) fn uninstall_validation(site: &PathShared, log: bool) -> io::Result<()> {
    let fp_launcher = site.join(FN_LAUNCHER_PTH);
    if log {
        logger!(module_path!(), "Removing: {}", fp_launcher.display());
    }
    let _ = fs::remove_file(fp_launcher);
    let fp_validate = site.join(FN_VALIDATE_PY);
    if log {
        logger!(module_path!(), "Removing: {}", fp_validate.display());
    }
    let _ = fs::remove_file(fp_validate);
    Ok(())
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_validation_command_a() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let post = get_validation_command(&exe, &bound, bound_options, &vf);
        assert_eq!(
            post,
            "fetter -e python3 validate --bound requirements.txt --subset"
        )
    }
    #[test]
    fn test_get_validation_command_b() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = Some(vec!["foo".to_string(), "bar".to_string()]);
        let vf = ValidationFlags {
            permit_superset: true,
            permit_subset: true,
        };
        let post = get_validation_command(&exe, &bound, bound_options, &vf);
        assert_eq!(post, "fetter -e python3 validate --bound requirements.txt --bound_options foo bar --subset --superset")
    }
    #[test]
    fn test_get_validation_command_c() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = Some(vec!["foo".to_string(), "bar".to_string()]);
        let vf = ValidationFlags {
            permit_superset: true,
            permit_subset: true,
        };
        let post = get_validation_command(&exe, &bound, bound_options, &vf);
        assert_eq!(post, "fetter -e python3 validate --bound requirements.txt --bound_options foo bar --subset --superset")
    }
    //--------------------------------------------------------------------------

    #[test]
    fn test_get_validation_subprocess_a() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let ec: Option<i32> = Some(4);
        let post = get_validation_subprocess(&exe, &bound, bound_options, &vf, ec, None);
        assert_eq!(post, "from subprocess import run\nr = run('fetter -e python3 validate --bound requirements.txt --subset'.split(' '))\nimport sys\nif r.returncode != 0: sys.exit(4) # fetter validation failed")
    }

    #[test]
    fn test_get_validation_subprocess_b() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let ec: Option<i32> = None;
        let post = get_validation_subprocess(&exe, &bound, bound_options, &vf, ec, None);
        assert_eq!(post, "from subprocess import run\nr = run('fetter -e python3 validate --bound requirements.txt --subset'.split(' '))\n")
    }

    #[test]
    fn test_get_validation_subprocess_c() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let ec: Option<i32> = None;
        let cwd = Some(PathBuf::from("/home/foo"));
        let post = get_validation_subprocess(&exe, &bound, bound_options, &vf, ec, cwd);
        assert_eq!(post, "from subprocess import run\nr = run('fetter -e python3 validate --bound requirements.txt --subset'.split(' '), cwd='/home/foo')\n")
    }
}

