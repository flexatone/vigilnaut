

# `fetter`: A Command-line tool for system-wide Python package allow-listing and vulnerability scanning

# Scan Your Entire System for Python Packages with Vulnerabilities

# System-Wide Python Package Control: Search, Allow-List, and Find Vulnerabilities,


A modern Python developer's environment is likely littered with numerous virtual environments and hundreds of packages. Many of these virtual environments might hold out-of-date packages with known security vulnerabilities.

Even when working in a single virtual environment, developer's installed packages can drift away from the projects specified requirements. Installing trial package might force an upgrade to requirement pinned package, leading to behaviors that deviate from expectations. Going further, teams might want to enforce that, within project virtual environments, only specified packages are permitted. This can thought of as a type Python package allow listing.

The `fetter` command-line application is an "extremely fast" tool for searching an entire system (or targeted virtual environments) for Python packages. Once found, those packages can be validated against a requirements or lock file, or searched for security vulnerabilities.