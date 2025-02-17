use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use rayon::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::audit_report::AuditReport;
use crate::count_report::CountReport;
use crate::dep_manifest::DepManifest;
use crate::dep_spec::DepOperator;
use crate::dep_spec::DepSpec;
use crate::env_marker::EnvMarkerState;
use crate::exe_search::find_exe;
use crate::package::Package;
use crate::package_match::match_str;
use crate::path_shared::PathShared;
use crate::scan_report::ScanReport;
use crate::site_customize::install_validation;
use crate::site_customize::uninstall_validation;
use crate::unpack_report::UnpackReport;
use crate::ureq_client::UreqClientLive;
use crate::util::exe_path_normalize;
use crate::util::hash_paths;
use crate::util::logger;
use crate::util::path_cache;
use crate::util::path_is_component;
use crate::util::path_normalize;
use crate::util::path_within_duration;
use crate::util::ResultDynError;
use crate::util::DURATION_0;
use crate::validation_report::ValidationFlags;
use crate::validation_report::ValidationRecord;
use crate::validation_report::ValidationReport;
//------------------------------------------------------------------------------
#[derive(Debug, Copy, Clone)]
pub(crate) enum Anchor {
    Lower,
    Upper,
    Both,
}

//------------------------------------------------------------------------------

/// Given a path to a Python binary, call out to Python to get all known site packages; some site packages may not exist; we do not filter them here. This will include "dist-packages" on Linux. If `force_usite` is false, we use site.ENABLE_USER_SITE to determine if we should include the user site packages; if `force_usite` is true, we always include usite.
/// Calling Python using `-S` disables loading site so that we can mock sitecustomize.py (which fetter might customize). We then call `site.main()` to force proper initialization.
const PY_SITE_PACKAGES: &str = "import sys;import site;import types;sys.modules['fetter_validate'] = types.ModuleType('fetter_validate');site.main();print(site.ENABLE_USER_SITE);print(\"\\n\".join(site.getsitepackages()));print(site.getusersitepackages())";
fn get_site_package_dirs(
    executable: &Path,
    force_usite: bool,
    log: bool,
) -> Vec<PathShared> {
    match Command::new(executable)
        .arg("-S") // disable site on startup
        .arg("-c")
        .arg(PY_SITE_PACKAGES)
        .output()
    {
        Ok(output) => {
            let mut paths = Vec::new();
            let mut usite_enabled = false;

            let lines = std::str::from_utf8(&output.stdout)
                .expect("Failed to convert to UTF-8")
                .trim()
                .lines();
            for (i, line) in lines.enumerate() {
                if i == 0 {
                    usite_enabled = line.trim() == "True";
                } else {
                    paths.push(PathShared::from_str(line.trim()));
                }
            }
            // if necessary, remove the usite
            if !force_usite && !usite_enabled {
                let _p = paths.pop();
            }
            paths
        }
        Err(e) => {
            if log {
                logger!(
                    module_path!(),
                    "Failed to execute command with {:?}: {}",
                    executable,
                    e
                );
            }
            Vec::with_capacity(0)
        }
    }
}

// Given a package directory, collect the name of all packages.
fn get_packages(site_packages: &Path) -> Vec<Package> {
    let mut packages = Vec::new();
    if let Ok(entries) = fs::read_dir(site_packages) {
        for entry in entries.flatten() {
            let file_path = entry.path();
            if let Some(package) = Package::from_file_path(&file_path) {
                packages.push(package);
            }
        }
    }
    packages
}

//------------------------------------------------------------------------------

// The result of a file-system scan.
#[derive(Clone, Debug)]
pub(crate) struct ScanFS {
    // NOTE: these attributes are used by reporters
    /// A mapping of exe path to site packages paths
    pub(crate) exe_to_sites: HashMap<PathBuf, Vec<PathShared>>,
    /// A mapping of Package tp a site package paths
    pub(crate) package_to_sites: HashMap<Package, Vec<PathShared>>,
    // A mapping of site package to exe paths
    pub(crate) site_to_exe: HashMap<PathShared, PathBuf>,

