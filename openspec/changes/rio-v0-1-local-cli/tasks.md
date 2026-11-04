## 1. Workspace and contracts

- [ ] 1.1 Create the Cargo workspace with crates `rio-cli`, `rio-core`, `rio-git`, `rio-profile`, `rio-adapters`, `rio-graph`, `rio-engine`, and placeholder `rio-client` and `rio-runner` (README only), plus `modules/`, `schemas/`, `tests/fixtures/`, `eval/README.md`; verify `cargo build --workspace` and `cargo test --workspace` succeed on an empty tree
- [ ] 1.2 Add shared workspace dependencies and lints (`serde`, `serde_json`, `serde_yaml`, `jsonschema`, `blake3`, `thiserror`, `anyhow`, `tokio`, `tracing`, `clap`, `insta`, `proptest`, `assert_cmd`) with `rust-toolchain.toml` and `clippy` set to deny warnings; verify `cargo clippy --workspace --all-targets` is clean
- [ ] 1.3 Write `schemas/profile-v1.json` covering every top-level key in the review-profiles spec (including `fail_on`, `min_evidence`, `max_lines`, step `rules` and `agents` blocks) and `schemas/module-v1.json` per design D7; verify a fixture set of valid and invalid YAML documents under `crates/rio-profile/tests/fixtures/` validates as expected
- [ ] 1.4 Write `schemas/finding-v1.json` and `schemas/report-v1.json` from the findings and review-report specs; verify the schemas load with the `jsonschema` crate and a hand-written example document validates
- [ ] 1.5 Author the built-in modules under `modules/`: `adapters/typescript/module.yaml`, `adapters/python/module.yaml`, `steps/{prepare,checks,graph,triage,agents,verify,report}/module.yaml` with `local` flags and `with` schemas, `adapters/common/secrets.toml`, and `profiles/base.yaml` per design D7; verify each manifest validates against `module-v1.json` in a test

## 2. Core review model (`rio-core`)

- [ ] 2.1 Implement `Finding`, `Location`, `EvidenceLevel` (ordered), `Category`, `Severity`, `State` and `CheckSkipped` with serde derives matching `finding-v1.json`; verify a round-trip test serializes a finding and validates it against the schema
- [ ] 2.2 Implement finding identity per design D10 (location normalization, message normalization, `blake3` id) and root-cause merging of `locations`; verify unit tests for each normalization rule, a test that messages differing only in a numeric literal get different ids, and a `proptest` that inserting lines above a symbol and reordering functions in a file leaves the id unchanged
- [ ] 2.3 Implement the `Report` model (header, checks, impact summary, findings, risks, summary counts, footer) and the `ReviewEvent` enum per design D9; verify serde round-trip tests and that the event names match the TRD SSE event list in a doc comment test
- [ ] 2.4 Implement suppression matching (`path` glob, `rule` glob, `finding` id) and `fail_on` evaluation (evidence, categories, states, rules) over a report; verify unit tests for each entry kind and for the default policy ignoring risks and pre-existing findings

## 3. Git access, diff and snapshot (`rio-git`)

