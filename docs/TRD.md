# Rio — Technical Requirements Document (TRD)

Nov 4, 2026 · @Marcos Lisboa

This document specifies how Rio is built: a Rust CLI, a Cloudflare control plane that interprets YAML review profiles, pluggable runners that execute untrusted code, and an evidence-gated pipeline of deterministic checks, agents and an independent verifier. It implements the requirements in the PRD.

## System overview

&#91;embedded content: system architecture · control plane on Cloudflare, pluggable runner\]

The control plane (Worker API, Workflows, D1, R2) lives in the user’s Cloudflare account and never executes repository code. Execution of untrusted code — checkout, build, indexing, scanners, tests, verifier reproductions — happens in a runner chosen at setup, submitted as a job and resumed through a signed event.

| Component | Technology | Responsibility |
| --- | --- | --- |
| CLI | Rust, single binary | Diff and snapshot, local deterministic checks, submit review, render report, wizard and deploy |
| Worker API | Cloudflare Workers, TypeScript (Hono) | Authentication, review submission, status, report delivery, webhook ingestion, PR publishing |
| Workflow engine | Cloudflare Workflows + Queues | Interprets the resolved profile: durable steps, retries, waits for runner events, agent fan-out |
| Runner | Cloudflare Sandbox, self-hosted Rust binary, GitHub Actions | Executes jobs in isolation; writes results to R2; emits completion events |
| Storage | D1 (SQLite), R2 (objects) | Reviews, findings, feedback, resolved profiles; snapshots, indexes, logs, evidence |
| Model gateway | Worker module | Calls configured providers with per-task models; enforces token budgets; records usage |

The Rust code is shared between the CLI and the self-hosted runner (diffing, snapshotting, Tree-sitter parsing, adapter execution) so the local and remote pipelines behave identically.

## CLI design (Rust)

A Cargo workspace; the CLI and the self-hosted runner are thin binaries over shared crates.

| Crate | Purpose | Key dependencies |
| --- | --- | --- |
| `rio-cli` | Commands, TTY rendering, wizard | `clap`, `ratatui`/`indicatif`, `keyring` |
| `rio-core` | Review model: findings, evidence levels, finding identity, report rendering (terminal, md, json, sarif) | `serde`, `serde_json` |
| `rio-git` | Diff, base resolution, content-addressed snapshots, PR metadata | `gix` (fallback `git2`) |
| `rio-profile` | YAML profile parsing, inheritance resolution, JSON-schema validation, triage rule evaluation | `serde_yaml`, `jsonschema` |
| `rio-adapters` | Language adapter trait and built-ins (TypeScript, Python); Tree-sitter parsing; running project tools | `tree-sitter`, `tokio`, `which` |
| `rio-graph` | Lazy impact graph: symbol tables, reference resolution via LSP/Tree-sitter, before/after comparison | `tree-sitter`, `lsp-types` |
| `rio-client` | HTTP client for the Worker API, auth tokens, streaming status | `reqwest`, `tokio` |
| `rio-runner` | Self-hosted runner: job polling, sandboxed execution, R2 upload, event signing | `tokio`, `bollard` (Docker) or `nsjail` wrapper |

**Local pipeline (`rio review`)**

1. Resolve base and head: working tree, staged index, `--base` ref or PR head. Working-tree and staged states are materialized into a **snapshot**: a tarball of changed files plus base commit hash, addressed by its BLAKE3 hash.
2. Detect repo configuration: languages, package managers, test runners, existing lint/format configs, `.rio/` directory.
3. Resolve the profile (`--profile`, `.rio/profiles/default.yaml`, or `rio/base`) and validate it.
4. Run deterministic checks locally when the adapter supports it and `--offline` or no endpoint is configured; otherwise delegate to the runner. Results stream to the terminal as they arrive.
5. Submit `{snapshot_ref, base, head, resolved_profile, intent_input}` to the Worker; subscribe to status events (SSE); render findings progressively.
6. On completion, render the report, exit with a code from the profile’s `fail_on` policy, and cache the result under `.rio/cache/<review-id>`.