    /// Optionally populate EnvMarkerState for all exe, only if env markers are found
    pub(crate) exe_to_ems: Option<HashMap<PathBuf, EnvMarkerState>>,
    /// Optionally force usage of user site
    force_usite: bool,
    /// Store the hash of the un-normalized exe inputs for cache lookup.
    exes_hash: String,
}

impl Serialize for ScanFS {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Collect and sort by keys for stable ordering
        let mut exe_to_sites: Vec<_> = self.exe_to_sites.iter().collect();
        exe_to_sites.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        let mut package_to_sites: Vec<_> = self.package_to_sites.iter().collect();
        package_to_sites.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        let site_to_exe: Vec<_> = self.site_to_exe.iter().collect();
        // site_to_exe.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        // Serialize as tuple of sorted vectors
        let data = (
            &exe_to_sites,
            &package_to_sites,
            &site_to_exe,
            self.force_usite,
            &self.exes_hash,
        );
        data.serialize(serializer)
    }
}

/// Flattened data representation used for serialization.
type ScanFSData = (
    Vec<(PathBuf, Vec<PathShared>)>,
    Vec<(Package, Vec<PathShared>)>,
    Vec<(PathShared, PathBuf)>,
    bool,   // force_usite
    String, // exes hash
);

impl<'de> Deserialize<'de> for ScanFS {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (exe_to_sites, package_to_sites, site_to_exe, force_usite, exes_hash): ScanFSData =
            Deserialize::deserialize(deserializer)?;

        let exe_to_sites = exe_to_sites.into_iter().collect();
        let site_to_exe = site_to_exe.into_iter().collect();
        let package_to_sites = package_to_sites.into_iter().collect();

        Ok(ScanFS {
            exe_to_sites,
            package_to_sites,
            site_to_exe,
            exe_to_ems: None,
            force_usite,
            exes_hash,
        })
    }
}

impl ScanFS {
    /// Main entry point for creating a ScanFS. All public creation should go through this interface.
    fn from_exe_to_sites(
        exe_to_sites: HashMap<PathBuf, Vec<PathShared>>,
        force_usite: bool,
        exes_hash: String,
    ) -> ResultDynError<Self> {
        // Some site packages will be repeated; let them be processed more than once here, as it seems easier than filtering them out
        let site_to_packages = exe_to_sites
            .par_iter()
            .flat_map(|(_, site_packages)| {
                site_packages.par_iter().map(|site_package_path| {
                    let packages = get_packages(site_package_path.as_path());
                    (site_package_path.clone(), packages)
                })
            })
            .collect::<HashMap<PathShared, Vec<Package>>>();

        let site_to_exe: HashMap<PathShared, PathBuf> = exe_to_sites
            .iter()
            .flat_map(|(exe, sites)| sites.iter().map(|site| (site.clone(), exe.clone())))
            .collect();

        let mut package_to_sites: HashMap<Package, Vec<PathShared>> = HashMap::new();
        for (site_package_path, packages) in site_to_packages.iter() {
            for package in packages {
                package_to_sites
                    .entry(package.clone())
                    .or_default()
                    .push(site_package_path.clone());
            }
        }
        Ok(ScanFS {
            exe_to_sites,
            package_to_sites,
            site_to_exe,
            exe_to_ems: None,
            force_usite,
            exes_hash,
        })
    }

    /// Create a ScanFS from a cache: exes provided here should be pre-normalization.
    pub(crate) fn from_cache(
        exes: &[PathBuf],
        force_usite: bool,
        cache_dur: Duration,
        log: bool,
    ) -> ResultDynError<Self> {
        if cache_dur == DURATION_0 {
            Err("Cache disabled by duration".into())
        } else if let Some(mut cache_dir) = path_cache(true) {
            let exes_hash = hash_paths(exes, force_usite);
            cache_dir.push(exes_hash);
            let cache_fp = cache_dir.with_extension("json");

            if path_within_duration(&cache_fp, cache_dur) {
                if log {
                    logger!(module_path!(), "Loading cache: {:?}", cache_fp);
                }
                let mut file = File::open(cache_fp)?;
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let data: ScanFS = serde_json::from_str(&contents)?;
                Ok(data)
            } else if cache_fp.exists() {
                Err("Cache expired".into())
            } else {
                Err("Cache file does not exist".into())
            }
        } else {
            Err("Could not get cache directory".into())
        }
    }

