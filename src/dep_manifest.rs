use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;
use toml::Value;

use crate::table::ColumnFormat;
use crate::table::Rowable;
use crate::table::RowableContext;
use crate::table::Tableable;
use crate::ureq_client::UreqClient;

use crate::dep_spec::DepSpec;
use crate::package::Package;
use crate::util::ResultDynError;

//------------------------------------------------------------------------------
pub(crate) struct DepManifestRecord {
    dep_spec: DepSpec,
}

impl Rowable for DepManifestRecord {
    fn to_rows(&self, _context: &RowableContext) -> Vec<Vec<String>> {
        vec![vec![self.dep_spec.to_string()]]
    }
}

// Simple report around dep manifest for common display/output needs
pub struct DepManifestReport {
    records: Vec<DepManifestRecord>,
}

impl Tableable<DepManifestRecord> for DepManifestReport {
    fn get_header(&self) -> Vec<ColumnFormat> {
        vec![ColumnFormat::new(
            "# via fetter".to_string(),
            false,
            "#666666".to_string(),
        )]
    }
    fn get_records(&self) -> &Vec<DepManifestRecord> {
        &self.records
    }
}

//------------------------------------------------------------------------------
// A DepManifest is a requirements listing, implemented as HashMap for quick lookup by package name.
#[derive(Debug, Clone)]
pub(crate) struct DepManifest {
    dep_specs: HashMap<String, DepSpec>,
}

