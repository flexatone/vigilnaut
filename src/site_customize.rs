use crate::path_shared::PathShared;
use crate::util::logger;
use crate::validation_report::ValidationFlags;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

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
) -> String {
    let cmd = get_validation_command(executable, bound, bound_options, vf);
    // NOTE: exit_else_warn is only handled here to achieve a true exit; raising an exception will not abort the process
    let eew = exit_else_warn.map_or(String::new(), |i| {
        format!(
            "import sys;sys.stdout.flush();sys.exit({}) if r.returncode != 0 else None",
            i
        )
    });
    format!(
        "from subprocess import run;r = run('{}'.split(' '));{}",
        cmd, eew
    )
}

pub(crate) fn to_sitecustomize(
    executable: &Path,
    bound: &Path,
    bound_options: Option<Vec<String>>,
    vf: &ValidationFlags,
    exit_else_warn: Option<i32>,
    site: &PathShared,
    log: bool,
) -> io::Result<()> {
    let code =
        get_validation_subprocess(executable, bound, bound_options, vf, exit_else_warn);
    let fp = site.join("sitecustomize.py");
    if log {
        logger!(module_path!(), "Writing: {}", fp.display());
    }
    let mut file = File::create(&fp)?;
    writeln!(file, "{}", code)?;
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
        let post = get_validation_subprocess(&exe, &bound, bound_options, &vf, ec);
        assert_eq!(post, "from subprocess import run;r = run('fetter -e python3 validate --bound requirements.txt --subset'.split(' '));import sys;sys.stdout.flush();sys.exit(4) if r.returncode != 0 else None")
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
        let post = get_validation_subprocess(&exe, &bound, bound_options, &vf, ec);
        assert_eq!(post, "from subprocess import run;r = run('fetter -e python3 validate --bound requirements.txt --subset'.split(' '));")
    }
}