    /// Given a Vec of PathBuf to executables, use them to collect site packages. In this function, provided PathBuf are normalized to absolute paths, and if a PathBuf is "*", a system-wide path search will be conducted.
    pub(crate) fn from_exes(
        exes: &Vec<PathBuf>,
        force_usite: bool,
        log: bool,
    ) -> ResultDynError<Self> {
        let path_wild = PathBuf::from("*");
        let mut exes_norm = Vec::new();
        for e in exes {
            if path_is_component(e) && *e == path_wild {
                exes_norm.extend(find_exe());
            } else {
                exes_norm.push(exe_path_normalize(e)?);
            }
        }

        let exe_to_sites: HashMap<PathBuf, Vec<PathShared>> = exes_norm
            .into_par_iter()
            .map(|exe| {
                let dirs = get_site_package_dirs(&exe, force_usite, log);
                (exe, dirs)
            })
            .collect();

        let exes_hash = hash_paths(exes, force_usite);
        Self::from_exe_to_sites(exe_to_sites, force_usite, exes_hash)
    }

    /// Alternative constructor from in-memory objects, only for testing. Here we provide notional exe and site paths, and focus just on collecting Packages.
    #[allow(dead_code)]
    pub(crate) fn from_exe_site_packages(
        exe: PathBuf,
        site: PathBuf,
        packages: Vec<Package>,
    ) -> ResultDynError<Self> {
        let mut exe_to_sites = HashMap::new();
        let site_shared = PathShared::from_path_buf(site);

        exe_to_sites.insert(exe.clone(), vec![site_shared.clone()]);
        let exes = vec![exe];

        let site_to_exe: HashMap<PathShared, PathBuf> = exe_to_sites
            .iter()
            .flat_map(|(exe, sites)| sites.iter().map(|site| (site.clone(), exe.clone())))
            .collect();

        let mut package_to_sites = HashMap::new();
        for package in packages {
            package_to_sites
                .entry(package)
                .or_insert_with(Vec::new)
                .push(site_shared.clone());
        }
        let force_usite = false;
        let exes_hash = hash_paths(&exes, force_usite);
        Ok(ScanFS {
            exe_to_sites,
            package_to_sites,
            site_to_exe,
            exe_to_ems: None,
            force_usite,
            exes_hash,
        })
    }

    //--------------------------------------------------------------------------

    // If not set, optionally load EnvMarkerState for each exe
    pub(crate) fn load_env_marker_state(&mut self) -> ResultDynError<()> {
        if self.exe_to_ems.is_none() {
            // TODO: multi-thread
            let mut ems_map = HashMap::new();
            for exe in self.exe_to_sites.keys() {
                ems_map.insert(exe.clone(), EnvMarkerState::from_exe(exe)?);
            }
            self.exe_to_ems = Some(ems_map);
        }
        Ok(())
    }

    // searching
    pub(crate) fn search_by_match(
        &self,
        pattern: &str,
        case_insensitive: bool,
    ) -> Vec<Package> {
        // take ownership of Package in the result of get_packages
        let matched = self
            .get_packages()
            .into_par_iter()
            .filter(|package| {
                match_str(pattern, package.to_string().as_str(), case_insensitive)
            })
            .collect();
        matched
    }

    /// Return sorted packages.
    pub(crate) fn get_packages(&self) -> Vec<Package> {
        let mut packages: Vec<Package> = self.package_to_sites.keys().cloned().collect();
        packages.sort();
        packages
    }

    //--------------------------------------------------------------------------