- [ ] 3.1 Define the `Repo` trait (resolve ref, merge base, read tree and blob, index entries, status, untracked listing) and implement it with `gix`; verify tests on a temporary repository created in the test for refs, merge base and blob reads
- [ ] 3.2 Implement the three selection modes producing a `ChangeSet` with base/head identifiers, including unborn `HEAD` as an empty base and untracked non-ignored files in working-tree mode; verify integration tests for each scenario in the change-snapshot spec, including unknown ref and no merge base errors
- [ ] 3.3 Implement the diff with per-file status, rename detection, hunks with base and head line ranges, binary detection, and exclusion marking (vendored, `linguist-generated`, lockfile); verify `insta` snapshot tests over a fixture repository covering add, modify, delete, rename-with-edit, binary and lockfile cases
- [ ] 3.4 Implement the content-addressed snapshot (canonical encoding, BLAKE3 key, `tar.zst` archive with manifest) written under the cache; including explicit deletion records in the key and the manifest; verify tests that equal inputs give equal keys, a whitespace change alters the key, two deletion-only changes of different files get different keys, and applying the archive to the base tree reconstructs the head without deleted or renamed-away files
- [ ] 3.5 Implement base tree materialization into `.rio/cache/baselines/<base-sha>/tree/` and head tree materialization into `.rio/cache/heads/<snapshot-key>/tree/` by object extraction and snapshot application, with dependency directory symlinks, disposable copy-on-write or full-copy clones for tool runs (never hard links), dependency-state hashing, and the dependency compatibility checks per design D3 (installed set read from package-manager installation metadata compared with the head's and the base's lockfile, and in-repository link targets compared across work tree, head and base); verify a test that materializes a fixture base and head, confirms `git status` in the source repository is unchanged, confirms `node_modules` resolves through the symlink, confirms a `--staged` head excludes an unstaged edit, confirms that a tool rewriting a file in place leaves the cached tree unchanged, and confirms that an unstaged lockfile change, a workspace link to modified source, a stale installation after a branch switch and a root without a lockfile each skip the root with `dependency_mismatch`

## 4. Profile engine (`rio-profile`)

- [ ] 4.1 Implement profile discovery (path, `.rio/profiles/`, `~/.config/rio/profiles/`, built-ins embedded with `include_dir!`) and the default resolution order; verify tests with temporary directories for every discovery scenario in the review-profiles and `cli/profile` specs
- [ ] 4.2 Implement YAML load to `serde_json::Value`, schema validation with YAML-path error reporting, and single inheritance with deep-merge mappings and list replacement, including cycle detection and chain recording; verify fixture tests for the merge scenarios and a `proptest` that merging an empty child is the identity
- [ ] 4.3 Implement semantic validation: unique step ids, `needs` DAG and cycle detection, built-in module resolution with third-party rejection, `with` validation against module schemas, positive budgets, verifier model differs from default, `min_evidence` floor, verifier-required-with-agents guardrail; verify one failing fixture per rule with the expected error text
- [ ] 4.4 Implement the rule expression parser (predicates, boolean operators, actions) producing an AST with syntax-only validation; verify parser tests for every predicate and action and for unknown names being rejected
- [ ] 4.5 Implement `ResolvedProfile` serialization to `profile.resolved.yaml` with chain, pinned module versions and engine version, plus its hash; verify a snapshot test of the resolved `rio/base` and a test that changing a suppression changes the hash

## 5. Language adapters (`rio-adapters`)

- [ ] 5.1 Define the `Adapter` trait, capability enum, `ToolResolver` (project-local before `PATH`, version capture) and `CheckOutcome` (findings or `CheckSkipped`); verify unit tests for resolver precedence using temporary directories with fake executables
- [ ] 5.2 Implement repository detection and per-file project roots for TypeScript and Python manifests, including monorepos and manifest-less fallbacks; verify tests on `tests/fixtures/monorepo-sample` and on a manifest-less tree
- [ ] 5.3 Implement exclusion rules (vendored directories, `.gitattributes` `linguist-generated`, profile globs) with reasons; verify unit tests per rule
- [ ] 5.4 Implement the TypeScript adapter `typecheck` (`tsc --noEmit --pretty false`) and `lint` (`eslint -f json`, only with a project config) with their parsers; verify parser tests against recorded outputs and an integration test on `ts-sample` producing `typecheck/tsc/TS2322` and a `lint/eslint/*` finding when the tools are installed, skipped with reason otherwise
- [ ] 5.5 Implement the TypeScript `test`/`coverage` capability for vitest and jest with LCOV parsing; verify an integration test on `ts-sample` yielding the expected untested line ranges when the runner is installed
- [ ] 5.6 Implement the Python adapter `typecheck` (pyright JSON or mypy JSON with line fallback, chosen by configuration then availability) and `lint` (`ruff check --output-format json`, only with a config); verify parser tests against recorded outputs and an integration test on `py-sample`
- [ ] 5.7 Implement the Python `test`/`coverage` capability (`coverage run -m pytest`, `coverage json`) with the coverage.py JSON parser; verify an integration test on `py-sample` yielding expected untested ranges
- [ ] 5.8 Implement the built-in secrets scanner (pattern catalog from `secrets.toml`, entropy heuristic, hunk-based state, four-character redaction); verify unit tests for every catalog pattern's positive and negative cases and that no rendered output contains a full matched value
- [ ] 5.9 Implement lockfile parsers (`package-lock.json` v2/v3, `pnpm-lock.yaml`, `yarn.lock` classic and berry, `uv.lock`, `poetry.lock`, `Pipfile.lock`, pinned `requirements*.txt`) producing `(ecosystem, name, version)` sets; verify parser tests on recorded lockfiles for each format
- [ ] 5.10 Implement the dependency diff and the OSV batch client (changed packages only, batches of 100, 10-second timeout, `offline` and `network_error` skips); verify a test against a local mock HTTP server for the finding mapping and a test that `--offline` makes no request

