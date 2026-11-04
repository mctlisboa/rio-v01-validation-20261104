## Context

Greenfield: the repository contains the PRD, the TRD and an empty OpenSpec root. No code, no CI, no fixtures. See proposal.md for scope and motivation; the ten delta specs under `specs/` define the behavior. This document fixes the technical shape of v0.1 so that v0.2 (endpoint, agents, verifier) extends it without rewriting it.

Constraints that shape the design:

- The TRD prescribes a Cargo workspace with named crates and says the Rust code is later shared with the self-hosted runner. Crate boundaries chosen now are therefore load-bearing.
- Profile schema v1, module manifest v1 and finding v1 become public contracts the moment they ship; v0.2 must only add to them.
- Everything runs on the developer's machine using tools the project already has. Rio never installs tools and never bundles a toolchain.
- "Old issues separated from regressions" requires running checks on the base version, which requires a materialized base tree with the project's dependencies available.
- The engine's event stream must map one-to-one onto the Server-Sent Events the TRD defines for v0.2, so the terminal renderer written now is reused unchanged against a remote review.

## Goals / Non-Goals

**Goals:**

- A `rio review` that completes in the fast budget on a 300-line TypeScript diff in a mid-size repo, with no network except OSV.
- Crate APIs that the runner (v0.2) can call directly: diff, snapshot, adapter execution, graph.
- Deterministic, reproducible runs: same inputs, same review id, same report.
- Every degradation visible in the footer.

**Non-Goals:**

- Any model call, endpoint, runner protocol, PR integration or publishing.
- Triage rule evaluation, hypothesis board, consolidation across runs, SARIF.
- Windows as a first-class target in v0.1 (best effort; symlink-based baseline may degrade to `baseline_failed` there).
- LSP-backed reference resolution.

## Decisions

### D1. Workspace layout mirrors the TRD, with two placeholder crates

```text
crates/
  rio-cli/        clap commands, renderers' wiring, config, keyring (unused in v0.1)
  rio-core/       Finding, EvidenceLevel, identity, Report, ReviewEvent, renderers (terminal, md, json)
  rio-git/        Repo access (gix), modes, Diff/Hunk model, Snapshot, base materialization
  rio-profile/    schema types, YAML load, inheritance, jsonschema validation, rule parser, module registry
  rio-adapters/   Adapter trait, ToolResolver, TypeScript + Python adapters, output parsers, secrets scanner, OSV client, coverage parsers
  rio-graph/      symbol tables (tree-sitter), reference search, expansion, slices, delta
  rio-engine/     local step executor: walks the resolved profile DAG and emits ReviewEvents (NEW crate, not in the TRD list)
  rio-client/     empty placeholder with a README (v0.2)
  rio-runner/     empty placeholder with a README (v0.2)
modules/          built-in rio/* modules: adapters/typescript, adapters/python, steps/{prepare,checks,graph,triage,agents,verify,report}, profiles/base.yaml
schemas/          profile-v1.json, module-v1.json, finding-v1.json, report-v1.json
tests/fixtures/   ts-sample/, py-sample/, monorepo-sample/ with seeded issues
eval/             placeholder README only
```

Rationale: `rio-engine` is added because the TRD puts step orchestration in the Cloudflare Workflow, but v0.1 needs a local orchestrator. Keeping it in its own crate makes the local-versus-remote split explicit: `rio-cli` depends on `rio-engine` in v0.1 and on `rio-client` in v0.2, selecting by configuration. Alternative considered: orchestration inside `rio-cli`. Rejected because the runner needs the same step executor in v0.2.

### D2. Git access through `gix`, with the backend behind a trait

`rio-git` exposes a `Repo` trait (resolve ref, merge base, read tree, read blob, status, untracked listing) implemented by `GixRepo`. If a needed operation turns out to be missing or unstable in `gix`, a `Git2Repo` implementation can be swapped in without touching callers. The `git` binary is never required. Alternative: shell out to `git`. Rejected because the self-hosted runner and sandbox should not depend on a git binary and because parsing porcelain output is brittle.

