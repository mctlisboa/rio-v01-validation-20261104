## Purpose

The `rio review` command is the terminal-first entry point of Rio: it reviews a set of local changes and renders a report on the developer's machine without any backend, endpoint or model provider.

## ADDED Requirements

### Requirement: Change selection modes
The command SHALL review exactly one change set per run, selected by mutually exclusive flags: no selection flag reviews working-tree changes (tracked modifications, staged changes and untracked files not ignored by git) against `HEAD`; `--staged` reviews the index against `HEAD`; `--base <ref>` reviews `HEAD` against the merge base of `<ref>` and `HEAD`. Supplying more than one selection flag SHALL be a usage error.

#### Scenario: Default reviews the working tree
- **WHEN** `rio review` is run with no selection flag in a repository with an unstaged edit, a staged edit and a new untracked file
- **THEN** all three files are part of the reviewed change set and the report header names the mode as `working-tree`

#### Scenario: Staged mode ignores unstaged edits
- **WHEN** `rio review --staged` is run with one staged file and a different file modified but unstaged
- **THEN** only the staged file is in the reviewed change set

#### Scenario: Branch mode uses the merge base
- **WHEN** `rio review --base origin/main` is run on a feature branch that is three commits ahead of `origin/main`, while `origin/main` has also advanced
- **THEN** the reviewed change set is the diff from the merge base of `origin/main` and `HEAD` to `HEAD`, and commits that only exist on `origin/main` are not reviewed

#### Scenario: Conflicting selection flags
- **WHEN** `rio review --staged --base main` is run
- **THEN** the command exits with code 2 and a message stating that selection flags are mutually exclusive, and no review runs

### Requirement: Endpoint-only flags fail explicitly
Flags that need a Rio endpoint (`--pr`, `--deep`, `--publish`, `--since-last`) SHALL be accepted by the parser and SHALL cause the command to exit with code 2 and a message that names the flag and states that it requires a configured endpoint, which v0.1 does not support. The command SHALL NOT run any part of the review in that case.

#### Scenario: PR flag without endpoint
- **WHEN** `rio review --pr 123` is run
- **THEN** the command exits with code 2 and prints a message naming `--pr` and stating that an endpoint is required and not available in this version

### Requirement: Output format selection
The command SHALL accept `--format terminal|json|md` and default to `terminal`. In `terminal` mode results SHALL stream to standard output as checks complete. In `json` and `md` modes the command SHALL write only the final document to standard output and SHALL send progress and diagnostics to standard error, so standard output is machine-consumable.

#### Scenario: JSON output is clean
- **WHEN** `rio review --format json` is run and standard output is redirected to a file
- **THEN** the file contains exactly one JSON document that parses, and all progress lines went to standard error

#### Scenario: Terminal output honors NO_COLOR
- **WHEN** `rio review` is run with the `NO_COLOR` environment variable set, or with `--no-color`, or with standard output not attached to a terminal
- **THEN** the terminal report contains no ANSI escape sequences

### Requirement: Offline operation
The command SHALL accept `--offline`. When set, the Rio process SHALL open no network connection, and built-in checks that need the network SHALL be reported as skipped with reason `offline`. Tool processes Rio executes (type checkers, linters and their plugins, and the project's tests run for coverage) are not network-isolated in this version; the guarantee does not extend to them, and the report footer SHALL state this whenever such a process ran. Preventing all network activity requires running Rio under external network isolation, such as a container or sandbox without network access, or disabling every check that runs a subprocess (`typecheck`, `lint` and `coverage`). When no endpoint is configured, every step that would need one SHALL be reported as deferred rather than failing the run.

#### Scenario: Offline run skips the dependency audit
- **WHEN** `rio review --offline` is run on a change that modifies a lockfile
- **THEN** the report footer lists the dependency audit as skipped with reason `offline` and every other check runs normally

#### Scenario: Offline footer states the subprocess limit
- **WHEN** `rio review --offline` runs and at least one tool or test process executes
- **THEN** the footer states that tool and test processes were not network-isolated

#### Scenario: No endpoint configured is not an error
- **WHEN** `rio review` is run on a machine with no Rio endpoint configured
- **THEN** the deterministic checks, impact graph and report run to completion, and the footer lists the agent and verifier steps as deferred because no endpoint is configured

### Requirement: Profile selection
The command SHALL accept `--profile <name-or-path>` and otherwise resolve the default profile as defined by the review-profiles capability. The resolved profile SHALL be validated before any check runs; a validation failure SHALL exit with code 2 listing every error.

#### Scenario: Invalid profile stops the run before checks
- **WHEN** `rio review --profile .rio/profiles/broken.yaml` is run and the file declares a step whose `needs` forms a cycle
- **THEN** the command exits with code 2, prints the cycle, and no adapter tool is executed

### Requirement: Progress and interruption
While checks run in terminal mode, the command SHALL show a progress line naming the checks in flight. A first interrupt signal SHALL stop pending checks, keep results already obtained, render the report marked as partial, and exit using the normal exit-code rules. A second interrupt signal SHALL terminate immediately with code 130.

#### Scenario: Single interrupt keeps the partial report
- **WHEN** the user presses Ctrl-C once while the coverage check is still running and the type check has finished with two findings
- **THEN** the coverage check is cancelled, the report shows the two type-check findings, the header marks the report as `partial`, and the footer lists the coverage check as skipped with reason `interrupted`

### Requirement: Exit codes
The command SHALL exit with code 0 when the review completed and no finding matched the profile's `fail_on` policy; 1 when at least one finding matched `fail_on`; 2 on usage or configuration errors; 3 when the review could not start, for example outside a git repository. A result with zero findings SHALL exit 0 and SHALL state explicitly that no findings were produced.

#### Scenario: Zero findings is a success
- **WHEN** `rio review` runs on a change where every check passes
- **THEN** the report states that no findings were produced and the exit code is 0

#### Scenario: Failing finding sets exit code
- **WHEN** the resolved profile's `fail_on` requires evidence `static_proven` on new defects and the type check reports a new error in the change
- **THEN** the exit code is 1 and the report marks that finding as failing the policy

#### Scenario: Not a git repository
- **WHEN** `rio review` is run in a directory that is not inside a git work tree
- **THEN** the command exits with code 3 and prints a message saying a git repository is required

### Requirement: Empty change set
When the selected mode yields no changed files, the command SHALL print that there is nothing to review and exit with code 0 without running any check.

#### Scenario: Clean working tree
- **WHEN** `rio review` is run with a clean working tree
- **THEN** the command prints "No changes to review" (or equivalent) and exits 0 without invoking any adapter tool

### Requirement: Run identity and reproducibility
Every run SHALL be identified by a review id derived deterministically from the snapshot key and the resolved profile hash, SHALL print the id, base and head identifiers, and the profile name in the report header, and SHALL persist its inputs and outputs under `.rio/cache/<review-id>/` as defined by the review-report capability.

#### Scenario: Identical inputs give identical ids
- **WHEN** `rio review --staged` is run twice without changing the index or the profile
- **THEN** both runs print the same review id