## 6. Impact graph (`rio-graph`)

- [ ] 6.1 Implement tree-sitter symbol tables for TypeScript and Python from the adapters' `queries/symbols.scm`, with in-memory caching by content hash; verify snapshot tests of symbol tables for the fixture files, including nested symbol paths
- [ ] 6.2 Implement seed selection from hunks for base and head, including file-level seeds and deleted symbols kept in base; verify tests on the `ts-sample` and `py-sample` diffs
- [ ] 6.3 Implement reference search (Aho-Corasick over `ignore`-walked files, tree-sitter scope filtering, short-identifier rule) and import resolution for TypeScript and Python; verify tests that comment and string occurrences create no edge and that an imported reference is `resolved`
- [ ] 6.4 Implement bounded breadth-first expansion with `hops` and `max_nodes`, the edge model, `tests` edges, and the base/head/delta slices written as JSON; verify snapshot tests of the three slices on the fixtures and a truncation test with a low `max_nodes`
- [ ] 6.5 Implement the impact summary (references per hop, test coverage by reference, unresolved count) consumed by the report; verify a unit test on a fixture graph

## 7. Local engine (`rio-engine`)

- [ ] 7.1 Implement the step executor that topologically orders the resolved profile, executes steps whose module is `local`, defers the others with reason `no_endpoint`, and emits `ReviewEvent`s on a channel; verify a test with the `rio/base` profile asserting the executed and deferred sets and event order
- [ ] 7.2 Implement the `prepare` step's local part (detection, exclusions, `max_lines` note) and the `checks` step scheduling with per-kind enable flags, per-check timeouts, bounded parallelism and `kill_on_drop` processes; verify tests with a fake slow tool that a timeout produces `check_skipped(timeout)` without affecting other checks
- [ ] 7.3 Implement baseline execution for `typecheck` and `lint` using materialization, classification into new/pre-existing/fixed/unclassified, all tool-executing checks run on materialized trees, and the baseline cache keyed per design D3 (base, root, tool and version, config, invocation with target files, dependency state); verify integration tests on a fixture where a signature change breaks an unchanged caller (classified `new`), a pre-existing error stays `pre-existing`, a second run reports the baseline as cached, linting a different changed file does not reuse the cached lint baseline, a lockfile change makes that root's findings `unclassified` with `baseline_failed`, and an edit to a linked workspace package with unchanged manifests also makes the importing root's findings `unclassified`
- [ ] 7.4 Implement the `graph` and `report` steps, suppression application, `fail_on` evaluation, and review id derivation; verify an end-to-end engine test on `ts-sample` producing a `Report` whose JSON validates against `report-v1.json`
- [ ] 7.5 Implement cancellation (first signal cancels in-flight checks and marks the report partial, second signal exits 130) and the review cache writer with `.rio/cache/.gitignore`, raw tool output retention and age-based pruning; verify a test that cancels mid-run and finds the partial report with the interrupted check listed, a test that pruning removes only expired `.rio/cache/<review-id>/` directories and unreferenced snapshots and heads, and a test that an `--offline` run in which any tool executes records the subprocess network note in the footer