    pub(crate) fn to_cache(&self, cache_dur: Duration, log: bool) -> ResultDynError<()> {
        if let Some(mut cache_dir) = path_cache(true) {
            // use hash of exes observed at initialization
            cache_dir.push(self.exes_hash.clone());
            let cache_fp = cache_dir.with_extension("json");

            // only write if cache does not exist or it is out of duration
            if !cache_fp.exists() || !path_within_duration(&cache_fp, cache_dur) {
                if log {
                    logger!(module_path!(), "Writing cache: {:?}", cache_fp);
                }
                let json = serde_json::to_string(self)?;
                let mut file = File::create(cache_fp)?;
                file.write_all(json.as_bytes())?;
                return Ok(());
            } else {
                if log {
                    logger!(module_path!(), "Keeping existing cache {:?}", cache_fp);
                }
                return Ok(());
            }
        }
        Err("could not get cache directory".into())
    }

    //--------------------------------------------------------------------------

    /// Validate this scan against the provided DepManifest.
    pub(crate) fn to_validation_report(
        &self,
        dm: DepManifest,
        vf: ValidationFlags,
    ) -> ValidationReport {
        let mut records: Vec<ValidationRecord> = Vec::new();
        let mut ds_keys_matched: HashSet<&String> = HashSet::new();

        // iterate over found packages in order for better reporting
        for package in self.get_packages() {
            // For each package, if the DepManifest has env_marker_active, we have to get the EnvMarkerState for the python exe from which this Package came. That means that we hae to get each site package associated with this Package, git the exe for each site package, and then determine if the dependency remains after filtering EnvMarkerState
            let (valid, ds) = dm.validate(&package, vf.permit_superset);
            if let Some(ds) = ds {
                ds_keys_matched.insert(&ds.key);
            }
            if !valid {
                // package should always have defined sites
                let sites = self.package_to_sites.get(&package).cloned();
                // ds is an Option type, might be None
                records.push(ValidationRecord::new(
                    Some(package), // can take ownership of Package
                    ds.cloned(),
                    sites,
                ));
            }
        }
        if !vf.permit_subset {
            // packages defined in DepSpec but not found
            // NOTE: this is sorted, but not sorted with the other records
            for key in dm.get_dep_spec_difference(&ds_keys_matched) {
                records.push(ValidationRecord::new(
                    None,
                    dm.get_dep_spec(key).cloned(),
                    None,
                ));
            }
        }
        ValidationReport { records }
    }

    pub(crate) fn to_audit_report(
        &self,
        pattern: &str,
        case_insensitive: bool,
    ) -> AuditReport {
        let packages = self.search_by_match(pattern, case_insensitive);
        AuditReport::from_packages(&UreqClientLive, &packages)
    }

    /// The `count` Boolean determine if what type of UnpackReport is returned
    pub(crate) fn to_unpack_report(
        &self,
        pattern: &str,
        case_insensitive: bool,
        count: bool,
    ) -> UnpackReport {
        let mut packages = self.search_by_match(pattern, case_insensitive);
        packages.sort();
        let package_to_sites = packages
            .iter()
            .map(|p| (p.clone(), self.package_to_sites.get(p).unwrap().clone()))
            .collect();

        UnpackReport::from_package_to_sites(count, &package_to_sites)
    }

    /// Given an `anchor`, produce a DepManifest based ont the packages observed in this scan.
    pub(crate) fn to_dep_manifest(
        &self,
        anchor: Anchor,
    ) -> Result<DepManifest, Box<dyn std::error::Error>> {
        let mut package_name_to_package: HashMap<String, Vec<Package>> = HashMap::new();

        for package in self.package_to_sites.keys() {
            package_name_to_package
                .entry(package.name.clone())
                .or_default()
                .push(package.clone());
        }
        let names: Vec<String> = package_name_to_package.keys().cloned().collect();
        let mut dep_specs: Vec<DepSpec> = Vec::new();
        for name in names {
            let packages = match package_name_to_package.get_mut(&name) {
                Some(packages) => packages,
                None => continue,
            };
            packages.sort();

            let pkg_min = match packages.first() {
                Some(pkg) => pkg,
                None => continue,
            };
            let pkg_max = match packages.last() {
                Some(pkg) => pkg,
                None => continue,
            };

            let ds = match anchor {
                Anchor::Lower => {
                    DepSpec::from_package(pkg_min, DepOperator::GreaterThanOrEq)
                }
                Anchor::Upper => {
                    DepSpec::from_package(pkg_max, DepOperator::LessThanOrEq)
                }
                Anchor::Both => return Err("Not implemented".into()),
            };
            if let Ok(dep_spec) = ds {
                dep_specs.push(dep_spec);
            }
        }
        DepManifest::from_dep_specs(&dep_specs)
    }

