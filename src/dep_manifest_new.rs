use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;

use crate::table::ColumnFormat;
use crate::table::Rowable;
use crate::table::RowableContext;
use crate::table::Tableable;
use crate::ureq_client::UreqClient;

use crate::dep_spec::DepSpec;
use crate::lock_file::LockFile;
use crate::package::Package;
use crate::pyproject::PyProjectInfo;
use crate::ureq_client::UreqClientLive;
use crate::util::path_normalize;
use crate::util::ResultDynError;

//------------------------------------------------------------------------------
static LOCK_PRIORITY: &[&str] = &[
    "uv.lock",
    "poetry.lock",
    "Pipfile.lock",
    "requirements.lock",
    "requirements.txt",
    "pyproject.toml",
];

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


type
//------------------------------------------------------------------------------
// A DepManifest is a pool of dependencies that can be allocated into Python exe specific groups based on environment markers. Inner depdencies are stored as HashMap for quick lookup by package name.
#[derive(Debug, Clone)]
pub(crate) struct DepManifest {
    // dep_specs: HashMap<String, DepSpec>,
    dep_spec_pool: Vec<DepSpec>
    // mapping from exe to dependency map
    exe_dep_specs: HashMap<PathBuf, HashMap<String, &DepSpec>>
}

impl DepManifest {
    //--------------------------------------------------------------------------
    // constructors from internal structs

