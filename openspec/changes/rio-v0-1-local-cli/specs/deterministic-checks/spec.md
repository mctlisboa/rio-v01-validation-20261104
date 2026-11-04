## Purpose

Deterministic checks are the first layer of a review: everything a tool can prove about the change (types, lint rules, secrets, untested lines, vulnerable dependencies) is established without any model, and old issues are separated from regressions by comparing both versions.

## ADDED Requirements

### Requirement: Check kinds and rule ids
The checks step SHALL run, per detected language and project root, the following kinds with stable rule id prefixes: `typecheck/<tool>/<code>`, `lint/<tool>/<rule>`, `secret/<pattern>`, `coverage/untested_change`, `dependency/vulnerable_dependency`. Each kind SHALL be individually enabled or disabled through the `rio/checks` step's `with` options, and every finding SHALL carry evidence level `static_proven`.

#### Scenario: Kind disabled in profile
- **WHEN** the `rio/checks` step sets `with: { coverage: false }`
- **THEN** no test or coverage tool runs and the footer lists coverage as skipped with reason `disabled`

### Requirement: Baseline classification
For `typecheck` and `lint`, the checks step SHALL run the tool on the materialized head tree and, when `baseline` is enabled (default), on the materialized base tree, and SHALL classify each finding by identity as `new` (head only), `pre-existing` (both) or `fixed` (base only). Base results SHALL be cached under a key that covers every input that can change them: base identifier, project root, check kind, tool and tool version, the tool's configuration files, the exact invocation including the set of target files, and the dependency state (the hash of the root's manifests and lockfiles plus the content hash of every in-repository linked package's source). The base run uses the same installed dependencies as the head run, so it SHALL be used for classification only when that installation is also valid for the base: the installed dependency set equals the set the base's lockfile resolves, and every installed dependency resolving into the repository (workspace links, editable installs) points at source identical in the base and the work tree. When either condition fails or cannot be established, that root's `typecheck` and `lint` findings SHALL be classified `unclassified` and the footer SHALL record `baseline_failed` with a hint naming the dependency or linked package that differs. When the base run cannot complete, findings SHALL be classified `unclassified` and the footer SHALL say why.

#### Scenario: Error caused in an unchanged file
- **WHEN** a change alters a function signature and `tsc` reports a new error in an unchanged caller file
- **THEN** that finding is classified `new`, because it is absent from the base run

#### Scenario: Pre-existing error not shown as a regression
- **WHEN** `tsc` reports the same error in both base and head
- **THEN** the finding is classified `pre-existing`, is not listed among new findings, and is counted in the report's pre-existing summary

#### Scenario: Base run reused from cache
- **WHEN** two consecutive reviews share the same base commit, project root, tool configuration, invocation and dependency state
- **THEN** the second review does not re-run the base type check and the footer reports the baseline as cached

#### Scenario: Lint baseline not reused across different file sets
- **WHEN** one review lints changed file `a.ts` and a later review on the same base lints changed file `b.ts`
- **THEN** the second review does not reuse the first review's lint baseline, and pre-existing lint errors in `b.ts` are classified `pre-existing`, not `new`

#### Scenario: Dependency change disables classification
- **WHEN** a change upgrades a package in the lockfile and the type check reports an error in an unchanged file
- **THEN** that root's type-check findings are classified `unclassified`, not `pre-existing`, and the footer records `baseline_failed` with a hint that dependencies changed

#### Scenario: Workspace source change disables classification
- **WHEN** a change edits `packages/ui/src/button.ts`, which another root imports through the workspace link `node_modules/@acme/ui`, without touching any manifest or lockfile, and the type check reports an error in an unchanged caller
- **THEN** the importing root's type-check findings are classified `unclassified`, not `pre-existing`, and the footer records `baseline_failed` with a hint naming `packages/ui`

### Requirement: Head dependency compatibility
Tool-executing checks (`typecheck`, `lint`, `coverage`) use the dependencies installed in the work tree. Before running them on a project root, the checks step SHALL verify that the installed dependencies match the selected head, by comparing the `(name, version)` set recorded in the package manager's installation metadata with the set resolved by the head's lockfile, without assuming the installation was made from the work tree's current manifests; and that every installed dependency resolving into the repository (workspace links, editable installs) points at source identical in the work tree and the selected head. When either condition fails, or the installed state cannot be established (no lockfile, no exact pins, missing or unsupported installation metadata), those checks SHALL be skipped for that root with reason `dependency_mismatch` and a hint naming the differing file or package, rather than producing results computed against dependencies that do not belong to the reviewed head.

#### Scenario: Unstaged dependency upgrade during a staged review
- **WHEN** `rio review --staged` runs while `package.json` and `package-lock.json` have an unstaged upgrade that is installed in `node_modules`
- **THEN** the root's type check, lint and coverage are skipped with reason `dependency_mismatch` and the hint names `package-lock.json`

#### Scenario: Workspace link to locally modified source
- **WHEN** `rio review --base main` runs and `node_modules/@acme/ui` links to `packages/ui`, which has uncommitted edits
- **THEN** checks for roots that depend on `@acme/ui` are skipped with reason `dependency_mismatch` and the hint names `packages/ui`

#### Scenario: Stale installation after a branch switch
- **WHEN** the user switches to a branch whose `package-lock.json` pins `react@18.3.1` without reinstalling, `node_modules/.package-lock.json` still records `react@18.2.0`, and `rio review` runs in working-tree mode
- **THEN** the root's type check, lint and coverage are skipped with reason `dependency_mismatch` and the hint names `react` and suggests reinstalling

#### Scenario: Installed state cannot be verified
- **WHEN** a Python root has only a `requirements.txt` without exact pins
- **THEN** the root's type check, lint and coverage are skipped with reason `dependency_mismatch` and the hint says the installed dependencies cannot be verified

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
Any check that does not run or does not complete SHALL appear in the report footer as a `check_skipped` entry with the check kind, language, project root, a reason from `tool_missing`, `no_project_config`, `not_supported_by_adapter`, `disabled`, `offline`, `network_error`, `timeout`, `unparseable_output`, `baseline_failed`, `dependency_mismatch`, `interrupted`, and a hint when one applies.

#### Scenario: Nothing skipped silently
- **WHEN** a review finishes and some capability was declared by an adapter but produced no result
- **THEN** the footer contains a `check_skipped` entry for it

### Requirement: Checks run concurrently and stream
Independent checks SHALL run concurrently up to a configured parallelism and SHALL report results to the renderer as each completes, in completion order. Each check SHALL have a timeout derived from the profile budget.

#### Scenario: Fast check appears before slow check
- **WHEN** the secrets scan finishes before the type check
- **THEN** the terminal shows the secrets results while the type check is still running