    pub(crate) fn to_scan_report(&self) -> ScanReport {
        ScanReport::from_package_to_sites(&self.package_to_sites)
    }

    pub(crate) fn to_count_report(&self) -> CountReport {
        CountReport::from_scan_fs(self)
    }

    pub(crate) fn to_search_report(
        &self,
        pattern: &str,
        case_insensitive: bool,
    ) -> ScanReport {
        let packages = self.search_by_match(pattern, case_insensitive);
        // println!("packages: {:?}", packages);
        ScanReport::from_packages(&packages, &self.package_to_sites)
    }

    pub(crate) fn to_purge_pattern(
        &self,
        pattern: &Option<String>,
        case_insensitive: bool,
        log: bool,
    ) -> io::Result<()> {
        let packages = match pattern {
            Some(p) => self.search_by_match(p, case_insensitive),
            None => self.package_to_sites.keys().cloned().collect(),
        };
        // packages.sort();
        let package_to_sites = packages
            .iter()
            .map(|p| (p.clone(), self.package_to_sites.get(p).unwrap().clone()))
            .collect();

        let sr = UnpackReport::from_package_to_sites(false, &package_to_sites);
        sr.remove(log)
    }

    pub(crate) fn to_purge_invalid(
        &self,
        dm: DepManifest,
        vf: ValidationFlags,
        log: bool,
    ) -> io::Result<()> {
        let vr = self.to_validation_report(dm, vf);
        let packages: Vec<Package> = vr
            .records
            .iter()
            .filter_map(|r| r.package.as_ref().cloned())
            .collect();
        // packages.sort();
        let package_to_sites = packages
            .iter()
            .map(|p| (p.clone(), self.package_to_sites.get(p).unwrap().clone()))
            .collect();

        let sr = UnpackReport::from_package_to_sites(false, &package_to_sites);
        sr.remove(log)
    }