### D3. Base tree materialization by object extraction, with dependency directories linked in

To run `typecheck` and `lint` on the base version, `rio-git` writes the base tree from git objects into `.rio/cache/baselines/<base-sha>/tree/`, honoring the repository's ignore rules only for untracked paths (tracked files are always written). Dependency directories that are gitignored in the work tree (`node_modules`, `.venv`, `venv`, `__pycache__` are skipped) are symlinked from the matching project root in the work tree so tools can resolve imports. If the lockfile differs between base and head, the baseline still runs but the footer notes "baseline used head dependencies". Python editable installs that point at the work tree are detected (a `.pth` or `direct_url.json` under the venv referencing the work tree) and noted the same way.

Alternatives: (a) `git worktree add` — needs the git binary and pollutes `.git/worktrees`; (b) classify by hunk membership only — misses cross-file regressions, which is the main value of the baseline; (c) copy the whole work tree and revert changed files — fast but wrong for untracked state and large repos. (a) and (c) rejected; (b) is kept as the automatic fallback when materialization fails (`baseline_failed`, findings `unclassified`).

Baseline results are cached at `.rio/cache/baselines/<base-sha>/<check>/<config-hash>.json`, where `config-hash` covers the tool version and the tool's configuration files, so a profile change that does not affect the tool does not invalidate the baseline.

### D4. Tool output: machine-readable formats where they exist

| Capability | Tool | Invocation | Parse |
| --- | --- | --- | --- |
| typecheck TS | tsc | `tsc --noEmit --pretty false -p <root>` | line regex `path(line,col): error TSxxxx: msg` |
| lint TS | eslint | `eslint -f json <changed files>` | JSON |
| coverage TS | vitest / jest | `vitest run --coverage --coverage.reporter=lcov --coverage.reportsDirectory=<cache>` / `jest --coverage --coverageReporters=lcov --coverageDirectory=<cache>` | LCOV |
| typecheck Py | pyright / mypy | `pyright --outputjson` / `mypy --output json` (mypy >= 1.11; older versions fall back to line parsing) | JSON |
| lint Py | ruff | `ruff check --output-format json <changed files>` | JSON |
| coverage Py | coverage.py + pytest | `coverage run -m pytest -q` then `coverage json -o <cache>/coverage.json` | coverage.py JSON |

Lint runs on changed files only (head paths, and base paths for the baseline). Type checks run on the whole project root because partial type checking is unsound. Coverage runs the whole test suite; a per-change test selection is a later optimization.

Parsers live in `rio-adapters` next to each adapter and are unit-tested against recorded fixtures per tool version. A parse failure produces `unparseable_output` and keeps the raw output, per the language-adapters spec.

### D5. Secrets scanning is built in, not delegated

A pattern catalog (`modules/adapters/common/secrets.toml`) of well-known token formats and private-key blocks, plus an entropy heuristic (Shannon entropy over 3.5 bits per character on values of 20 or more characters assigned to names matching `secret|token|password|passwd|api[_-]?key|private[_-]?key`). The scanner runs in-process over head content of changed text files; line membership in hunks sets the state. Alternative: require `gitleaks`. Rejected because the scan must work with no tools installed, which is the zero-setup promise of the deterministic layer. A `gitleaks`-backed scanner can be added as an adapter tool later without changing finding shapes.

### D6. Dependency audit through OSV batch queries, changed packages only

Lockfile parsers for the formats listed in the deterministic-checks spec produce `(ecosystem, name, version)` sets for base and head. Only packages whose resolved version differs are sent to `POST https://api.osv.dev/v1/querybatch` in batches of 100 with a 10-second timeout and no retries beyond one. The response is mapped to one finding per advisory. `requirements*.txt` entries without an exact `==` pin are reported in the footer as unauditable rather than guessed. Rationale for changed-only: unchanged vulnerable dependencies are pre-existing issues, which the product principle keeps out of a change review; a full audit belongs to a later `rio audit` command.