## 8. Renderers (`rio-core`)

- [ ] 8.1 Implement the terminal renderer: live consumption of events, section order, finding ordering, redaction, `--show-pre-existing`, width-aware location lines, color gating by TTY, `NO_COLOR` and `--no-color`; verify `insta` snapshots of the fixture reports at 80 and 120 columns and a test that `NO_COLOR` output has no escape sequences
- [ ] 8.2 Implement the Markdown renderer with stable headings and per-check tables; verify `insta` snapshots and a test that two renders of different reviews share heading text
- [ ] 8.3 Implement the JSON renderer with metrics and schema validation at render time; verify a test that the output validates against `report-v1.json` and contains per-check wall times

## 9. CLI (`rio-cli`)

- [ ] 9.1 Implement `rio review` argument parsing (selection flags with mutual exclusion, `--profile`, `--format`, `--offline`, `--no-color`, `--show-pre-existing`, endpoint-only flags rejected with the version message) and the exit code mapping 0/1/2/3; verify `assert_cmd` tests for every `cli/review` scenario including conflicting flags, `--pr`, not-a-repo and empty change set
- [ ] 9.2 Wire `rio review` to the engine with stdout/stderr separation per format, progress on stderr, the review id header and signal handling; verify an `assert_cmd` test that `--format json` yields exactly one parseable document on stdout with progress on stderr
- [ ] 9.3 Implement `rio profile list|show|validate` with `--raw`, source marking and default marking; verify `assert_cmd` tests for each scenario in the `cli/profile` spec, including the third-party module rejection and the local-only warning on `rio/base`
- [ ] 9.4 Implement `rio doctor` with `--format json`, warnings versus blocking problems and the exit code rule; verify `assert_cmd` tests for the missing type checker, project-local tool preference and warnings-only exit 0 scenarios
- [ ] 9.5 Implement `~/.config/rio/config.toml` reading for `endpoint.url` with the "not supported in this version" notice; verify a test with a temporary home directory

## 10. Fixtures, integration and CI

- [ ] 10.1 Build `tests/fixtures/ts-sample` (vitest, eslint config, tsconfig, seeded type error, pre-existing error, untested function, planted fake secret, lockfile with a known-vulnerable version) and `tests/fixtures/py-sample` (pyproject with ruff and mypy config, pytest, seeded issues) and `tests/fixtures/monorepo-sample` with two TypeScript packages; verify each fixture installs and its own tests pass with its toolchain
- [ ] 10.2 Write end-to-end `assert_cmd` tests running `rio review` on each fixture in working-tree, staged and `--base` modes with all three formats, asserting the seeded findings, their states, the footer entries and exit codes; verify they pass locally with tools installed and report skips without them
- [ ] 10.3 Add a GitHub Actions workflow that installs Rust, Node with pnpm, Python with `pyright`, `mypy`, `ruff` and `coverage`, runs `cargo fmt --check`, `cargo clippy`, `cargo test --workspace` and the end-to-end tests on Linux and macOS; verify the workflow is green on the first push
- [ ] 10.4 Measure fast-mode wall time of `rio review --base` on a 300-line diff in a mid-size TypeScript repository and record p50 over five runs in `docs/benchmarks.md`; verify the recorded p50 is at or under 4 minutes or an issue is filed with the breakdown by check

## 11. Documentation

- [ ] 11.1 Write `README.md` (install from source, `rio review` quick start, what runs locally, the OSV network call, how to disable coverage) and `docs/profiles.md` (schema v1 reference generated from `profile-v1.json`, inheritance rules, `fail_on`, suppressions); verify every documented flag and key exists by running the documented commands against the fixtures
- [ ] 11.2 Write `docs/adapters.md` describing the module manifest v1, the adapter contract and how to add a language; verify the document's example manifest validates against `module-v1.json` in a doc test