    pub(crate) fn from_iter<I, S>(ds_iter: I) -> ResultDynError<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        // let mut dep_specs = HashMap::new();
        let mut dep_spec_pool = Vec::new();
        for line in ds_iter {
            let spec = line.as_ref().trim();
            if spec.is_empty() {
                continue;
            }
            let dep_spec = DepSpec::from_string(spec)?;
            // if dep_specs.contains_key(&dep_spec.key) {
            //     return Err(
            //         format!("Duplicate package key found: {}", dep_spec.key).into()
            //     );
            // }
            // dep_specs.insert(dep_spec.key.clone(), dep_spec);
            dep_spec_pool.push(dep_spec);
        }
        let exe_dep_specs: HashMap<PathBuf, HashMap<String, &DepSpec>> = HashMap::new();
        Ok(DepManifest { dep_spec_pool, exe_dep_specs })
    }

    pub(crate) fn from_dep_specs(dep_specs: &Vec<DepSpec>) -> ResultDynError<Self> {
        // TODO: replace with a set, make DepSpec hashable
        let mut ds: HashMap<String, DepSpec> = HashMap::new();
        let mut dep_spec_pool = Vec::new();

        for dep_spec in dep_specs {
            if let Some(dep_spec_prev) = ds.remove(&dep_spec.key) {
                // remove and replace with composite
                let dep_spec_new =
                    DepSpec::from_dep_specs(vec![&dep_spec_prev, &dep_spec])?;
                // ds.insert(dep_spec_new.key.clone(), dep_spec_new);
                dep_spec_pool.push(dep_spec_new);
            } else {
                ds.insert(dep_spec.key.clone(), dep_spec.clone());
                dep_spec_pool.push(dep_spec);
            }
        }
        // Ok(DepManifest { dep_specs: ds })
        let exe_dep_specs: HashMap<PathBuf, HashMap<String, &DepSpec>> = HashMap::new();
        Ok(DepManifest { dep_spec_pool, exe_dep_specs })

    }

    //--------------------------------------------------------------------------

    // Create a DepManifest from a requirements.txt file, which might reference other requirements.txt files.
    pub(crate) fn from_requirements_file(file_path: &Path) -> ResultDynError<Self> {
        let mut files: VecDeque<PathBuf> = VecDeque::new();
        files.push_back(file_path.to_path_buf());
        // let mut dep_specs = HashMap::new();
        let mut dep_spec_pool = Vec::new();

        while !files.is_empty() {
            let fp = files.pop_front().unwrap();
            let file = File::open(&fp)
                .map_err(|e| format!("Failed to open file: {:?} {}", fp, e))?;
            let lines = io::BufReader::new(file).lines();
            for line in lines.map_while(Result::ok) {
                let t = line.trim();
                if t.is_empty() || t.starts_with('#') {
                    continue;
                }
                if let Some(post) = t.strip_prefix("-r ") {
                    files.push_back(file_path.parent().unwrap().join(post.trim()));
                } else if let Some(post) = t.strip_prefix("--requirement ") {
                    files.push_back(file_path.parent().unwrap().join(post.trim()));
                } else {
                    let ds = DepSpec::from_string(&line)?;
                    // if dep_specs.contains_key(&ds.key) {
                    //     return Err(
                    //         format!("Duplicate package key found: {}", ds.key).into()
                    //     );
                    // }
                    // dep_specs.insert(ds.key.clone(), ds);
                    dep_spec_pool.push(ds);
                }
            }
        }
        // Ok(DepManifest { dep_specs })
        let exe_dep_specs: HashMap<PathBuf, HashMap<String, &DepSpec>> = HashMap::new();
        Ok(DepManifest { dep_spec_pool, exe_dep_specs })

    }

    pub(crate) fn from_pyproject(
        content: &str,
        options: Option<&Vec<String>>,
    ) -> ResultDynError<Self> {
        let ppi = PyProjectInfo::new(content)?;
        Self::from_iter(ppi.get_dependencies(options)?.iter())
    }

    pub(crate) fn from_pyproject_file(
        file_path: &PathBuf,
        bound_options: Option<&Vec<String>>,
    ) -> ResultDynError<Self> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        Self::from_pyproject(&content, bound_options)
    }

    // Create a DepManifest from a URL point to a requirements.txt or pyproject.toml file.
    pub(crate) fn from_url<U: UreqClient>(
        client: &U,
        url: &Path,
        bound_options: Option<&Vec<String>>,
    ) -> ResultDynError<Self> {
        let url_str = url.to_str().ok_or("Invalid URL")?;
        let content = client.get(url_str)?;
        if url_str.ends_with("pyproject.toml") {
            Self::from_pyproject(&content, bound_options)
        } else {
            // handle any lock file format, or requirements.txt
            let lf = LockFile::new(content);
            Self::from_iter(lf.get_dependencies(bound_options)?)
        }
    }

    //--------------------------------------------------------------------------
    pub(crate) fn from_path(
        file_path: &Path,
        bound_options: Option<&Vec<String>>,
    ) -> ResultDynError<Self> {
        let fp = path_normalize(file_path, true)?;
        match fp.to_str() {
            Some(s) if s.ends_with("pyproject.toml") => {
                Self::from_pyproject_file(&fp, bound_options)
            }
            Some(s) if s.ends_with("requirements.txt") => {
                Self::from_requirements_file(&fp)
            }
            Some(_) => {
                let content = fs::read_to_string(fp)
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                // handle uv.lock, poetry.lock, requirements.lock, Pipfile.lock, or a requirements.txt format (via uv or pip-compile)
                let lf = LockFile::new(content);
                Self::from_iter(lf.get_dependencies(bound_options)?)
            }
            None => Err("Path contains invalid UTF-8".into()),
        }
    }

    /// Given a directory, load the first canddiate file based on LOCK_PRIORITY.
    pub(crate) fn from_dir(
        dir: &Path,
        bound_options: Option<&Vec<String>>,
    ) -> ResultDynError<Self> {
        match LOCK_PRIORITY
            .iter()
            .map(|file| dir.join(file))
            .find(|path| path.exists())
        {
            Some(file_path) => Self::from_path(&file_path, bound_options),
            None => {
                Err("Cannot find lock file, requirements file, or pyproject.toml".into())
            }
        }
    }

    pub(crate) fn from_git_repo(
        url: &Path,
        bound_options: Option<&Vec<String>>,
    ) -> ResultDynError<Self> {
        let tmp_dir = tempdir()
            .map_err(|e| format!("Failed to create temporary directory: {}", e))?;
        let repo_path = tmp_dir.path().join("repo");

        let status = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                url.to_str().unwrap(),
                repo_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| format!("Failed to execute git: {}", e))?;

        if !status.success() {
            return Err(format!("Git clone failed: {}", url.display()).into());
        }
        Self::from_dir(&repo_path, bound_options)
    }

    pub(crate) fn from_path_or_url(
        file_path: &Path,
        bound_options: Option<&Vec<String>>,
    ) -> ResultDynError<Self> {
        match file_path.to_str() {
            Some(s) if s.ends_with(".git") => {
                Self::from_git_repo(file_path, bound_options)
            }
            Some(s) if s.starts_with("http") => {
                Self::from_url(&UreqClientLive, file_path, bound_options)
            }
            Some(_) => Self::from_path(file_path, bound_options),
            None => Err("Path contains invalid UTF-8".into()),
        }
    }

    //--------------------------------------------------------------------------
    fn load_exe_dep_specs(%self, exe: Option<PathBuf>) {
        let key = match exe {
            Some(e) => e.to_str();
            None => "".to_string();
        }
        if !self.exe_dep_specs.contains_key(&key) {
            dep_specs = HashMap::new();
            for ds in dep_specs {

            }
        }
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
            let valid = ds.validate_package(package);
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
