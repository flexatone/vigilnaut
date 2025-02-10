use crate::util::ResultDynError;
use serde_json::Value as JsonValue;
use toml::Value as TomlValue; // Use the `toml` crate for parsing

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

        let mut count = 0;
        for line in contents.lines() {
            let trimmed = line.trim();
            // both uv/requests and toml poetry files use # comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            count += 1;
            if count > 20 {
                break;
            }
            // Poetry toml format
            if trimmed.starts_with("[metadata]") || trimmed.starts_with("[[package]]") {
                return LockFileType::Poetry;
            }
            return LockFileType::Uv;
        }

        LockFileType::Unknown
    }

    /// Extracts dependencies from a `uv` lock file. Unlike with  requirements.txt files, this will not try to load other files
    fn get_uv_dep(&self) -> ResultDynError<Vec<String>> {
        let dependencies = self
            .contents
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return None;
                }
                Some(trimmed.to_string())
            })
            .collect();
        Ok(dependencies)
    }

    /// Extracts dependencies from a `Poetry` lock file and formats them as `package==version`.
    fn get_poetry_dep(&self) -> ResultDynError<Vec<String>> {
        let parsed: TomlValue = self.contents.parse()?; // Parse as TOML
        let mut dependencies = Vec::new();

        if let Some(packages) = parsed.get("package").and_then(|p| p.as_array()) {
            for package in packages {
                // assume that there is always a version nubmer
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

    fn get_pipfilelock_dep(
        &self,
        options: Option<&Vec<String>>,
    ) -> ResultDynError<Vec<String>> {
        // always include `default`
        let mut groups = vec!["default".to_string()];
        if let Some(opts) = options {
            groups.extend(opts.iter().cloned());
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
            LockFileType::Uv => self.get_uv_dep(),
            LockFileType::Poetry => self.get_poetry_dep(),
            LockFileType::PipfileLock => self.get_pipfilelock_dep(options),
            LockFileType::Unknown => Err("Unknown lock file format".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_dependencies_uv_a() {
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
    fn test_get_dependencies_poetry_a() {
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
    fn test_get_dependencies_poetry_b() {
        let poetry_contents = r#"
# This file is automatically @generated by Poetry 2.0.1 and should not be changed by hand.

[[package]]
name = "arraykit"
version = "0.10.0"
description = "Array utilities for StaticFrame"
optional = false
python-versions = ">=3.9"
groups = ["main"]

[package.dependencies]
numpy = ">=1.19.5"

[[package]]
name = "arraymap"
version = "0.4.0"
description = "Dictionary-like lookup from NumPy array values to their integer positions"
optional = false
python-versions = ">=3.9"
groups = ["main"]

[package.dependencies]
numpy = ">=1.19.5"

[[package]]
name = "certifi"
version = "2025.1.31"
description = "Python package for providing Mozilla's CA Bundle."
optional = false
python-versions = ">=3.6"
groups = ["main"]
files = [
    {file = "certifi-2025.1.31-py3-none-any.whl", hash = "sha256:ca78db4565a652026a4db2bcdf68f2fb589ea80d0be70e03929ed730746b84fe"},
    {file = "certifi-2025.1.31.tar.gz", hash = "sha256:3d5da6925056f6f18f119200434a4780a94263f10d1c21d032a6f6b2baa20651"},
]

[[package]]
name = "charset-normalizer"
version = "3.4.1"
description = "The Real First Universal Charset Detector. Open, modern and actively maintained alternative to Chardet."
optional = false
python-versions = ">=3.7"
groups = ["main"]

[[package]]
name = "idna"
version = "3.10"
description = "Internationalized Domain Names in Applications (IDNA)"
optional = false
python-versions = ">=3.6"
groups = ["main"]
files = [
    {file = "idna-3.10-py3-none-any.whl", hash = "sha256:946d195a0d259cbba61165e88e65941f16e9b36ea6ddb97f00452bae8b1287d3"},
    {file = "idna-3.10.tar.gz", hash = "sha256:12f65c9b470abda6dc35cf8e63cc574b1c52b11df2c86030af0ac09b01b13ea9"},
]

[package.extras]
all = ["flake8 (>=7.1.1)", "mypy (>=1.11.2)", "pytest (>=8.3.2)", "ruff (>=0.6.2)"]

[[package]]
name = "jinja2"
version = "3.1.3"
description = "A very fast and expressive template engine."
optional = false
python-versions = ">=3.7"
groups = ["main"]
files = [
    {file = "Jinja2-3.1.3-py3-none-any.whl", hash = "sha256:7d6d50dd97d52cbc355597bd845fabfbac3f551e1f99619e39a35ce8c370b5fa"},
    {file = "Jinja2-3.1.3.tar.gz", hash = "sha256:ac8bd6544d4bb2c9792bf3a159e80bba8fda7f07e81bc3aed565432d5925ba90"},
]

[package.dependencies]
MarkupSafe = ">=2.0"

[package.extras]
i18n = ["Babel (>=2.7)"]

[[package]]
name = "markupsafe"
version = "3.0.2"
description = "Safely add untrusted strings to HTML/XML markup."
optional = false
python-versions = ">=3.9"
groups = ["main"]

[[package]]
name = "numpy"
version = "2.2.2"
description = "Fundamental package for array computing in Python"
optional = false
python-versions = ">=3.10"
groups = ["main"]

[[package]]
name = "requests"
version = "2.32.3"
description = "Python HTTP for Humans."
optional = false
python-versions = ">=3.8"
groups = ["main"]
files = [
    {file = "requests-2.32.3-py3-none-any.whl", hash = "sha256:70761cfe03c773ceb22aa2f671b4757976145175cdfca038c02654d061d6dcc6"},
    {file = "requests-2.32.3.tar.gz", hash = "sha256:55365417734eb18255590a9ff9eb97e9e1da868d4ccd6402399eaf68af20a760"},
]

[package.dependencies]
certifi = ">=2017.4.17"
charset-normalizer = ">=2,<4"
idna = ">=2.5,<4"
urllib3 = ">=1.21.1,<3"

[package.extras]
socks = ["PySocks (>=1.5.6,!=1.5.7)"]
use-chardet-on-py3 = ["chardet (>=3.0.2,<6)"]

[[package]]
name = "static-frame"
version = "2.16.1"
description = "Immutable and statically-typeable DataFrames with runtime type and data validation."
optional = false
python-versions = ">=3.9"
groups = ["main"]
files = [
    {file = "static-frame-2.16.1.tar.gz", hash = "sha256:0f0e5e1c09c06891d71dca9b24e04526e99407dfae9d25d79e2011cb69c9ed9a"},
    {file = "static_frame-2.16.1-py3-none-any.whl", hash = "sha256:9f330f0672a6491bb9be0f64c64ee8abf06ffb9ee1b253f1f7695719538cc4df"},
]

[package.dependencies]
arraykit = "0.10.0"
arraymap = "0.4.0"
numpy = ">=1.23.5"
typing-extensions = ">=4.12.0"

[package.extras]
extras = ["duckdb (>=1.0.0)", "msgpack (>=1.0.4)", "msgpack-numpy (>=0.4.8)", "openpyxl (>=3.0.9)", "pandas (>=1.1.5)", "pyarrow (>=3.0.0)", "tables (>=3.9.1)", "visidata (>=2.4)", "xarray (>=0.13.0)", "xlsxwriter (>=1.1.2)"]

[[package]]
name = "typing-extensions"
version = "4.12.2"
description = "Backported and Experimental Type Hints for Python 3.8+"
optional = false
python-versions = ">=3.8"
groups = ["main"]
files = [
    {file = "typing_extensions-4.12.2-py3-none-any.whl", hash = "sha256:04e5ca0351e0f3f85c6853954072df659d0d13fac324d0072316b67d7794700d"},
    {file = "typing_extensions-4.12.2.tar.gz", hash = "sha256:1a7ead55c7e559dd4dee8856e3a88b41225abfe1ce8df57b7c13915fe121ffb8"},
]

[[package]]
name = "urllib3"
version = "2.3.0"
description = "HTTP library with thread-safe connection pooling, file post, and more."
optional = false
python-versions = ">=3.9"
groups = ["main"]
files = [
    {file = "urllib3-2.3.0-py3-none-any.whl", hash = "sha256:1cee9ad369867bfdbbb48b7dd50374c0967a0bb7710050facf0dd6911440e3df"},
    {file = "urllib3-2.3.0.tar.gz", hash = "sha256:f8c5449b3cf0861679ce7e0503c7b44b5ec981bec0d1d3795a07f1ba96f0204d"},
]

[package.extras]
brotli = ["brotli (>=1.0.9)", "brotlicffi (>=0.8.0)"]
h2 = ["h2 (>=4,<5)"]
socks = ["pysocks (>=1.5.6,!=1.5.7,<2.0)"]
zstd = ["zstandard (>=0.18.0)"]

[[package]]
name = "zipp"
version = "3.18.1"
description = "Backport of pathlib-compatible object wrapper for zip files"
optional = false
python-versions = ">=3.8"
groups = ["main"]
files = [
    {file = "zipp-3.18.1-py3-none-any.whl", hash = "sha256:206f5a15f2af3dbaee80769fb7dc6f249695e940acca08dfb2a4769fe61e538b"},
    {file = "zipp-3.18.1.tar.gz", hash = "sha256:2884ed22e7d8961de1c9a05142eb69a247f120291bc0206a00a7642f09b5b715"},
]

[package.extras]
docs = ["furo", "jaraco.packaging (>=9.3)", "jaraco.tidelift (>=1.4)", "rst.linker (>=1.9)", "sphinx (>=3.5)", "sphinx-lint"]
testing = ["big-O", "jaraco.functools", "jaraco.itertools", "more-itertools", "pytest (>=6)", "pytest-checkdocs (>=2.4)", "pytest-cov", "pytest-enabler (>=2.2)", "pytest-ignore-flaky", "pytest-mypy", "pytest-ruff (>=0.2.1)"]

[metadata]
lock-version = "2.1"
python-versions = ">=3.12"
content-hash = "88d4af2d19b75cf5d80ba6b72bbee80790fa9757747e24304c4b1c51e86f3837"
        "#;
        let lockfile = LockFile::new(poetry_contents.to_string());
        let dependencies = lockfile.get_dependencies(None).unwrap();
        assert_eq!(
            dependencies,
            vec![
                "arraykit==0.10.0",
                "arraymap==0.4.0",
                "certifi==2025.1.31",
                "charset-normalizer==3.4.1",
                "idna==3.10",
                "jinja2==3.1.3",
                "markupsafe==3.0.2",
                "numpy==2.2.2",
                "requests==2.32.3",
                "static-frame==2.16.1",
                "typing-extensions==4.12.2",
                "urllib3==2.3.0",
                "zipp==3.18.1"
            ]
        );
    }

    #[test]
    fn test_get_dependencies_pipfilelock() {
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