`rio review` must produce useful output with no endpoint at all (v0.1): the deterministic layer and the lazy graph are fully local.

## Profile schema and module contract

**Profile schema (version 1)**, published as JSON Schema at `schemas/profile-v1.json`:

```yaml
version: 1
name: string
extends: <module ref>              # optional; single inheritance, deep-merge with list override
budget:
  fast: { wall_time: duration, tokens: int, agents_max: int }
  deep: { wall_time: duration, tokens: int, agents_max: int }
models:
  <alias>: <provider>/<model>      # 'verifier' alias must differ from 'default'
steps:
  - id: string
    uses: <module ref>             # rio/<name> or <org>/<name>@<semver|git-sha>
    needs: [step ids]              # DAG; cycles rejected at validation
    parallel: bool
    with: { ... }                  # module-specific, validated by the module's own schema
    rules: [ { when: <expr>, then: <action> } ]   # triage only
    agents: [ { <name>: { model: alias, ... } } ]  # agents step only
suppress: [ { path: glob } | { rule: pattern } | { finding: id } ]
publish: never | ask | auto
```

Rule expressions are a small, non-Turing-complete language evaluated by `rio-profile`: predicates `touches(glob)`, `adds_dependency()`, `lines_changed > n`, `risk >= level`, `language == x`; actions `enable(agent)`, `disable(agent)`, `budget(fast|deep)`, `require_human()`.

**Module contract.** A module is a directory with a `module.yaml` manifest and, depending on kind, prompts or a binary:

| Kind | Manifest declares | Executes where |
| --- | --- | --- |
| `adapter` | Languages, capabilities (parse, typecheck, lint, test, coverage, index, scan), commands, network domains needed | Runner |
| `agent` | Prompt template, allowed tools, output schema, default model alias, budget hints | Workflow step calling the model gateway |
| `runner` | Job protocol version, resource limits, supported features | Outside the control plane |
| `step` | Input/output schema for a pipeline step (`rio/prepare`, `rio/triage`, …) | Workflow step |

Module refs resolve from the built-in set (`rio/*`), a git registry (`registry.rio.dev` mirrorable), or a local path. Third-party modules are pinned by version or SHA and their resolved manifest is stored with the review. v0.x modules are declarative (YAML + prompts); whether adapters may ship WebAssembly logic is an open question (PRD).

## Workflow engine on Cloudflare

One Cloudflare Workflow instance per review (`ReviewWorkflow`), parameterized by the review id. The Workflow does not contain review logic; it walks the resolved profile’s step DAG.

**Execution model**

1. Load the resolved profile and module manifests from D1/R2.
2. Topologically sort `steps` by `needs`; steps marked `parallel` with no mutual dependency run as concurrent `step.do` calls, or are fanned out through a Queue when more than the configured concurrency.
3. Each step is one of three shapes:
   - **Inline step** (prepare, triage, report): runs in the Worker, reads inputs from R2 by reference, writes outputs to R2, records a row in D1.
   - **Runner job** (checks, graph, reproductions): writes a job record, enqueues it for the runner, then `step.waitForEvent("job:<id>")` with a timeout from the budget. The runner’s signed completion event resumes the step with result references.
   - **Model step** (agents, verifier): calls the model gateway with the prompt, tools and budget; tool calls that need code execution become nested runner jobs.
4. Budgets are enforced at two levels: a per-step timeout and a review-level token and wall-time ledger kept in D1; exceeding the ledger cancels remaining optional steps and marks the review `partial`.
5. On completion the report step consolidates findings and emits `review:done`; the CLI’s SSE subscription and PR publisher react to it.

**Idempotency and retries.** Every step and job carries `review_id + step_id + attempt`; outputs are written to R2 under that key, so a retried step overwrites rather than duplicates. PR comments carry a hidden marker with the finding identity and are edited in place. Before publishing, the Worker checks that the PR head SHA still matches the reviewed head.

**Limits to respect** (verify current values before implementation): Workflow step result size, event payload size, maximum concurrent instances per account, Queue message size. Anything beyond a few kilobytes is stored in R2 and passed by key.

