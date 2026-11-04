## Purpose

Deterministic checks are the first layer of a review: everything a tool can prove about the change (types, lint rules, secrets, untested lines, vulnerable dependencies) is established without any model, and old issues are separated from regressions by comparing both versions.

## ADDED Requirements

### Requirement: Check kinds and rule ids
The checks step SHALL run, per detected language and project root, the following kinds with stable rule id prefixes: `typecheck/<tool>/<code>`, `lint/<tool>/<rule>`, `secret/<pattern>`, `coverage/untested_change`, `dependency/vulnerable_dependency`. Each kind SHALL be individually enabled or disabled through the `rio/checks` step's `with` options, and every finding SHALL carry evidence level `static_proven`.

#### Scenario: Kind disabled in profile
- **WHEN** the `rio/checks` step sets `with: { coverage: false }`
- **THEN** no test or coverage tool runs and the footer lists coverage as skipped with reason `disabled`

### Requirement: Baseline classification
For `typecheck` and `lint`, the checks step SHALL run the tool on the head version and, when `baseline` is enabled (default), on the base version materialized from the change snapshot, and SHALL classify each finding by identity as `new` (head only), `pre-existing` (both) or `fixed` (base only). Base results SHALL be cached by base identifier, tool and tool configuration hash. When the base run cannot complete, findings SHALL be classified `unclassified` and the footer SHALL say why.

#### Scenario: Error caused in an unchanged file
- **WHEN** a change alters a function signature and `tsc` reports a new error in an unchanged caller file
- **THEN** that finding is classified `new`, because it is absent from the base run

#### Scenario: Pre-existing error not shown as a regression
- **WHEN** `tsc` reports the same error in both base and head
- **THEN** the finding is classified `pre-existing`, is not listed among new findings, and is counted in the report's pre-existing summary

#### Scenario: Base run reused from cache
- **WHEN** two consecutive reviews share the same base commit and tool configuration
- **THEN** the second review does not re-run the base type check and the footer reports the baseline as cached

### Requirement: Secrets scan
The secrets scan SHALL run over the head content of every changed text file using a built-in catalog of credential patterns (provider token formats, private key blocks, connection strings with embedded passwords) plus a high-entropy heuristic for values assigned to secret-like names. A match on an added or modified line SHALL be classified `new`; a match elsewhere in a changed file SHALL be `pre-existing`. The report SHALL NEVER print the matched value beyond its first four characters.

#### Scenario: Added API key
- **WHEN** an added line contains a value matching the AWS access key pattern
- **THEN** a `secret/aws-access-key` finding is produced, classified `new`, showing the value redacted after four characters

#### Scenario: Test fixture false positive is suppressible
- **WHEN** a profile has `suppress: [ { path: "tests/fixtures/**" } ]`
- **THEN** secret findings under that path are suppressed and counted in the footer

### Requirement: Diff coverage
When `coverage` is enabled and a test runner with coverage is resolved, the checks step SHALL run the project's tests with coverage, intersect the coverage data with the added and modified line ranges of non-test source files, and produce one `coverage/untested_change` finding per enclosing function (or per file when no enclosing symbol exists) listing the untested line ranges, with category `risk`. A changed source file absent from the coverage data SHALL count as entirely untested. Test files themselves SHALL be excluded. Test execution SHALL be bounded by the profile budget and killed on timeout, producing a skip with reason `timeout`.

#### Scenario: Partially tested function
- **WHEN** a function has added lines 10 to 20 and coverage shows lines 10 to 14 executed
- **THEN** one `untested_change` finding for that function lists lines 15 to 20

#### Scenario: Tests exceed budget
- **WHEN** the test command runs longer than the budget allows
- **THEN** it is terminated, coverage is skipped with reason `timeout`, and other checks' results are unaffected

### Requirement: Dependency audit
The dependency audit SHALL parse the manifests and lockfiles of both versions (npm `package-lock.json` v2 and v3, `pnpm-lock.yaml`, `yarn.lock`; Python `uv.lock`, `poetry.lock`, `Pipfile.lock`, and `requirements*.txt` with exact pins), compute added, upgraded and downgraded packages, query the OSV batch API for the head versions of changed packages only, and produce one `dependency/vulnerable_dependency` finding per package, version and advisory with the advisory id, severity and fixed version when known. With `--offline` or on network failure the audit SHALL be skipped with reason `offline` or `network_error`, never silently empty.

#### Scenario: Upgraded package with an advisory
- **WHEN** a lockfile upgrades a package to a version with a known OSV advisory
- **THEN** a finding names the package, the new version, the advisory id and the first fixed version

#### Scenario: Unchanged dependencies not queried
- **WHEN** the change does not modify any manifest or lockfile
- **THEN** the dependency audit reports nothing to audit and no network request is made

### Requirement: Skipped checks are always visible
Any check that does not run or does not complete SHALL appear in the report footer as a `check_skipped` entry with the check kind, language, project root, a reason from `tool_missing`, `no_project_config`, `not_supported_by_adapter`, `disabled`, `offline`, `network_error`, `timeout`, `unparseable_output`, `baseline_failed`, `interrupted`, and a hint when one applies.

#### Scenario: Nothing skipped silently
- **WHEN** a review finishes and some capability was declared by an adapter but produced no result
- **THEN** the footer contains a `check_skipped` entry for it

### Requirement: Checks run concurrently and stream
Independent checks SHALL run concurrently up to a configured parallelism and SHALL report results to the renderer as each completes, in completion order. Each check SHALL have a timeout derived from the profile budget.

#### Scenario: Fast check appears before slow check
- **WHEN** the secrets scan finishes before the type check
- **THEN** the terminal shows the secrets results while the type check is still running