    pub(crate) fn site_validate_install(
        &self,
        bound: &Path,
        bound_options: &Option<Vec<String>>,
        vf: &ValidationFlags,
        exit_else_warn: Option<i32>,
        log: bool,
    ) -> ResultDynError<()> {
        if self.exe_to_sites.len() > 1 {
            return Err(format!("site-install will not operate on multiple ({}) Python environments; use `-e` to specify a single Python environment.", self.exe_to_sites.len()).into());
        }
        let ba = path_normalize(bound, true)?;
        // generally expect this to run with a single exe, so no need to parallelize
        for (exe, sites) in &self.exe_to_sites {
            // NOTE: taking the first site, but might prioritize by some other criteria
            if let Some(site) = sites.first() {
                install_validation(
                    exe,
                    &ba,
                    bound_options.clone(),
                    vf,
                    exit_else_warn,
                    site,
                    env::current_dir().ok(), // as option type
                    log,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn site_validate_uninstall(&self, log: bool) -> ResultDynError<()> {
        if self.exe_to_sites.len() > 1 {
            return Err(format!("site-install will not operate on multiple ({}) Python environments; use `-e` to specify a single Python environment.", self.exe_to_sites.len()).into());
        }
        for sites in self.exe_to_sites.values() {
            if let Some(site) = sites.first() {
                uninstall_validation(site, log)?;
            }
        }
        Ok(())
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_get_site_package_dirs_a() {
        let p1 = Path::new("python3");
        let paths1 = get_site_package_dirs(p1, true, false);
        assert_eq!(paths1.len() > 0, true);
        let paths2 = get_site_package_dirs(p1, false, false);
        assert!(paths1.len() >= paths2.len());
    }
    #[test]
    fn test_from_exe_to_sites_a() {
        let fp_dir = tempdir().unwrap();
        let fp_exe = fp_dir.path().join("python");
        let _ = File::create(fp_exe.clone()).unwrap();

        let fp_sp = fp_dir.path().join("site-packages");
        fs::create_dir(fp_sp.clone()).unwrap();

        let fp_p1 = fp_sp.join("numpy-1.19.1.dist-info");
        fs::create_dir(&fp_p1).unwrap();

        let fp_p2 = fp_sp.join("foo-3.0.dist-info");
        fs::create_dir(&fp_p2).unwrap();

        let mut exe_to_sites = HashMap::<PathBuf, Vec<PathShared>>::new();
        exe_to_sites.insert(
            fp_exe.clone(),
            vec![PathShared::from_path_buf(fp_sp.to_path_buf())],
        );
        let sfs = ScanFS::from_exe_to_sites(exe_to_sites, false, "".to_string()).unwrap();
        assert_eq!(sfs.package_to_sites.len(), 2);

        let dm1 = DepManifest::from_iter(vec!["numpy >= 1.19", "foo==3"]).unwrap();
        assert_eq!(dm1.len(), 2);
        let invalid1 = sfs.to_validation_report(
            dm1,
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        assert_eq!(invalid1.len(), 0);

        let dm2 = DepManifest::from_iter(vec!["numpy >= 2", "foo==3"]).unwrap();
        let invalid2 = sfs.to_validation_report(
            dm2,
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        assert_eq!(invalid2.len(), 1);
    }
    //--------------------------------------------------------------------------
    #[test]
    fn from_exe_site_packages_a() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3.8/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("numpy", "1.20.1", None).unwrap(),
            Package::from_name_version_durl("numpy", "2.1.1", None).unwrap(),
            Package::from_name_version_durl("requests", "0.7.6", None).unwrap(),
            Package::from_name_version_durl("requests", "2.32.3", None).unwrap(),
            Package::from_name_version_durl("flask", "3.0.3", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();
        assert_eq!(sfs.package_to_sites.len(), 7);
        // sfs.report();
        let dm = sfs.to_dep_manifest(Anchor::Lower).unwrap();
        assert_eq!(dm.len(), 3);
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_validation_a() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("requests", "0.7.6", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let dm = DepManifest::from_iter(
            vec!["numpy>1.19", "requests==0.7.6", "flask> 1"].iter(),
        )
        .unwrap();

        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();
        let vr = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        assert_eq!(vr.len(), 0);
    }
    #[test]
    fn test_validation_b() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("requests", "0.7.6", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let dm = DepManifest::from_iter(
            vec!["numpy>1.19", "requests==0.7.6", "flask> 2"].iter(),
        )
        .unwrap();

        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();
        let vr = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );

        let json = serde_json::to_string(&vr.to_validation_digest()).unwrap();
        assert_eq!(
            json,
            r#"[{"package":"flask-1.1.3","dependency":"flask>2","explain":"Misdefined","sites":["/usr/lib/python3/site-packages"]}]"#
        );
    }
    #[test]
    fn test_validation_c() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("requests", "0.7.6", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let dm = DepManifest::from_iter(
            vec!["numpy>2", "requests==0.7.1", "flask> 2,<3"].iter(),
        )
        .unwrap();

        let sfs = ScanFS::from_exe_site_packages(exe.clone(), site, packages).unwrap();
        let vr = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        assert_eq!(sfs.exe_to_sites.get(&exe).unwrap()[0].strong_count(), 8);
        let json = serde_json::to_string(&vr.to_validation_digest()).unwrap();
        assert_eq!(
            json,
            r#"[{"package":"flask-1.1.3","dependency":"flask>2,<3","explain":"Misdefined","sites":["/usr/lib/python3/site-packages"]},{"package":"numpy-1.19.3","dependency":"numpy>2","explain":"Misdefined","sites":["/usr/lib/python3/site-packages"]},{"package":"requests-0.7.6","dependency":"requests==0.7.1","explain":"Misdefined","sites":["/usr/lib/python3/site-packages"]}]"#
        );
    }

    #[test]
    fn test_validation_d() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("requests", "0.7.6", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let dm = DepManifest::from_iter(vec!["numpy>2", "flask> 2,<3"].iter()).unwrap();

        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();

        let vr = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: true,
                permit_subset: false,
            },
        );
        let json = serde_json::to_string(&vr.to_validation_digest()).unwrap();
        assert_eq!(
            json,
            r#"[{"package":"flask-1.1.3","dependency":"flask>2,<3","explain":"Misdefined","sites":["/usr/lib/python3/site-packages"]},{"package":"numpy-1.19.3","dependency":"numpy>2","explain":"Misdefined","sites":["/usr/lib/python3/site-packages"]}]"#
        );
    }
    #[test]
    fn test_validation_e() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();

        // hyphen / underscore are normalized
        let dm = DepManifest::from_iter(
            vec!["numpy==1.19.3", "flask>1,<2", "static_frame==2.13.0"].iter(),
        )
        .unwrap();
        let vr = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        assert_eq!(vr.len(), 0);
    }
    #[test]
    fn test_validation_f() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();

        // hyphen / underscore are normalized
        let dm = DepManifest::from_iter(
            vec!["numpy==1.19.3", "flask>1,<2", "static_frame==2.13.0"].iter(),
        )
        .unwrap();
        let vr = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        assert_eq!(vr.len(), 1);
        let json = serde_json::to_string(&vr.to_validation_digest()).unwrap();
        assert_eq!(
            json,
            r#"[{"package":null,"dependency":"flask>1,<2","explain":"Missing","sites":null}]"#
        );
    }
    #[test]
    fn test_validation_g() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();
        let dm = DepManifest::from_iter(vec!["numpy==1.19.3"].iter()).unwrap();
        let vr1 = sfs.to_validation_report(
            dm.clone(),
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        assert_eq!(vr1.len(), 1);
        let json = serde_json::to_string(&vr1.to_validation_digest()).unwrap();
        assert_eq!(
            json,
            r#"[{"package":"static-frame-2.13.0","dependency":null,"explain":"Unrequired","sites":["/usr/lib/python3/site-packages"]}]"#
        );

        let vr2 = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: true,
                permit_subset: false,
            },
        );
        assert_eq!(vr2.len(), 0);
    }
    #[test]
    fn test_validation_h() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages).unwrap();

