import os
import site
import sys
from packaging.requirements import Requirement
from packaging.version import Version, InvalidVersion
import pkg_resources


def parse_requirements(file_path):
    """Parse the requirements.txt file and return a list of Requirement objects."""
    requirements = []
    with open(file_path, 'r') as file:
        for line in file:
            line = line.strip()
            if line and not line.startswith("#"):
                try:
                    requirements.append(Requirement(line))
                except Exception as e:
                    print(f"Failed to parse requirement '{line}': {e}")
    return requirements

def get_installed_packages():
    """Return a dictionary of installed packages and their versions."""
    packages = {}
    for dist in pkg_resources.working_set:
        packages[dist.project_name] = dist.version
    return packages


def validate_requirements(requirements, installed_packages):
    """Validate each requirement against installed packages."""
    for req in requirements:
        package_name = req.name
        if package_name in installed_packages:
            installed_version = Version(installed_packages[package_name])
            if req.specifier.contains(installed_version, prereleases=True):
                print(f"{package_name} {installed_version} is compatible with {req}")
            else:
                print(f"{package_name} {installed_version} is NOT compatible with {req}")
        else:
            print(f"{package_name} is not installed")


def main():
    file_path = "requirements.txt"
    if not os.path.exists(file_path):
        print(f"File {file_path} does not exist.")
        sys.exit(1)

    requirements = parse_requirements(file_path)
    installed_packages = get_installed_packages()
    validate_requirements(requirements, installed_packages)

