use crate::util::ResultDynError;
use serde_json::Value as JsonValue;
use std::error::Error;
use toml::Value; // Use the `toml` crate for parsing

#[derive(Debug, PartialEq)]
enum LockFileType {
    Uv,
    Poetry,
    PipfileLock,
    Unknown,
}

#[derive(Debug)]
struct LockFile {
    file_type: LockFileType,
    contents: String,
}

impl LockFile {
    fn new(contents: String) -> Self {
        let file_type = Self::detect_type(&contents);
        Self {
            file_type,
            contents,
        }
    }

    fn detect_type(contents: &str) -> LockFileType {
        if let Ok(json) = serde_json::from_str::<JsonValue>(contents) {
            if json.get("_meta").is_some() && json.get("default").is_some() {
                return LockFileType::PipfileLock;
            }
        }

        let mut non_comment_lines = 0;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            non_comment_lines += 1;
            if non_comment_lines > 10 {
                break;
            }

            // Check for Poetry format
            if trimmed.starts_with("[metadata]") || trimmed.starts_with("[[package]]") {
                return LockFileType::Poetry;
            }

            // Check for uv format (uv files contain package specs without TOML formatting)
            return LockFileType::Uv;
        }

        LockFileType::Unknown
    }

    /// Extracts dependencies from a `uv` lock file.
    fn extract_uv_dependencies(&self) -> ResultDynError<Vec<String>> {
        let dependencies = self
            .contents
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return None;
                }
                if line.trim_start().starts_with("# via") {
                    return None;
                }
                Some(trimmed.to_string())
            })
            .collect();

        Ok(dependencies)
    }

    /// Extracts dependencies from a `Poetry` lock file and formats them as `package==version`.
    fn extract_poetry_dependencies(&self) -> ResultDynError<Vec<String>> {
        let parsed: Value = self.contents.parse()?; // Parse as TOML
        let mut dependencies = Vec::new();

        if let Some(packages) = parsed.get("package").and_then(|p| p.as_array()) {
            for package in packages {
                if let (Some(name), Some(version)) = (
                    package.get("name").and_then(|n| n.as_str()),
                    package.get("version").and_then(|v| v.as_str()),
                ) {
                    dependencies.push(format!("{}=={}", name, version));
                }
            }
        }

        Ok(dependencies)
    }

    fn extract_pipfilelock_dependencies(
        &self,
        options: Option<&Vec<String>>,
    ) -> ResultDynError<Vec<String>> {
        let mut groups = vec!["default".to_string()];
        if let Some(extra_groups) = options {
            groups.extend(extra_groups.iter().cloned());
        }

        let parsed: JsonValue = serde_json::from_str(&self.contents)?;
        let mut dependencies = Vec::new();
        for group in groups {
            if let Some(packages) = parsed.get(group).and_then(|g| g.as_object()) {
                for (name, details) in packages.iter() {
                    if let Some(version) = details.get("version").and_then(|v| v.as_str())
                    {
                        dependencies.push(format!("{}{}", name, version));
                    }
                }
            }
        }

        Ok(dependencies)
    }

    /// Extracts dependency specifications from the lock file.
    fn get_dependencies(
        &self,
        options: Option<&Vec<String>>,
    ) -> ResultDynError<Vec<String>> {
        if options.is_some() && self.file_type != LockFileType::PipfileLock {
            return Err("Options can only be used with Pipfile.lock".into());
        }

        match self.file_type {
            LockFileType::Uv => self.extract_uv_dependencies(),
            LockFileType::Poetry => self.extract_poetry_dependencies(),
            LockFileType::PipfileLock => self.extract_pipfilelock_dependencies(options),
            LockFileType::Unknown => Err("Unknown lock file format".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uv_get_dependencies() {
        let uv_contents = r#"
opentelemetry-api==1.24.0
    # via
    #   apache-airflow
    #   opentelemetry-exporter-otlp-proto-grpc
    #   opentelemetry-exporter-otlp-proto-http
opentelemetry-exporter-otlp==1.24.0
    # via apache-airflow
apache-airflow
"#;
        let lockfile = LockFile::new(uv_contents.to_string());
        let dependencies = lockfile.get_dependencies(None).unwrap();

        assert_eq!(
            dependencies,
            vec![
                "opentelemetry-api==1.24.0".to_string(),
                "opentelemetry-exporter-otlp==1.24.0".to_string(),
                "apache-airflow".to_string(),
            ]
        );
    }

    #[test]
    fn test_poetry_get_dependencies_with_versions() {
        let poetry_contents = r#"
            [[package]]
            name = "packaging"
            version = "24.2"

            [[package]]
            name = "requests"
            version = "2.31.0"
        "#;
        let lockfile = LockFile::new(poetry_contents.to_string());
        let dependencies = lockfile.get_dependencies(None).unwrap();
        assert_eq!(dependencies, vec!["packaging==24.2", "requests==2.31.0"]);
    }

    #[test]
    fn test_pipfilelock_get_dependencies() {
        let pipfile_lock_contents = r#"
        {
            "_meta": { "hash": { "sha256": "abc123" } },
            "default": {
                "asgiref": { "version": "==3.6.0" },
                "django": { "version": "==4.1.7" }
            },
            "develop": {
                "attrs": { "version": "==22.2.0" }
            }
        }
        "#;

        let lockfile = LockFile::new(pipfile_lock_contents.to_string());

        let dependencies_default = lockfile.get_dependencies(None).unwrap();
        assert_eq!(
            dependencies_default,
            vec!["asgiref==3.6.0", "django==4.1.7"]
        );

        let dependencies_with_develop = lockfile
            .get_dependencies(Some(&vec!["develop".to_string()]))
            .unwrap();
        assert_eq!(
            dependencies_with_develop,
            vec!["asgiref==3.6.0", "django==4.1.7", "attrs==22.2.0"]
        );
    }
}
