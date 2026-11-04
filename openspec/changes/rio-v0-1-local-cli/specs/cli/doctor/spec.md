## Purpose

`rio doctor` diagnoses whether the local review pipeline can run in the current repository and tells the user exactly what is missing.

## ADDED Requirements

### Requirement: Diagnostic report
`rio doctor` SHALL report: whether the current directory is inside a git work tree and the current `HEAD`; the languages detected and their project roots; for each adapter capability, the tool that would be used with its resolved path and version, or that it is missing together with an install hint; the validity of the default profile; and that no endpoint is configured (informational in this version). `--format json` SHALL emit the same information as one JSON document.

#### Scenario: Missing type checker
- **WHEN** `rio doctor` runs in a Python project where neither `pyright` nor `mypy` is installed
- **THEN** the report marks the Python `typecheck` capability as missing and prints an install hint for both tools

#### Scenario: Project-local tool preferred
- **WHEN** a TypeScript project has `node_modules/.bin/tsc` and a different `tsc` on `PATH`
- **THEN** the report shows the project-local path and version for `typecheck`

### Requirement: Exit code reflects blocking problems
`rio doctor` SHALL exit 1 when a blocking problem exists (not a git repository, or the default profile is invalid) and 0 otherwise. Missing tools SHALL be reported as warnings and SHALL NOT affect the exit code.

#### Scenario: Only warnings
- **WHEN** the repository is valid, the profile is valid, and `eslint` is not installed
- **THEN** `rio doctor` exits 0 and shows `lint` as a warning