### D7. Profile engine: validate as JSON, merge as values, type afterwards

Profiles are loaded with `serde_yaml` into a `serde_json::Value`, validated against `schemas/profile-v1.json` with the `jsonschema` crate, inherited by value-level merge (mappings deep, sequences replaced), semantically validated (DAG, module refs, `with` against each module's `schema` block, guardrails), then deserialized into typed structs. Validating before typing yields error messages with YAML paths; merging at the value level makes the inheritance rule trivially specified and testable. The rule expression language gets a hand-written recursive-descent parser producing an AST that v0.2 will evaluate; v0.1 only parses.

Built-in modules are embedded in the binary with `include_dir!` from `modules/`, so a single binary carries `rio/base` and every `rio/*` manifest and the profile engine needs no filesystem lookup for built-ins. Module manifest v1 fields: `kind`, `name`, `version`, `description`, `schema` (JSON Schema for `with`), and kind-specific blocks (`languages`, `capabilities`, `tools` for adapters; `inputs`, `outputs`, `local: true|false` for steps). `local` is the flag the engine uses to decide execute-versus-defer.

`rio/base` v0.1 declares the full TRD step list (prepare, checks, graph, triage, agents, verify, report) with `rio/checks` options `{ typecheck: true, lint: true, secrets: true, coverage: true, dependencies: true, baseline: true }`, budgets `fast: { wall_time: 4m }`, `fail_on` defaults, `min_evidence: plausible`, `max_lines: 1500`.

### D8. Symbol tables and references with tree-sitter plus scoped text search

`rio-graph` builds per-file symbol tables with tree-sitter queries shipped in each adapter module (`queries/symbols.scm`, `queries/references.scm`), keyed in memory by file content hash so a file parsed for the head slice is not re-parsed for a second seed. Reference candidates are found by an Aho-Corasick scan of seed identifiers over all non-excluded files walked with the `ignore` crate; each candidate file is parsed and only identifier nodes outside string and comment nodes count. Import resolution (TypeScript relative imports and `tsconfig.json` `paths`; Python dotted modules against project roots) upgrades an edge to `resolved` when the referencing file imports the seed's defining module. Expansion is breadth-first with a node cap. Alternative: a full index of every symbol in the repository. Rejected for v0.1; it is the v0.4 background graph.

### D9. One event stream drives every renderer

`rio-engine` emits `ReviewEvent` values (`StepStarted`, `CheckFinished { kind, root, findings, skipped }`, `GraphReady`, `FindingVerified` reserved, `Done { report }`, `Partial`) on a channel. The terminal renderer consumes them live; the Markdown and JSON renderers consume the final `Report`. The event enum is defined in `rio-core` with `serde` derives using the field names the TRD's SSE endpoint will carry, so v0.2's `rio-client` deserializes server events into the same type.

Progress goes to standard error through `indicatif`; findings go to standard output. Interrupt handling uses a `CancellationToken`: first signal cancels in-flight tool processes (`kill_on_drop`) and flips the report to partial; second signal exits 130.

### D10. Finding identity normalization

`normalized_location = path + "::" + symbol_path` (no lines). `trigger_hash = blake3(normalized_message)` where normalization lowercases, replaces runs of digits with `#`, strips absolute path prefixes and the repository root, and removes tool-generated identifiers matching `[A-Za-z]+_[0-9a-f]{6,}`. The id is `blake3(rule_id | normalized_location | trigger_hash)` rendered as 16 hex characters in reports and in full in JSON. A property test asserts invariance under line insertion and function reordering within a file.

### D11. Review id, cache and pruning

`review_id = blake3(snapshot_key | blake3(profile.resolved.yaml))[..16]`. Cache layout follows the review-report spec. Pruning deletes `reviews/<id>` directories older than the configured age and baselines whose base commit is no longer reachable from any local ref; snapshots not referenced by a remaining review are deleted. Pruning runs at the start of `rio review` and is bounded to one second of work per run.

### D12. Concurrency and timeouts

`tokio` multi-threaded runtime; each check is a task; tool processes are `tokio::process::Command` with `kill_on_drop(true)` and a per-check timeout from `budget.fast.wall_time` apportioned as: typecheck and lint 40 % each of the budget at most, coverage 60 %, secrets and dependency audit 10 %; the whole run is bounded by the budget. Default parallelism is the number of physical cores, capped at 4 for tool processes to avoid starving the user's machine.

### D13. Configuration and endpoint detection

`~/.config/rio/config.toml` is read if present; v0.1 only looks at `endpoint.url`. If set, `rio review` prints that endpoints are not supported by this version and continues locally; if absent, local mode is silent. This keeps the v0.2 `rio init` output file compatible.

### D14. Testing strategy

- Unit tests per crate; `insta` snapshot tests for diff, graph slices and all three report formats on the fixture repositories.
- `proptest` for identity stability (D10) and for inheritance merge laws (merging with an empty child is identity; lists replace).
- `assert_cmd` integration tests that run the built binary against `tests/fixtures/*` for every CLI scenario in the specs, including exit codes and `NO_COLOR`.
- Tool-dependent tests detect the tool at runtime and are marked skipped with a reason when absent; CI installs Node, pnpm, Python, `pyright`, `mypy`, `ruff`, `coverage` so they always run there.
- Recorded tool outputs (one fixture per supported tool version) drive parser tests offline.

## Risks / Trade-offs

- [`gix` lacks an operation, for example untracked-file enumeration with ignore rules] → `Repo` trait (D2) allows a `git2` implementation; the `ignore` crate already handles ignore rules for the walk.
- [Baseline doubles type-check time on large repos] → cached per base commit (D3); `baseline: false` in the profile; `tsc --incremental` with a `tsbuildinfo` kept in the baseline cache as a follow-up.
- [Materialized base tree does not match base dependencies] → footer note; `baseline_failed` fallback keeps findings visible as `unclassified` rather than hiding them.
- [Text-based reference search over-connects common identifiers such as `get` or `run`] → scope filtering, import resolution for confidence, node cap with a truncation note, and identifiers shorter than three characters are only matched when import-resolved.
- [Running the project's test suite executes project code on the user's machine] → identical to the user running their tests; `coverage: false` disables it; documented in the README. Isolation is the runner's job in v0.2.
- [Tool output formats drift between versions] → machine-readable formats where available (D4); parser fixtures per version; `unparseable_output` degradation.
- [Secrets heuristic false positives in fixtures and tests] → path suppressions in the profile; the catalog carries per-pattern test cases.
- [Symlinks and long paths on Windows] → Windows is best effort in v0.1; baseline falls back to `baseline_failed` when symlinking fails.
- [Scope creep toward v0.2] → anything requiring a model or endpoint is rejected at the CLI with an explicit message, and the deferred list in the footer is the only place it appears.

## Migration Plan

Greenfield; nothing to migrate. Contracts frozen by this change and extended, never broken, by later ones: `schemas/profile-v1.json`, `schemas/module-v1.json`, `schemas/finding-v1.json`, `schemas/report-v1.json`, the `ReviewEvent` enum, the review cache layout, the `~/.config/rio/config.toml` shape.

## Open Questions

- Entropy threshold and minimum length for the secrets heuristic will be tuned against the fixture set; the spec only requires redaction and classification.
- Whether to ship a `rio/minimal` built-in profile (checks only, no graph) in v0.1; it is additive and can land in a follow-up.
- Whether baseline lint should run on the base version of changed files only (current plan) or on the full base tree; the former is cheaper and matches lint's per-file nature.