impl DepManifest {
    #[allow(dead_code)]
    pub(crate) fn from_iter<I, S>(ds_iter: I) -> ResultDynError<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut dep_specs = HashMap::new();
        for line in ds_iter {
            let spec = line.as_ref().trim();
            if spec.is_empty() {
                continue;
            }
            let dep_spec = DepSpec::from_string(spec)?;
            if dep_specs.contains_key(&dep_spec.key) {
                return Err(
                    format!("Duplicate package key found: {}", dep_spec.key).into()
                );
            }
            dep_specs.insert(dep_spec.key.clone(), dep_spec);
        }
        Ok(DepManifest { dep_specs })
    }
    // Create a DepManifest from a requirements.txt file, which might reference onther requirements.txt files.
    pub(crate) fn from_requirements(file_path: &PathBuf) -> ResultDynError<Self> {
        let mut files: VecDeque<PathBuf> = VecDeque::new();
        files.push_back(file_path.clone());
        let mut dep_specs = HashMap::new();

        while files.len() > 0 {
            let fp = files.pop_front().unwrap();
            let file = File::open(&fp)
                .map_err(|e| format!("Failed to open file: {:?} {}", fp, e))?;
            let lines = io::BufReader::new(file).lines();
            for line in lines {
                if let Ok(s) = line {
                    let t = s.trim();
                    if t.is_empty() || t.starts_with('#') {
                        continue;
                    }
                    if t.starts_with("-r ") {
                        files.push_back(file_path.parent().unwrap().join(&t[3..].trim()));
                    } else if t.starts_with("--requirement ") {
                        files
                            .push_back(file_path.parent().unwrap().join(&t[14..].trim()));
                    } else {
                        let ds = DepSpec::from_string(&s)?;
                        if dep_specs.contains_key(&ds.key) {
                            return Err(format!(
                                "Duplicate package key found: {}",
                                ds.key
                            )
                            .into());
                        }
                        dep_specs.insert(ds.key.clone(), ds);
                    }
                }
            }
        }
        Ok(DepManifest { dep_specs })
    }
    pub(crate) fn from_dep_specs(dep_specs: &Vec<DepSpec>) -> ResultDynError<Self> {
        let mut ds: HashMap<String, DepSpec> = HashMap::new();
        for dep_spec in dep_specs {
            if ds.contains_key(&dep_spec.key) {
                return Err(
                    format!("Duplicate DepSpec key found: {}", dep_spec.key).into()
                );
            }
            ds.insert(dep_spec.key.clone(), dep_spec.clone());
        }
        Ok(DepManifest { dep_specs: ds })
    }

    pub(crate) fn from_pyproject(file_path: &PathBuf) -> ResultDynError<Self> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let value: Value = content
            .parse::<Value>()
            .map_err(|e| format!("Failed to parse TOML: {}", e))?;

        if let Some(dependencies) = value
            .get("project")
            .and_then(|project| project.get("dependencies"))
            .and_then(|deps| deps.as_array())
        {
            let deps_list: Vec<_> = dependencies
                .iter()
                .filter_map(|dep| dep.as_str().map(String::from))
                .collect();
            return DepManifest::from_iter(deps_list.iter());
        }

        if let Some(dependencies) = value
            .get("tool")
            .and_then(|tool| tool.get("poetry"))
            .and_then(|poetry| poetry.get("dependencies"))
            .and_then(|deps| deps.as_table())
        {
            let deps_list: Vec<_> = dependencies.keys().cloned().collect();
            return DepManifest::from_iter(deps_list.iter());
        }
        Err("Dependencies section not found in pyproject.toml".into())
    }

    // Create a DepManifest from a URL point to a requirements.txt or pyproject.toml file.
    pub(crate) fn from_url<U: UreqClient>(
        client: &U,
        url: &PathBuf,
    ) -> ResultDynError<Self> {
        let body_str = client.get(url.to_str().ok_or("Invalid URL")?)?;
        // TODO: based on url file path ending, handle txt or toml
        Self::from_iter(body_str.lines())
    }

    pub(crate) fn from_git_repo(url: &PathBuf) -> ResultDynError<Self> {
        let tmp_dir = tempdir()
            .map_err(|e| format!("Failed to create temporary directory: {}", e))?;
        let repo_path = tmp_dir.path().join("repo");

        let status = Command::new("git")
            .args(&[
                "clone",
                "--depth",
                "1",
                url.to_str().unwrap(),
                repo_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| format!("Failed to execute git: {}", e))?;

        if !status.success() {
            return Err("Git clone failed".into());
        }
        // might look for pyproject first
        let requirements_path = repo_path.join("requirements.txt");
        let manifest = DepManifest::from_requirements(&requirements_path)?;
        Ok(manifest)
    }

    //--------------------------------------------------------------------------
    fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.dep_specs.keys().cloned().collect();
        keys.sort_by_key(|name| name.to_lowercase());
        keys
    }

    // Return an optional DepSpec reference.
    pub(crate) fn get_dep_spec(&self, key: &str) -> Option<&DepSpec> {
        self.dep_specs.get(key)
    }

    // Return all DepSpec in this DepManifest that are not in observed.
    pub(crate) fn get_dep_spec_difference(
        &self,
        observed: &HashSet<&String>,
    ) -> Vec<&String> {
        // iterating over keys, collect those that are not in observed
        let mut dep_specs: Vec<&String> = self
            .dep_specs
            .keys()
            .filter(|key| !observed.contains(key))
            .collect();
        dep_specs.sort();
        dep_specs
    }

    //--------------------------------------------------------------------------
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.dep_specs.len()
    }

    pub(crate) fn validate(
        &self,
        package: &Package,
        permit_superset: bool,
    ) -> (bool, Option<&DepSpec>) {
        if let Some(ds) = self.dep_specs.get(&package.key) {
            let valid =
                ds.validate_version(&package.version) && ds.validate_url(&package);
            (valid, Some(ds))
        } else {
            (permit_superset, None) // cannot get a dep spec
        }
    }

    //--------------------------------------------------------------------------

    pub(crate) fn to_dep_manifest_report(&self) -> DepManifestReport {
        let mut records = Vec::new();
        for key in self.keys() {
            if let Some(ds) = self.dep_specs.get(&key) {
                records.push(DepManifestRecord {
                    dep_spec: ds.clone(),
                });
            }
        }
        DepManifestReport { records }
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_durl::DirectURL;
    use crate::ureq_client::UreqClientMock;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_dep_spec_a() {
        let dm =
            DepManifest::from_iter(vec!["pk1>=0.2,<0.3", "pk2>=1,<3"].iter()).unwrap();

        let p1 = Package::from_dist_info("pk2-2.0.dist-info", None, None).unwrap();
        assert_eq!(dm.validate(&p1, false).0, true);

        let p2 = Package::from_dist_info("foo-2.0.dist-info", None, None).unwrap();
        assert_eq!(dm.validate(&p2, false).0, false);

        let p3 = Package::from_dist_info("pk1-0.2.5.dist-info", None, None).unwrap();
        assert_eq!(dm.validate(&p3, false).0, true);

        let p3 = Package::from_dist_info("pk1-0.3.0.dist-info", None, None).unwrap();
        assert_eq!(dm.validate(&p3, false).0, false);
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_from_dep_specs_a() {
        let ds = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("requests>=1.4").unwrap(),
        ];
        let dm = DepManifest::from_dep_specs(&ds).unwrap();
        assert_eq!(dm.len(), 2);
    }
    #[test]
    fn test_from_requirements_a() {
        // Create a temporary directory and file
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("requirements.txt");

        // Write test content to the temp file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "# comment").unwrap();
        writeln!(file, "pk1>=0.2,  <0.3    ").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "pk2>=1,<3").unwrap();
        writeln!(file, "# ").unwrap();

        let dep_manifest = DepManifest::from_requirements(&file_path).unwrap();
        assert_eq!(dep_manifest.len(), 2);

        let p1 = Package::from_name_version_durl("pk2", "2.1", None).unwrap();
        assert_eq!(dep_manifest.validate(&p1, false).0, true);
        let p2 = Package::from_name_version_durl("pk2", "0.1", None).unwrap();
        assert_eq!(dep_manifest.validate(&p2, false).0, false);
        let p3 = Package::from_name_version_durl("pk1", "0.2.2.999", None).unwrap();
        assert_eq!(dep_manifest.validate(&p3, false).0, true);

        let p4 = Package::from_name_version_durl("pk99", "0.2.2.999", None).unwrap();
        assert_eq!(dep_manifest.validate(&p4, false).0, false);
    }

    #[test]
    fn test_from_requirements_b() {
        let content = r#"
termcolor==2.2.0
    # via
    #   invsys (pyproject.toml)
    #   apache-airflow
terminado==0.18.1
    # via notebook
testpath==0.6.0
    # via nbconvert
text-unidecode==1.3
    # via python-slugify
threadpoolctl==3.4.0
    # via scikit-learn
toml==0.10.2
    # via
    #   coverage
    #   pre-commit
tomlkit==0.12.4
    # via pylint
"#;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("requirements.txt");
        let mut file = File::create(&file_path).unwrap();
        write!(file, "{}", content).unwrap();

        let dm1 = DepManifest::from_requirements(&file_path).unwrap();
        assert_eq!(dm1.len(), 7);
        let p1 = Package::from_name_version_durl("termcolor", "2.2.0", None).unwrap();
        assert_eq!(dm1.validate(&p1, false).0, true);
        let p2 = Package::from_name_version_durl("termcolor", "2.2.1", None).unwrap();
        assert_eq!(dm1.validate(&p2, false).0, false);
        let p3 = Package::from_name_version_durl("text-unicide", "1.3", None).unwrap();
        assert_eq!(dm1.validate(&p3, false).0, false);
        let p3 = Package::from_name_version_durl("text-unidecode", "1.3", None).unwrap();
        assert_eq!(dm1.validate(&p3, false).0, true);
    }

    #[test]
    fn test_from_requirements_c() {
        let content = r#"
opentelemetry-api==1.24.0
    # via
    #   apache-airflow
    #   opentelemetry-exporter-otlp-proto-grpc
    #   opentelemetry-exporter-otlp-proto-http
    #   opentelemetry-sdk
opentelemetry-exporter-otlp==1.24.0
    # via apache-airflow
opentelemetry-exporter-otlp-proto-common==1.24.0
    # via
    #   opentelemetry-exporter-otlp-proto-grpc
    #   opentelemetry-exporter-otlp-proto-http
opentelemetry-exporter-otlp-proto-grpc==1.24.0
    # via opentelemetry-exporter-otlp
opentelemetry-exporter-otlp-proto-http==1.24.0
    # via opentelemetry-exporter-otlp
opentelemetry-proto==1.24.0
    # via
    #   opentelemetry-exporter-otlp-proto-common
    #   opentelemetry-exporter-otlp-proto-grpc
    #   opentelemetry-exporter-otlp-proto-http
opentelemetry-sdk==1.24.0
    # via
    #   opentelemetry-exporter-otlp-proto-grpc
    #   opentelemetry-exporter-otlp-proto-http
opentelemetry-semantic-conventions==0.45b0
    # via opentelemetry-sdk
"#;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("requirements.txt");
        let mut file = File::create(&file_path).unwrap();
        write!(file, "{}", content).unwrap();

        let dm1 = DepManifest::from_requirements(&file_path).unwrap();
        assert_eq!(dm1.len(), 8);
        let p1 = Package::from_name_version_durl(
            "opentelemetry-exporter-otlp-proto-grpc",
            "1.24.0",
            None,
        )
        .unwrap();
        assert_eq!(dm1.validate(&p1, false).0, true);
        let p2 = Package::from_name_version_durl(
            "opentelemetry-exporter-otlp-proto-grpc",
            "1.24.1",
            None,
        )
        .unwrap();
        assert_eq!(dm1.validate(&p2, false).0, false);
        let p3 = Package::from_name_version_durl(
            "opentelemetry-exporter-otlp-proto-gpc",
            "1.24.0",
            None,
        )
        .unwrap();
        assert_eq!(dm1.validate(&p3, false).0, false);
    }

    #[test]
    fn test_from_requirements_d() {
        let content = r#"
python-slugify==8.0.4
    # via
    #   apache-airflow
    #   python-nvd3
pytz==2023.3
pytzdata==2020.1
    # via pendulum
pyyaml==6.0
pyzmq==26.0.0
readme-renderer==43.0
    # via twine
redshift-connector==2.1.1
    # via apache-airflow-providers-amazon
referencing==0.34.0
    # via
    #   jsonschema
    #   jsonschema-specifications
regex==2024.4.16
"#;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("requirements.txt");
        let mut file = File::create(&file_path).unwrap();
        write!(file, "{}", content).unwrap();

        let dm1 = DepManifest::from_requirements(&file_path).unwrap();
        assert_eq!(dm1.len(), 9);
        let p1 = Package::from_name_version_durl("regex", "2024.4.16", None).unwrap();
        assert_eq!(dm1.validate(&p1, false).0, true);
        let p2 = Package::from_name_version_durl("regex", "2024.04.16", None).unwrap();
        assert_eq!(dm1.validate(&p2, false).0, true);
        let p2 = Package::from_name_version_durl("regex", "2024.04.17", None).unwrap();
        assert_eq!(dm1.validate(&p2, false).0, false);
    }

    #[test]
    fn test_from_requirements_e() {
        let content1 = r#"
python-slugify==8.0.4
pytz==2023.3
pytzdata==2020.1
pyyaml==6.0
pyzmq==26.0.0
"#;
        let dir = tempdir().unwrap();
        let fp1 = dir.path().join("requirements-a.txt");
        let mut f1 = File::create(&fp1).unwrap();
        write!(f1, "{}", content1).unwrap();

        let content2 = r#"
readme-renderer==43.0
redshift-connector==2.1.1
referencing==0.34.0
regex==2024.4.16
-r requirements-a.txt
"#;
        let fp2 = dir.path().join("requirements-b.txt");
        let mut f2 = File::create(&fp2).unwrap();
        write!(f2, "{}", content2).unwrap();

        let dm1 = DepManifest::from_requirements(&fp2).unwrap();
        assert_eq!(dm1.len(), 9);
    }

    #[test]
    fn test_from_requirements_f() {
        let content1 = r#"
python-slugify==8.0.4
pytz==2023.3
pytzdata==2020.1
pyyaml==6.0
pyzmq==26.0.0
"#;
        let dir = tempdir().unwrap();
        let fp1 = dir.path().join("requirements-a.txt");
        let mut f1 = File::create(&fp1).unwrap();
        write!(f1, "{}", content1).unwrap();

        let content2 = r#"
readme-renderer==43.0
redshift-connector==2.1.1
--requirement  requirements-a.txt
"#;
        let fp2 = dir.path().join("requirements-b.txt");
        let mut f2 = File::create(&fp2).unwrap();
        write!(f2, "{}", content2).unwrap();

        let content3 = r#"
referencing==0.34.0
regex==2024.4.16
--requirement    requirements-b.txt
"#;
        let fp3 = dir.path().join("requirements-c.txt");
        let mut f3 = File::create(&fp3).unwrap();
        write!(f3, "{}", content3).unwrap();

        let dm1 = DepManifest::from_requirements(&fp3).unwrap();
        assert_eq!(dm1.len(), 9);
    }

    //--------------------------------------------------------------------------

    #[test]
    fn test_from_pyproject_a() {
        let content = r#"
[project]
name = "example_package_YOUR_USERNAME_HERE"
version = "0.0.1"
authors = [
  { name="Example Author", email="author@example.com" },
]
description = "A small example package"
readme = "README.md"
requires-python = ">=3.8"
classifiers = [
    "Programming Language :: Python :: 3",
    "License :: OSI Approved :: MIT License",
    "Operating System :: OS Independent",
]
dependencies = [
  "httpx",
  "gidgethub[httpx]>4.0.0",
  "django>2.1; os_name != 'nt'",
]
"#;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("pyproject.toml");
        let mut file = File::create(&file_path).unwrap();
        write!(file, "{}", content).unwrap();

        let dm = DepManifest::from_pyproject(&file_path).unwrap();
        assert_eq!(dm.keys(), vec!["django", "gidgethub", "httpx"])
    }

    //--------------------------------------------------------------------------

    #[test]
    fn test_from_url_a() {
        let mock_get = r#"
dill>=0.3.9
six>=1.15.0
numpy>= 2.0
        "#;

        let client = UreqClientMock {
            mock_post: None,
            mock_get: Some(mock_get.to_string()),
        };

        let url = PathBuf::from("http://example.com/requirements.txt");
        let dm = DepManifest::from_url(&client, &url).unwrap();
        assert_eq!(dm.keys(), vec!["dill", "numpy", "six"])
    }

    //--------------------------------------------------------------------------

    #[test]
    fn test_to_requirements_a() {
        let ds = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("requests>=1.4").unwrap(),
            DepSpec::from_string("static-frame>2.0,!=1.3").unwrap(),
        ];
        let dm1 = DepManifest::from_dep_specs(&ds).unwrap();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("requirements.txt");
        let dmr1 = dm1.to_dep_manifest_report();
        dmr1.to_file(&file_path, ' ').unwrap();

        let dm2 = DepManifest::from_requirements(&file_path).unwrap();
        assert_eq!(dm2.len(), 3)
    }

    //--------------------------------------------------------------------------

    #[test]
    fn test_get_dep_spec_a() {
        let ds = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("requests>=1.4").unwrap(),
            DepSpec::from_string("static-frame>2.0,!=1.3").unwrap(),
        ];
        let dm1 = DepManifest::from_dep_specs(&ds).unwrap();
        let ds1 = dm1.get_dep_spec("requests").unwrap();
        assert_eq!(format!("{}", ds1), "requests>=1.4");
    }

    #[test]
    fn test_get_dep_spec_b() {
        let ds = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("requests>=1.4").unwrap(),
            DepSpec::from_string("static-frame>2.0,!=1.3").unwrap(),
        ];
        let dm1 = DepManifest::from_dep_specs(&ds).unwrap();
        assert!(dm1.get_dep_spec("foo").is_none());
    }

    #[test]
    fn test_get_dep_spec_c() {
        let ds = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("Cython==3.0.11").unwrap(),
        ];
        let dm1 = DepManifest::from_dep_specs(&ds).unwrap();
        let ds1 = dm1.get_dep_spec("cython").unwrap();
        assert_eq!(format!("{}", ds1), "Cython==3.0.11");
    }

    //--------------------------------------------------------------------------

    #[test]
    fn test_get_dep_spec_difference_a() {
        let ds = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("requests>=1.4").unwrap(),
            DepSpec::from_string("static-frame>2.0,!=1.3").unwrap(),
        ];
        let dm1 = DepManifest::from_dep_specs(&ds).unwrap();
        let mut observed = HashSet::new();
        let n1 = "static_frame".to_string();
        observed.insert(&n1);

        let post = dm1.get_dep_spec_difference(&observed);

        assert_eq!(post, vec!["numpy", "requests"]);
    }

    //--------------------------------------------------------------------------

    #[test]
    fn test_validate_a() {
        // if we install as "packaging @ git+https://github.com/pypa/packaging.git@cf2cbe2aec28f87c6228a6fb136c27931c9af407"
        // in site packages we get packaging-24.2.dev0.dist-info
        // and writes this in direct_url.json
        // {"url": "https://github.com/pypa/packaging.git", "vcs_info": {"commit_id": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "requested_revision": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "vcs": "git"}}

        let json_str = r#"
        {"url": "https://github.com/pypa/packaging.git", "vcs_info": {"commit_id": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "requested_revision": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "vcs": "git"}}
        "#;
        let durl: DirectURL = serde_json::from_str(json_str).unwrap();
        let p1 =
            Package::from_dist_info("packaging-24.2.dev0.dist-info", None, Some(durl))
                .unwrap();

        let ds1 = DepSpec::from_string("packaging @ git+https://github.com/pypa/packaging.git@cf2cbe2aec28f87c6228a6fb136c27931c9af407").unwrap();
        let specs = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("requests>=1.4").unwrap(),
            ds1,
        ];
        let dm1 = DepManifest::from_dep_specs(&specs).unwrap();
        // ds1 has no version information, while p1 does: meaning version passes
        // ds1 has url of git+https://github.com/pypa/packaging.git@cf2cbe2aec28f87c6228a6fb136c27931c9af407
        // DirectURL: git+https://github.com/pypa/packaging.git@cf2cbe2aec28f87c6228a6fb136c27931c9af407
        assert_eq!(dm1.validate(&p1, false).0, true);
    }

    #[test]
    fn test_validate_b() {
        // if we install as "packaging @ git+https://foo@github.com/pypa/packaging.git@cf2cbe2aec28f87c6228a6fb136c27931c9af407"
        // in site packages we get packaging-24.2.dev0.dist-info
        // and writes this in direct_url.json, without the user part of the url
        // {"url": "https://github.com/pypa/packaging.git", "vcs_info": {"commit_id": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "requested_revision": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "vcs": "git"}}

        let json_str = r#"
        {"url": "https://github.com/pypa/packaging.git", "vcs_info": {"commit_id": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "requested_revision": "cf2cbe2aec28f87c6228a6fb136c27931c9af407", "vcs": "git"}}
        "#;
        let durl: DirectURL = serde_json::from_str(json_str).unwrap();
        let p1 =
            Package::from_dist_info("packaging-24.2.dev0.dist-info", None, Some(durl))
                .unwrap();

        let ds1 = DepSpec::from_string("packaging @ git+https://foo@github.com/pypa/packaging.git@cf2cbe2aec28f87c6228a6fb136c27931c9af407").unwrap();
        let specs = vec![
            DepSpec::from_string("numpy==1.19.1").unwrap(),
            DepSpec::from_string("requests>=1.4").unwrap(),
            ds1,
        ];
        let dm1 = DepManifest::from_dep_specs(&specs).unwrap();
        assert_eq!(dm1.validate(&p1, false).0, true);
    }

    //--------------------------------------------------------------------------
}
