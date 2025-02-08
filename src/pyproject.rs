use std::fs;
use std::path::Path;
use toml::Value;

#[derive(Debug)]
struct PyProjectInfo {
    parsed: Value,
    has_project_dep: bool,
    has_project_dep_optional: bool,
    has_poetry_dep: bool,
    has_poetry_dep_group: bool,
}

impl PyProjectInfo {
    /// Parses `pyproject.toml` and initializes the struct with stored values.
    fn from_string(contents: &String) -> Result<Self, Box<dyn std::error::Error>> {
        let parsed: Value = toml::from_str(&contents)?;

        let has_project_dep = parsed
            .get("project")
            .and_then(|t| t.get("dependencies"))
            .is_some();

        let has_project_dep_optional = parsed
            .get("project")
            .and_then(|t| t.get("optional-dependencies"))
            .is_some();

        let has_poetry_dep = parsed
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|t| t.get("dependencies"))
            .is_some();

        let has_poetry_dep_group = parsed
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("group"))
            .and_then(|groups| groups.as_table())
            .is_some_and(|groups| {
                groups
                    .values()
                    .any(|group| group.get("dependencies").is_some())
            });

        Ok(Self {
            parsed,
            has_project_dep,
            has_project_dep_optional,
            has_poetry_dep,
            has_poetry_dep_group,
        })
    }

    fn from_file(fp: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(fp)?;
        Self::from_string(&contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper function to create a temporary pyproject.toml file.
    fn create_temp_pyproject(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        file
    }

    #[test]
    fn test_detects_project_dependencies() {
        let contents = r#"
        [project]
        dependencies = ["requests", "numpy"]
        "#
        .to_string();

        let info = PyProjectInfo::from_string(&contents).expect("Failed to parse toml");
        assert!(info.has_project_dep);
        assert!(!info.has_project_dep_optional);
        assert!(!info.has_poetry_dep);
        assert!(!info.has_poetry_dep_group);
    }

    #[test]
    fn test_detects_optional_project_dependencies() {
        let contents = r#"
        [project]
        optional-dependencies = { dev = ["pytest", "black"] }
        "#
        .to_string();

        let info = PyProjectInfo::from_string(&contents).expect("Failed to parse toml");
        assert!(!info.has_project_dep);
        assert!(info.has_project_dep_optional);
        assert!(!info.has_poetry_dep);
        assert!(!info.has_poetry_dep_group);
    }

    #[test]
    fn test_detects_poetry_dependencies() {
        let contents = r#"
        [tool.poetry]
        dependencies = { requests = "^2.25.1", numpy = "^1.21.0" }
        "#
        .to_string();

        let info = PyProjectInfo::from_string(&contents).expect("Failed to parse toml");
        assert!(!info.has_project_dep);
        assert!(!info.has_project_dep_optional);
        assert!(info.has_poetry_dep);
        assert!(!info.has_poetry_dep_group);
    }

    #[test]
    fn test_detects_poetry_dependency_groups() {
        let contents = r#"
        [tool.poetry.group.dev.dependencies]
        pytest = "^6.2.5"
        black = "^21.7b0"

        [tool.poetry.group.docs.dependencies]
        sphinx = "^4.0.0"
        "#
        .to_string();

        let info = PyProjectInfo::from_string(&contents).expect("Failed to parse toml");
        assert!(!info.has_project_dep);
        assert!(!info.has_project_dep_optional);
        assert!(!info.has_poetry_dep);
        assert!(info.has_poetry_dep_group);
    }

    #[test]
    fn test_detects_multiple_project_and_poetry_dependencies() {
        let contents = r#"
        [project]
        dependencies = ["requests", "numpy"]
        optional-dependencies = { dev = ["pytest", "black"] }

        [tool.poetry]
        dependencies = { flask = "^2.0.0" }

        [tool.poetry.group.test.dependencies]
        pytest = "^6.2.5"
        "#
        .to_string();

        let info = PyProjectInfo::from_string(&contents).expect("Failed to parse toml");
        assert!(info.has_project_dep);
        assert!(info.has_project_dep_optional);
        assert!(info.has_poetry_dep);
        assert!(info.has_poetry_dep_group);
    }

    #[test]
    fn test_no_dependencies_detected() {
        let contents = r#"
        [build-system]
        requires = ["setuptools", "wheel"]
        build-backend = "setuptools.build_meta"
        "#
        .to_string();

        let info = PyProjectInfo::from_string(&contents).expect("Failed to parse toml");
        assert!(!info.has_project_dep);
        assert!(!info.has_project_dep_optional);
        assert!(!info.has_poetry_dep);
        assert!(!info.has_poetry_dep_group);
    }
}
