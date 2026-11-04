# v0.1 release validation

A checked box records actual evidence. Source code or a workflow definition alone does not establish that a release ran successfully.

- [ ] CI passes on the first pushed implementation commit, including a negative check with Ruff removed under `RIO_REQUIRE_TOOLS=1`.
- [ ] Unix installer checks pass on Ubuntu and macOS; PowerShell checks pass on Windows with 5.1 and 7.
- [x] Cold/warm benchmark has ten runs of each, p50/p95 and per-check breakdown, with cold p50 at most four minutes or a linked performance issue.
- [x] Ten real PRs across at least three repositories and both languages have complete independent automated verdicts and a go/no-go conclusion in `docs/evaluation/v0.1.md`.
- [ ] A fork release of `v0.1.0-rc.1` has six archives, LICENSE/README per archive, matching SHA256SUMS, tagged installers and verified provenance.
- [ ] The RC is marked pre-release and `releases/latest` is unchanged.
- [ ] Forced asset-upload failure leaves only a draft; rerun replaces that draft, while published releases cannot be modified.
- [ ] Published Unix installer succeeds on Ubuntu, Alpine and macOS arm64; `rio --version`, `rio doctor` in a fixture repository, and uninstall work.
- [ ] Published PowerShell installer succeeds on Windows x64 in both shells; version, doctor and uninstall work.
- [x] Stable tag is permitted only after the acceptance record is complete and says go.

Local validation completed during implementation is recorded in the OpenSpec task checklist. Platform and published-release evidence must include run URLs, platform/architecture, exact tag and commit, and observed results here before the corresponding gate is checked.

## Local evidence (2026-11-04)

- Rust 1.99.1 on Linux x86_64: `RIO_REQUIRE_TOOLS=1 cargo test --workspace --locked` passed 163 tests with no ignored tests. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked` passed.
- All fixture-owned TypeScript, Python and pnpm workspace tests passed. Removing the fixture's Ruff executable made the required-tool end-to-end test fail immediately; restoring it made the positive run pass.
- Unix installer: ten tests passed on Ubuntu, including a different filesystem (`/dev/shm`, mounted noexec) and rollback. ShellCheck passed. PowerShell ScriptAnalyzer passed locally; Windows execution requires CI evidence below.
- Release publisher: nine fake-GitHub lifecycle/package tests passed, including forced upload failure, draft replacement, published-release refusal, exact asset/checksum membership, corrupt download and failed provenance verification.
- Ten real PR records from Immer, pydantic-settings and structlog are complete. Independent **automated** aggregate verdict: **no-go**, with 1 useful/9 not-useful assessments, no true positives and two reproduced missed regressions. The record-only checker passes; the stable-release checker correctly rejects no-go.
- External validation repository authorized by the user: https://github.com/mctlisboa/rio-v01-validation-20261104. Release candidates and CI use this temporary public repository; no upstream stable release is authorized or claimed.

## First pushed CI run

- Commit `8ea5d63f675a9208f39fe9064a3d0599bc3e7967`: [CI run 41905465387](https://github.com/mctlisboa/rio-v01-validation-20261104/actions/runs/41905465387). The Windows supervision job passed four tests but the timeout test expired after 750 ms before Python wrote its grandchild PID. The fixture now allows three seconds; the worker still sleeps for an hour, so the test continues to verify actual timeout termination. This initial run is not reported as green.

- The same initial run's Ubuntu suite reached the Git repository tests, where a merge fixture relied on a globally configured Git identity. The merge stopped before creating a conflict. Test repositories now configure their own identity, and the conflict test verifies conflict markers before exercising Rio. This is a test-isolation correction; the original failure remains part of the first-run record.
