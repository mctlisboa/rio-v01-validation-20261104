## Why

Rio does not exist yet: the repository holds only a PRD and a TRD. The PRD's release plan makes v0.1 a local-only CLI whose deterministic layer must prove useful on its own in 10 real PRs before any backend, agent or verifier work starts. This change establishes that foundation: the Cargo workspace, the review model, the profile engine, the TypeScript and Python adapters, the lazy impact graph and the terminal report, all running on the developer's machine with no endpoint.

**Scope assumption.** The PRD and TRD describe the whole product through v1.0. One change cannot carry that, so this change is scoped to the PRD's v0.1 row. Everything that needs a model, a Cloudflare endpoint, a runner or a git host (intent inference, agents, verifier, `rio init`, PR mode, publishing, `--since-last`, feedback) is explicitly out of scope and will come as later changes. Where v0.1 needs a piece of a later feature (for example the profile schema), this change builds the piece to the TRD contract so later changes extend rather than replace it.

## What Changes

- **New Cargo workspace** `crates/` with `rio-cli`, `rio-core`, `rio-git`, `rio-profile`, `rio-adapters`, `rio-graph`, plus `modules/` (built-in `rio/*` modules and the `rio/base` profile) and `schemas/` (`profile-v1.json`, `module-v1.json`, `finding-v1.json`). `rio-client` and `rio-runner` are stubbed as empty crates to fix the workspace layout but carry no behavior.
- **`rio review`** reviews working-tree, staged, or `--base <ref>` changes with no endpoint configured. `--pr`, `--deep`, `--publish`, `--since-last` are parsed but exit with an actionable "requires an endpoint (v0.2)" message. `--offline` is accepted and disables the only network call Rio itself makes (OSV); tool and test processes are not network-isolated in v0.1, and the footer says so.
- **Change resolution and snapshots**: base/head resolution, unified diff with hunk line ranges, and a BLAKE3 content-addressed snapshot of changed files plus base SHA, so every run is pinned to immutable versions.
- **Profile engine (local subset)**: YAML profile schema v1, single inheritance via `extends` with deep-merge and list override, JSON-schema validation, module reference resolution for built-in `rio/*` modules, `suppress` and `fail_on` policies, and the non-overridable guardrails (verifier always present, `min_evidence` never below `plausible`). Steps whose module cannot run locally (agents, verifier, prepare's intent agent) are validated but not executed, and are listed in the report footer. Triage rule *syntax* is validated; rule *evaluation* that selects agents is deferred to v0.2.
- **`rio profile list|show|validate`** against repo (`.rio/profiles/`), personal (`~/.config/rio/profiles/`) and built-in profiles.
- **Language adapters** for TypeScript and Python behind one `Adapter` trait with declared capabilities (`parse`, `typecheck`, `lint`, `test`, `coverage`, `scan`) and repository detection from manifests and config files.
- **Deterministic checks**: compile/type errors, lint groups (only when a project lint config exists), secrets, diff coverage (`untested_change`), dependency vulnerabilities via OSV (`vulnerable_dependency`). Checks that can run on both versions (typecheck, lint) classify each finding as new, pre-existing or fixed, so old issues are separated from regressions. A missing tool produces a `check_skipped` footer entry, never silence.
- **Lazy impact graph**: Tree-sitter symbol tables for both versions, seed set from changed hunks, N-hop expansion through scope-filtered identifier search, base/head/delta slices written as JSON. LSP-backed reference resolution is an optional adapter capability not implemented by the v0.1 built-ins.
- **Findings and report**: a single finding model with evidence levels, stable identity `blake3(rule_id | normalized_location | trigger_hash)`, one finding per root cause, suppression application, and terminal, Markdown and JSON renderers. The report ends with a footer of what could not be analyzed. Results are cached under `.rio/cache/<review-id>/`.
- **`rio doctor`** (local subset): reports detected languages, tool availability per adapter capability, profile validity and git state.

Nothing is breaking: there is no prior behavior.

## Capabilities

### New Capabilities
- `cli/review`: the `rio review` command surface for local use: change selection flags, output formats, `--offline`, progress and partial-report behavior, exit codes from `fail_on`, and the explicit failure of endpoint-only flags.
- `cli/profile`: `rio profile list|show|validate` over repo, personal and built-in profile locations.
- `cli/doctor`: `rio doctor` diagnostics for the local pipeline: git state, adapter tool availability, profile validity.
- `change-snapshot`: base and head resolution for working-tree, staged and `--base` modes; diff with hunk ranges; content-addressed snapshot pinned to immutable versions.
- `review-profiles`: profile schema v1, discovery order, inheritance, validation, built-in module resolution, guardrails, `suppress` and `fail_on`, and what the local engine executes versus defers.
- `language-adapters`: the adapter contract, capability declaration, repository detection, and the behavior of the built-in TypeScript and Python adapters.
- `deterministic-checks`: the five check kinds, their rule ids and finding types, base/head classification, and `check_skipped` reporting.
- `impact-graph`: lazy graph construction, node and edge model, confidence marking, slice output.
- `findings`: finding model, evidence levels, stable identity, root-cause grouping, regression vs pre-existing state, suppression.
- `review-report`: terminal, Markdown and JSON rendering, section order, ordering within sections, footer, result cache.

### Modified Capabilities
None. There are no existing specs.

## Impact

- **New code**: everything under `crates/`, `modules/`, `schemas/`, and a `tests/fixtures/` directory with small TypeScript and Python sample repositories carrying seeded issues.
- **External tools invoked on the user's machine** (never bundled): `tsc`, `eslint`, `vitest`/`jest`, `c8`, `pyright` or `mypy`, `ruff`, `pytest`, `coverage`. Each is optional; absence degrades to `check_skipped`.
- **Network**: one outbound call to the OSV batch API for dependency audit, disabled by `--offline`. No Rio backend, no model provider.
- **Dependencies** (Rust): `clap`, `serde`, `serde_yaml`, `serde_json`, `jsonschema`, `gix` (fallback `git2`), `blake3`, `tree-sitter` with the TypeScript and Python grammars, `tokio`, `reqwest`, `indicatif`, `globset`, `ignore`.
- **Contracts fixed for later changes**: profile schema v1 (including `fail_on`, which the TRD mentions but omits from its schema listing; this change adds it as a top-level key), module manifest v1, finding JSON v1, snapshot format, cache layout. v0.2 extends these; it must not break them.
- **Deferred to later changes**: endpoint, `rio init`/`deploy`/`upgrade`, intent agent, agents, hypothesis board, verifier, consolidation across runs, PR mode, SARIF, feedback, `--since-last`, LSP reference resolution, third-party module registry, GitHub Actions runner.