## Runner interface and implementations

A runner is anything that can execute a job description and report back. The protocol is deliberately simple so that new backends are a few hundred lines.

**Job** (JSON, written to D1 and delivered via Queue or polling):

```json
{
  "job_id": "...", "review_id": "...", "attempt": 1,
  "snapshot": { "r2_key": "snapshots/<hash>.tar.zst", "base_sha": "...", "head_sha": "..." },
  "kind": "checks" | "graph" | "reproduce" | "index",
  "adapter": "rio/adapter-typescript@1.2.0",
  "commands": [ { "id": "typecheck", "argv": ["npx","tsc","--noEmit"], "cwd": "." } ],
  "limits": { "cpu_seconds": 600, "memory_mb": 4096, "wall_seconds": 900 },
  "network": { "allow": ["registry.npmjs.org"] },
  "upload_token": "<short-lived R2 token scoped to results/<job_id>/>"
}
```

**Completion event** (POSTed to the Worker, signed with the runner’s registration key): `{ job_id, attempt, status: ok|failed|timeout, results: { <command id>: { exit_code, r2_key } }, metrics }`.

| Implementation | How it runs jobs | Fit |
| --- | --- | --- |
| Cloudflare Sandbox | Workflow spawns a sandbox, streams the snapshot in, runs commands, uploads results | Small and mid-size repos; zero setup; bounded by Sandbox CPU/memory/duration limits |
| Self-hosted (`rio runner serve`) | Rust binary polls the Worker for jobs; executes each in a fresh container (Docker) or namespace jail; uploads to R2 | Large builds, monorepos, private registries, GPU-free heavy toolchains |
| GitHub Actions (v0.4) | Workflow dispatches a repository workflow with the job id; the action runs the same Rust runner in `--one-shot` mode | Teams that already pay for Actions minutes; fork PRs under GitHub’s permission model |

All runners share the same executor crate, so a job produces identical results regardless of backend. A runner advertises its capabilities on registration (max limits, network policy, toolchains preinstalled); triage will not schedule a job a runner cannot honor.

## Review pipeline internals

**Prepare (`rio/prepare`).** Inputs: diff, base and head snapshots, PR metadata, `.rio/` config. Detects languages and frameworks from manifests and adapter probes; marks excluded paths (vendored, generated, lockfiles handled separately). Runs the **intent agent** on the cheap model: output is a JSON list of `{behavior, evidence_lines, confidence_kind: declared|inferred}`. In CLI mode the list is shown for confirmation; confirmed behaviors become the `intent` artifact every later step receives. A diff above `max_lines` (profile) is partitioned by directory and dependency cluster into logical parts reviewed separately.

**Checks (`rio/checks`).** One runner job per adapter. Each command maps to a deterministic finding type with a stable rule id. Diff coverage: run the project’s test command with coverage enabled, then intersect coverage data with added/modified line ranges → `untested_change` findings per function. Dependency audit: diff of manifest/lockfile resolved versions → OSV API query (batched) → `vulnerable_dependency` findings; new dependencies with install scripts or very recent first publish are flagged for the supply-chain agent. Tool unavailable → `check_skipped` entry in the report footer, never silence.

**Graph (`rio/graph`, lazy mode).** Seed set = symbols enclosing every changed hunk in both versions. Expand outward `hops` times: definitions via Tree-sitter symbol tables, references via LSP `textDocument/references` where a language server is available, else via indexed identifier search filtered by Tree-sitter scope. Nodes: file, function, class, endpoint, table, event, config, test. Edges carry `{kind, source: lsp|tree-sitter|text|framework|contract, confidence: resolved|probable|unresolved, version: base|head}`. Removed symbols from base are kept so agents can see what disappeared. Output: two graph slices (base, head) plus a delta, stored in R2 as JSON.

**Triage (`rio/triage`).** Evaluates profile rules against the prepare, checks and graph outputs to produce the execution plan: which agents run, with which budget and which context slice. Risk is computed from signals, not guessed: touched sensitive paths, public contract changes, missing coverage on changed logic, dependency changes, graph fan-out. The plan is stored and shown in the report (“why these agents ran”).

