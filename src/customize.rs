use crate::validation_report::ValidationFlags;
use std::path::PathBuf;

/// Produce the command line argument to reproduce a validation command.
pub(crate) fn to_validation_command(
    bound: PathBuf,
    bound_options: Option<Vec<String>>,
    vf: ValidationFlags,
) -> String {
    let bo = bound_options.as_ref().map_or(String::new(), |vec| {
        format!(" --bound_options {}", vec.join(" "))
    });

    let cmd = format!(
        "fetter validate --bound {}{}{}{}",
        bound.display(),
        bo,
        if vf.permit_subset { " --subset" } else { "" },
        if vf.permit_superset {
            " --superset"
        } else {
            ""
        },
    );
    cmd
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_validation_command_a() {
        let bound = PathBuf::from("requirements.txt");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: true,
        };
        let post = to_validation_command(bound, bound_options, vf);
        assert_eq!(post, "fetter validate --bound requirements.txt --subset")
    }
    #[test]
    fn test_to_validation_command_b() {
        let bound = PathBuf::from("requirements.txt");
        let bound_options = Some(vec!["foo".to_string(), "bar".to_string()]);
        let vf = ValidationFlags {
            permit_superset: true,
            permit_subset: true,
        };
        let post = to_validation_command(bound, bound_options, vf);
        assert_eq!(post, "fetter validate --bound requirements.txt --bound_options foo bar --subset --superset")
    }

}
