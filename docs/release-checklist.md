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

- Corrected commit `1a534ec366bd558a76f2e48b35af8b133c8a1ae7`: [CI run 41906943826](https://github.com/mctlisboa/rio-v01-validation-20261104/actions/runs/41906943826) passed Ubuntu and Windows, including both PowerShell shells, then the queued macOS 15 job was cancelled in favor of available macOS 26 ARM capacity. The first macOS 26 run caught an unused test variable under warnings-as-errors; the interrupt test now also checks worker survival on macOS.

- Commit `cac03d923e26de5045af2038c31cc096d9e27a95`: [CI run 41908450971](https://github.com/mctlisboa/rio-v01-validation-20261104/actions/runs/41908450971) passed Ubuntu and Windows. macOS exposed inherited nonblocking mode on the audit test server's accepted sockets. The fixture now explicitly selects blocking reads; all six focused audit tests pass locally.

## Release candidate validation

- Tag `v0.1.0-rc.1` points to `76c9b457f0ea541c145465101055d89c85c94e49` in the temporary public validation repository. [Release run 41910039900](https://github.com/mctlisboa/rio-v01-validation-20261104/actions/runs/41910039900) runs the complete CI gate before building or publishing. The repository variable `RIO_FORCE_UPLOAD_FAILURE=1` requests a deliberate upload failure for the first publication attempt.
- A local `cross 0.2.5` build of `x86_64-unknown-linux-musl` succeeded in an isolated target directory. `file` identifies a static PIE executable; `--version` reports the supplied `0.1.0-rc.1`, target and commit. `Cross.toml` passes release version and commit through to the container. Rootless local Docker required container UID/GID zero; GitHub's rootful Docker jobs use the normal defaults.
- The first RC workflow's Windows timeout fixture expired before its PID file was created, despite passing on the same commit in [branch CI 41909872295](https://github.com/mctlisboa/rio-v01-validation-20261104/actions/runs/41909872295). The Windows timeout budget is now 15 seconds, with a 30-second readiness budget for the cancellation fixture. Neither changes the one-hour worker lifetime or the requirement to confirm descendant termination. All six Linux supervision tests pass after this fixture correction.
- That RC attempt also exposed canonical-path mismatches in workspace and Python editable dependency detection on macOS (`/var` versus `/private/var`). Link discovery and compatibility now compare canonical roots. Three existing compatibility tests use an explicitly aliased worktree so Linux exercises the same case; all 13 compatibility tests pass locally. No assets were built or published by this failed attempt.
- [CI run 41911107532](https://github.com/mctlisboa/rio-v01-validation-20261104/actions/runs/41911107532) passed Ubuntu and Windows, including the adjusted timeout test. macOS passed dependency-link checks, supervision and end-to-end fixture tests, but exposed an ESLint target alias and two fixture assumptions. A separate Linux reproduction confirmed ESLint reported `File ignored because outside of base path` for an aliased absolute target. Commands now resolve the disposable root before constructing cwd and targets; the real-tool test explicitly uses an alias. Doctor's fixture compares canonical paths, and the case-only rename fixture uses `git mv -f` to update the index on case-insensitive filesystems.