        // hyphen / underscore are normalized
        let dm = DepManifest::from_iter(
            vec!["numpy==1.19.3", "flask>1,<2", "static_frame==2.13.0"].iter(),
        )
        .unwrap();
        let vr1 = sfs.to_validation_report(
            dm.clone(),
            ValidationFlags {
                permit_superset: false,
                permit_subset: false,
            },
        );
        let json = serde_json::to_string(&vr1.to_validation_digest()).unwrap();
        assert_eq!(
            json,
            r#"[{"package":null,"dependency":"flask>1,<2","explain":"Missing","sites":null}]"#
        );

        let vr2 = sfs.to_validation_report(
            dm,
            ValidationFlags {
                permit_superset: false,
                permit_subset: true,
            },
        );
        assert_eq!(vr2.len(), 0);
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_search_a() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages.clone()).unwrap();
        let matched = sfs.search_by_match("*.3", true);
        assert_eq!(matched, vec![packages[2].clone(), packages[0].clone()]);
    }

    #[test]
    fn test_search_b() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages.clone()).unwrap();
        let matched = sfs.search_by_match("*frame*", true);
        assert_eq!(matched, vec![packages[1].clone()]);
    }

    //--------------------------------------------------------------------------

    #[test]
    fn test_serialize_a() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages.clone()).unwrap();
        let json = serde_json::to_string(&sfs).unwrap();
        assert_eq!(json, "[[[\"/usr/bin/python3\",[\"/usr/lib/python3/site-packages\"]]],[[{\"name\":\"flask\",\"key\":\"flask\",\"version\":\"1.1.3\",\"direct_url\":null},[\"/usr/lib/python3/site-packages\"]],[{\"name\":\"numpy\",\"key\":\"numpy\",\"version\":\"1.19.3\",\"direct_url\":null},[\"/usr/lib/python3/site-packages\"]],[{\"name\":\"static-frame\",\"key\":\"static_frame\",\"version\":\"2.13.0\",\"direct_url\":null},[\"/usr/lib/python3/site-packages\"]]],[[\"/usr/lib/python3/site-packages\",\"/usr/bin/python3\"]],false,\"35cc8bbf5f965f99f2ed716a23e0cfbb70b8977ba65e837708e960fc13e51da2\"]");

        let sfsd: ScanFS = serde_json::from_str(&json).unwrap();
        assert_eq!(sfsd.exe_to_sites.len(), 1);
        assert_eq!(sfsd.package_to_sites.len(), 3);
    }

    #[test]
    fn test_to_hash_a() {
        let exe = PathBuf::from("/usr/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages.clone()).unwrap();
        assert_eq!(
            sfs.exes_hash,
            "35cc8bbf5f965f99f2ed716a23e0cfbb70b8977ba65e837708e960fc13e51da2"
        );
    }

    #[test]
    fn test_to_hash_b() {
        let exe = PathBuf::from("/usr/local/bin/python3");
        let site = PathBuf::from("/usr/lib/python3/site-packages");
        let packages = vec![
            Package::from_name_version_durl("numpy", "1.19.3", None).unwrap(),
            Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap(),
            Package::from_name_version_durl("flask", "1.1.3", None).unwrap(),
        ];
        let sfs = ScanFS::from_exe_site_packages(exe, site, packages.clone()).unwrap();
        assert_eq!(
            sfs.exes_hash,
            "973122597250deea4e62e359208ab4335782561c12032746ce044a387a201d09"
        );
    }

    #[test]
    fn test_site_install_a() {
        let site_shared1 = PathShared::from_str("foo");
        let site_shared2 = PathShared::from_str("bar");
        let exe1 = PathBuf::from("a");
        let exe2 = PathBuf::from("b");

        let p1 = Package::from_name_version_durl("numpy", "1.19.3", None).unwrap();
        let p2 = Package::from_name_version_durl("static-frame", "2.13.0", None).unwrap();
        let p3 = Package::from_name_version_durl("flask", "1.1.3", None).unwrap();

        let mut exe_to_sites = HashMap::new();
        exe_to_sites.insert(exe1.clone(), vec![site_shared1.clone()]);
        exe_to_sites.insert(exe2.clone(), vec![site_shared2.clone()]);

        let exes = vec![exe1.clone(), exe2.clone()];

        let mut package_to_sites = HashMap::new();
        package_to_sites.insert(p1, vec![site_shared1.clone()]);
        package_to_sites.insert(p2, vec![site_shared1.clone()]);
        package_to_sites.insert(p3, vec![site_shared1.clone(), site_shared2.clone()]);

        let mut site_to_exe = HashMap::new();
        site_to_exe.insert(site_shared1.clone(), exe1.clone());
        site_to_exe.insert(site_shared2.clone(), exe2.clone());

        let force_usite = false;
        let exes_hash = hash_paths(&exes, force_usite);
        let sfs = ScanFS {
            exe_to_sites,
            package_to_sites,
            site_to_exe,
            exe_to_ems: None,
            force_usite,
            exes_hash,
        };

        let bound = PathBuf::from("foo");
        let bound_options = None;
        let vf = ValidationFlags {
            permit_superset: false,
            permit_subset: false,
        };
        // ensure this retruns an error when multiple exe are defined
        assert!(sfs
            .site_validate_install(&bound, &bound_options, &vf, None, false)
            .is_err());
    }

    #[test]
    fn test_from_exes_a() {
        let exe1 = PathBuf::from("a");
        let exe2 = PathBuf::from("b");
        let exes = vec![exe1, exe2];
        let post = ScanFS::from_exes(&exes, false, false);
        // error for bad exe
        assert!(post.is_err());
    }
}
