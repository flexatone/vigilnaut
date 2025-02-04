use crate::path_shared::PathShared;
use crate::validation_report::ValidationFlags;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::path::Path;

/// Produce the command line argument to reproduce a validation command.
fn to_validation_command(
    executable: &PathBuf, // only accept one
    bound: &Path,
    bound_options: Option<Vec<String>>,
    vf: &ValidationFlags,
    exit_else_warn: Option<i32>,
) -> String {
    let bo = bound_options.as_ref().map_or(String::new(), |vec| {
        format!(" --bound_options {}", vec.join(" "))
    });
    let ec =
        exit_else_warn.map_or(String::new(), |code| format!(" exit --code {}", code));
    format!(
        "fetter -e {} validate --bound {}{}{}{}{}",
        executable.display(),
        bound.display(),
        bo,
        if vf.permit_subset { " --subset" } else { "" },
        if vf.permit_superset {
            " --superset"
        } else {
            ""
        },
        ec,
    )
}

fn to_validation_subprocess(
    executable: &PathBuf,
    bound: &PathBuf,
    bound_options: Option<Vec<String>>,
    vf: &ValidationFlags,
    exit_else_warn: Option<i32>,
) -> String {
    let cmd = to_validation_command(executable, bound, bound_options, vf, exit_else_warn);
    let eew = exit_else_warn.map_or(String::new(), |_| ", check=True".to_string());
    format!(
        "print('here');from subprocess import run;run('{}'.split(' '){}, capture_output=True)",
        cmd, eew
    )
}

pub(crate) fn to_sitecustomize(
    executable: &PathBuf,
    bound: &PathBuf,
    bound_options: Option<Vec<String>>,
    vf: &ValidationFlags,
    exit_else_warn: Option<i32>,
    site: &PathShared,
) {
    let code =
        to_validation_subprocess(executable, bound, bound_options, vf, exit_else_warn);
    let fp = site.join("sitecustomize.py");
    eprintln!("writing: {}", fp.display());
    let mut file = File::create(&fp).unwrap();
    writeln!(file, "{}", code).unwrap();
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_validation_command_a() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let post = to_validation_command(&exe, &bound, bound_options, &vf, None);
        assert_eq!(
            post,
            "fetter -e python3 validate --bound requirements.txt --subset"
        )
    }
    #[test]
    fn test_to_validation_command_b() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = Some(vec!["foo".to_string(), "bar".to_string()]);
        let vf = ValidationFlags {
            permit_superset: true,
            permit_subset: true,
        };
        let post = to_validation_command(&exe, &bound, bound_options, &vf, None);
        assert_eq!(post, "fetter -e python3 validate --bound requirements.txt --bound_options foo bar --subset --superset")
    }
    #[test]
    fn test_to_validation_command_c() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = Some(vec!["foo".to_string(), "bar".to_string()]);
        let vf = ValidationFlags {
            permit_superset: true,
            permit_subset: true,
        };
        let ec: Option<i32> = Some(4);
        let post = to_validation_command(&exe, &bound, bound_options, &vf, ec);
        assert_eq!(post, "fetter -e python3 validate --bound requirements.txt --bound_options foo bar --subset --superset exit --code 4")
    }
    //--------------------------------------------------------------------------

    #[test]
    fn test_to_validation_subprocess_a() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let ec: Option<i32> = Some(4);
        let post = to_validation_subprocess(&exe, &bound, bound_options, &vf, ec);
        assert_eq!(post, "from subprocess import run;run('fetter -e python3 validate --bound requirements.txt --subset exit --code 4'.split(' '), check=True)")
    }

    #[test]
    fn test_to_validation_subprocess_b() {
        let exe = PathBuf::from("python3");
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let ec: Option<i32> = None;
        let post = to_validation_subprocess(&exe, &bound, bound_options, &vf, ec);
        assert_eq!(post, "from subprocess import run;run('fetter -e python3 validate --bound requirements.txt --subset'.split(' '))")
    }
}