**Full graph (v0.4).** Background Workflow per repository builds per-file symbol tables keyed by file content hash; a commit’s graph is the union of its files’ tables plus a cross-file edge index. Changed files only recompute their own table; dependency or config changes invalidate the affected component.

## Agents, hypothesis board and verifier

**Agent runtime.** An agent is a model step with a bounded tool set and a structured output. Tools: `read_file(path, version)`, `graph_query(symbol, direction, hops)`, `search(pattern, scope)`, `run(command)` (becomes a runner job, only if the agent manifest allows), `board_read()`, `board_write(hypothesis)`. Each agent receives: the intent artifact, deterministic findings, its context slice from triage, and its budget. Output schema: a list of **hypotheses** `{id, location, trigger, observed, expected, impact, relation_to_change, graph_path, evidence[], suggested_fix, limitations}`.

**Hypothesis board.** A D1 table scoped to the review. Before investigating a location an agent reads the board; if another agent holds a hypothesis on the same symbol it either attaches evidence to it or skips. This moves deduplication before consolidation and lets agents hand off (contracts finds a consumer; correctness checks that consumer’s assumption).

**Built-in agents** (each a `rio/agent-*` module): intent, correctness, contracts, security (OWASP Top 10:2025 categories, ASVS 5.0 requirements as checklists; traces input → validation → authorization → transformation → sensitive operation), data-concurrency, supply-chain, performance, tests.

**Verifier (`rio/verify`).** Independent by construction:

- Uses the `verifier` model alias, which validation requires to differ from `default`.
- Receives the hypothesis and the code, not the generating agent’s reasoning.
- Has tools the generators lack: run tests or scripts on **both** versions, query the base graph, diff behavior.

For each hypothesis it establishes: introduced or worsened by the change; path reachable; no protection elsewhere; consistent with real contracts; enough evidence for a fix. When feasible it writes a minimal reproduction or regression test and runs it against base and head; the test must assert legitimate behavior, not restate the hypothesis. Result: an **evidence level** — `reproduced`, `static_proven`, `plausible`, `refuted` — with the artifacts that justify it. Only hypotheses at or above the profile’s `min_evidence` (never below `plausible`) become findings; `plausible` ones are rendered as risks for human judgment, never as defects. Agreement between agents is not evidence.

**Consolidation (`rio/report`).** Groups findings by root cause (same symbol path and trigger), assigns stable identity `blake3(rule_id | normalized_location | trigger_hash)`, diffs against the previous review of the same PR to set state transitions, applies suppressions, orders by impact × evidence, renders.

## Data model

**D1 tables** (SQLite; migrations shipped with the Worker and applied by `rio init` / `rio deploy`):

| Table | Key columns | Notes |
| --- | --- | --- |
| `repos` | id, host, owner, name, default\_profile, settings\_json | One row per repository seen |
| `reviews` | id, repo\_id, base\_sha, head\_sha, snapshot\_key, profile\_resolved\_key, budget\_mode, status, cost\_tokens, cost\_runner\_s, created\_at, finished\_at | `status`: queued, running, partial, done, failed |
| `steps` | review\_id, step\_id, attempt, status, input\_key, output\_key, started\_at, ended\_at | Workflow bookkeeping |
| `jobs` | id, review\_id, step\_id, attempt, runner\_id, kind, status, result\_key, metrics\_json | Runner protocol |
| `hypotheses` | id, review\_id, agent, symbol, status, payload\_key | The hypothesis board |
| `findings` | id (stable identity), review\_id, rule\_id, evidence\_level, state, location\_json, payload\_key, first\_seen\_review, last\_seen\_review | `state`: open, fixed, accepted, false\_positive |
| `feedback` | finding\_id, actor, verdict, reason, created\_at | Reasons: false\_positive, irrelevant, known, style |
| `suppressions` | repo\_id, kind, pattern, origin (manual, proposed), approved\_by | Proposed suppressions need approval |
| `runners` | id, kind, capabilities\_json, public\_key, last\_seen | Registration and event verification |
| `tokens` | id, scope, hash, created\_at, revoked\_at | CLI and webhook auth |
| `usage` | review\_id, model, prompt\_tokens, completion\_tokens, cost\_estimate | Per-model ledger |

