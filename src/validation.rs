// use std::fmt;
use std::cmp;
use std::path::PathBuf;

use crate::dep_spec::DepSpec;
use crate::package::Package;

// #[derive(PartialEq, Eq, Hash, Clone)]
#[derive(Debug)]
pub(crate) struct ValidationRecord {
    package: Package,
    dep_spec: Option<DepSpec>, // None if no depspec
    sites: Option<Vec<PathBuf>>,
}

impl ValidationRecord {
    pub(crate) fn new(
        package: Package,
        dep_spec: Option<DepSpec>,
        sites: Option<Vec<PathBuf>>,
    ) -> Self {
        ValidationRecord {
            package,
            dep_spec,
            sites,
        }
    }
}

// These are validation results
#[derive(Debug)]
pub struct Validation {
    pub records: Vec<ValidationRecord>,
}

impl Validation {
    /// The length of the scan is the number of unique packages.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn display(&self, include_sites: bool) {
        let mut package_displays: Vec<String> = Vec::new();
        let mut dep_spec_displays: Vec<String> = Vec::new();
        let mut site_displays: Vec<String> = Vec::new();

        let mut max_package_width = 0;
        let mut max_dep_spec_width = 0;

        let mut records: Vec<&ValidationRecord> = self.records.iter().collect();
        records.sort_by_key(|item| &item.package);

        for item in &records {
            let pkg_display = format!("{}", item.package);

            let dep_display = match &item.dep_spec {
                Some(dep_spec) => format!("{}", dep_spec),
                None => "-".to_string(),
            };

            if include_sites {
                let site_display = match &item.sites {
                    Some(sites) => sites
                        .iter()
                        .map(|s| format!("{:?}", s))
                        .collect::<Vec<_>>()
                        .join(","),
                    None => "".to_string(),
                };
                site_displays.push(site_display);
            }

            max_package_width = cmp::max(max_package_width, pkg_display.len());
            max_dep_spec_width = cmp::max(max_dep_spec_width, dep_display.len());

            package_displays.push(pkg_display);
            dep_spec_displays.push(dep_display);
        }
        // TODO: optionally show sites
        println!(
            "{:<package_width$} {:<dep_spec_width$}",
            "Package",
            "Dependency",
            package_width = max_package_width,
            dep_spec_width = max_dep_spec_width
        );

        for (pkg_display, dep_display) in package_displays.iter().zip(dep_spec_displays.iter()) {
            println!(
                "{:<package_width$} {:<dep_spec_width$}",
                pkg_display,
                dep_display,
                package_width = max_package_width,
                dep_spec_width = max_dep_spec_width
            );
        }
    }
}