**R2 layout**

```text
snapshots/<blake3>.tar.zst              content-addressed source snapshots
reviews/<id>/profile.resolved.yaml       exact profile and module manifests used
reviews/<id>/steps/<step>/<attempt>/     step inputs and outputs (JSON)
reviews/<id>/graph/{base,head,delta}.json
reviews/<id>/evidence/<finding>/         reproductions, test output, traces
results/<job_id>/<command>/              runner uploads (stdout, reports, coverage)
indexes/<repo>/files/<content_hash>.json per-file symbol tables (full graph, v0.4)
cache/<hunk_hash>/<context_hash>.json    agent result cache
```

Retention is configurable per repo (default 90 days for evidence, indefinite for findings and feedback). Snapshots are deleted when no review references them.

## APIs

All endpoints are served by the Worker; JSON over HTTPS; bearer tokens with scopes.

**CLI → Worker**

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/v1/snapshots` | Presigned R2 upload for a snapshot (`blake3` as key; returns 200 if already present) |
| POST | `/v1/reviews` | Submit `{repo, base_sha, head_sha, snapshot_key, profile, budget, intent_input}` → `{review_id}` |
| GET | `/v1/reviews/{id}` | Status, plan, costs, finding summary |
| GET | `/v1/reviews/{id}/events` | Server-sent events: step started/finished, finding verified, done |
| GET | `/v1/reviews/{id}/report?format=` | `terminal`, `md`, `json`, `sarif` |
| POST | `/v1/reviews/{id}/intent` | Author confirms or edits inferred behaviors (CLI mode) |
| POST | `/v1/reviews/{id}/publish` | Publish or update PR comments |
| POST | `/v1/findings/{id}/feedback` | `{verdict, reason}` |
| GET/PUT | `/v1/profiles/{name}` | Team profiles stored on the endpoint (optional; repo files take precedence) |
| GET | `/v1/version` | Endpoint version and supported CLI range |

**Git host → Worker (webhooks).** `POST /v1/hooks/github` and `/v1/hooks/gitlab`, signature-verified. Events handled: PR opened, synchronized (new commits), reopened, comment with `/rio review` or `/rio dismiss <id> <reason>`. Fork PRs are flagged and follow the restricted policy in Security.

**Runner ↔ Worker.** `POST /v1/runners/register` (one-time token → runner id + key pair), `GET /v1/runners/{id}/jobs` (long-poll, self-hosted only), `POST /v1/jobs/{id}/events` (signed completion event). Sandbox runners are driven directly by the Workflow and do not use these.

**Model gateway (internal).** One interface `complete(alias, messages, tools, budget)` with provider adapters for Anthropic, OpenAI-compatible and OpenRouter; records usage per review; enforces the review token ledger; strips repository content from logs.

Errors follow RFC 9457 problem details. Version negotiation: the CLI sends `Rio-CLI-Version`; the Worker returns `426 Upgrade Required` with the `rio deploy` or CLI upgrade instruction when outside the supported range.

## Security model

Rio executes code it does not control and generates code (reproductions) it then executes. The following are enforced by the engine and the runner protocol and cannot be relaxed by a profile.

| Threat | Control |
| --- | --- |
| Malicious code in the reviewed change escapes the runner | Fresh container or sandbox per job; no network by default; adapter-declared domains only, approved in the profile; CPU, memory and wall-time limits; results only via a short-lived R2 token scoped to `results/<job_id>/` |
| Secrets leak to the runner | Runner receives the snapshot and the upload token only; never repository secrets, model keys or Worker secrets |
| Fork PRs exfiltrate or abuse resources | Fork PRs run static-only (no test execution, no network, no agents with `run`), never publish automatically, and require a maintainer `/rio review` to escalate |
| Prompt injection via repository content | All repository text, PR descriptions, tool output and logs are passed to models as data in delimited blocks; policy comes only from the resolved profile; agents have no write tools; publishing is a separate step gated by `--publish` or `publish:` policy |
| Verifier reproductions do damage | Generated code runs under the same job isolation with no network; its output is evidence, never an instruction to other steps |
| Spoofed runner events | Runners register with a one-time token and sign events with their key; the Worker checks signature, job ownership and attempt number |
| Token theft | CLI tokens stored in the OS keychain; scoped (review, admin); revocable; webhook secrets rotated by `rio doctor --rotate` |
| Supply-chain attack through modules | Third-party modules pinned by version or SHA; manifests stored with the review; registry is a git repository with signed tags |
| Data exposure | Source code stays in the user’s account except for model calls to the configured provider; evidence and snapshots have retention limits; logs never contain repository content |

Every review stores the resolved profile, module versions, model identifiers and executed commands, so any result can be audited and reproduced.

## Observability, evaluation and testing

**Observability.** Structured logs per review and step (Workers Logs), with repository content redacted. Metrics in D1 `usage` and `jobs.metrics_json`: wall time per step, tokens per model, runner seconds, cache hit rate. `rio status` and the report footer expose cost and timing to the user. Optional, opt-in telemetry sends anonymous aggregate counters to the project; no code, no findings.

**Evaluation harness** (`eval/` in the repository):

- A reference set of historical PRs with labeled known bugs, stored as patch + expected findings; private repos can run the harness locally without contributing data.
- **Synthetic bug injection**: a mutation tool (`rio eval mutate`) applies typed mutations (off-by-one, inverted condition, dropped null check, removed authorization call, swapped transaction boundary) to clean commits and records whether the pipeline finds them. This gives repeatable recall per agent.
- Metrics computed per run: precision, recall, false positives per agent and evidence level, verifier calibration (plausible → real vs. refuted), p50/p95 wall time, cost.
- Regression gate in CI: a profile or prompt change that drops precision or recall below the last release fails the build.

**Testing strategy**

| Layer | Tests |
| --- | --- |
| Rust crates | Unit tests; snapshot tests for diff/graph outputs; property tests for finding identity stability |
| Profile engine | Schema fixtures (valid and invalid); DAG and rule evaluation tests |
| Worker | Miniflare integration tests for API, webhooks, workflow stepping with mocked runners and models |
| Runner protocol | Contract tests run against every implementation (sandbox, self-hosted, Actions) |
| End-to-end | Sample repositories (TypeScript, Python) with seeded bugs, run nightly through a real deployed endpoint |

## Deployment, versioning and repository layout

**Repository layout** (monorepo, single license):

```text
rio/
  crates/           rio-cli, rio-core, rio-git, rio-profile, rio-adapters, rio-graph, rio-client, rio-runner
  worker/           Worker API, Workflows, model gateway (TypeScript, wrangler)
  modules/          built-in rio/* modules: adapters, agents, steps, base profile
  schemas/          profile-v1.json, module-v1.json, job-v1.json, finding-v1.json
  eval/             reference set tooling, mutation tool, harness
  docs/
```

**`rio init` implementation.** The wizard is in `rio-cli`; it embeds the built Worker bundle for its own version, so deploying needs no Node toolchain on the user’s machine. Steps call the Cloudflare API directly (account lookup, D1/R2/Queue creation, Worker upload with bindings, secrets). The resulting state is written to `~/.config/rio/config.toml`; secrets go to the keychain. `rio init --from config.toml` and `rio deploy` are non-interactive for CI.

**Versioning.** Semantic versions for CLI and Worker released together; the Worker exposes the supported CLI range. Profile schema versions are independent (`version: 1`) and the engine supports the previous major for one release cycle. Module versions follow semver; `rio/base` pins the module versions it was tested with.

**Upgrade path.** `rio upgrade` fetches the new CLI, then `rio deploy` applies the paired Worker and D1 migrations; migrations are forward-only and run in a Workflow with a lock in D1 so concurrent deploys cannot interleave.

**Distribution.** GitHub Releases with signed binaries (Linux, macOS, Windows), Homebrew tap, `cargo install rio-cli`; container image for the self-hosted runner.
