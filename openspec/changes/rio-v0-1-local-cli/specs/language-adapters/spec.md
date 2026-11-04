## Purpose

Language adapters are the modules that teach Rio how a language's projects are laid out, which tools prove things about them, and how to turn tool output into findings; TypeScript and Python ship built in.

## ADDED Requirements

### Requirement: Adapter contract
An adapter SHALL declare, in a `module.yaml` manifest of kind `adapter`: the languages it handles, the file patterns that belong to it, the capabilities it implements from the set `parse`, `typecheck`, `lint`, `test`, `coverage`, `scan`, and for each capability the tools it can use in preference order, with the network domains each tool needs and the tool's exit-code contract (which codes mean completed without diagnostics, which mean completed with diagnostics; every other code is a failure). The engine SHALL only schedule a capability the adapter declares and for which a usable tool was resolved.

#### Scenario: Undeclared capability is never scheduled
- **WHEN** an adapter declares `parse` and `typecheck` only
- **THEN** the engine reports `lint`, `coverage` and `scan` for that language as skipped with reason `not_supported_by_adapter`, and invokes no lint tool

### Requirement: Repository detection and project roots
The engine SHALL detect languages from manifests and configuration files (`package.json` or `tsconfig.json` for TypeScript; `pyproject.toml`, `setup.py`, `setup.cfg`, `requirements*.txt` or `Pipfile` for Python) and from the extensions of changed files. For each changed file it SHALL determine a project root as the nearest ancestor directory containing that language's manifest, so tools run with the correct working directory. A repository with several project roots (monorepo) SHALL be handled by running tools once per scheduled root.

The engine SHALL discover workspaces from `package.json` `workspaces`, `pnpm-workspace.yaml`, and `[tool.uv.workspace]` in `pyproject.toml`. Each member root SHALL be associated with its workspace root, whose lockfile and installation govern the member when the member has neither of its own. It SHALL build a root dependency graph from workspace dependencies between members, TypeScript project `references`, and Python path or editable dependencies on other roots. The scheduled roots for `typecheck` and `coverage` SHALL be the roots that contain changed files, plus every root that transitively depends on one of them. Roots added this way SHALL be labelled `dependent` in the report. `lint` runs only on changed files, so it is scheduled only for roots that contain changed files.

#### Scenario: Monorepo with two TypeScript packages
- **WHEN** changes touch `packages/api/src/a.ts` and `packages/web/src/b.ts`, each package having its own `tsconfig.json`
- **THEN** the type check runs once in `packages/api` and once in `packages/web`, and findings carry repository-relative paths

#### Scenario: Shared package change checks its consumers
- **WHEN** a change touches only `packages/shared/src/format.ts`, and `packages/api` declares `"@acme/shared": "workspace:*"` while `packages/web` does not depend on it
- **THEN** the type check runs in `packages/shared` and in `packages/api` (labelled `dependent`), does not run in `packages/web`, and lint runs only in `packages/shared`

#### Scenario: Member root uses the workspace lockfile
- **WHEN** `packages/api` has a `package.json` but no lockfile, and the repository root has `pnpm-workspace.yaml` and `pnpm-lock.yaml`
- **THEN** dependency compatibility for `packages/api` is verified against the `packages/api` importer in the root `pnpm-lock.yaml`

#### Scenario: Changed file without a manifest
- **WHEN** a `.py` file is changed in a directory tree with no Python manifest
- **THEN** Python is still detected, the repository root is used as the project root, and the footer notes that no manifest was found

### Requirement: Tool resolution
For each capability the adapter SHALL prefer a project-local tool (`node_modules/.bin/`, a virtual environment's `bin/` detected from `.venv`, `venv` or the `VIRTUAL_ENV` variable) over a tool on `PATH`, SHALL record the resolved path and version in the review metadata, and SHALL never download or install a tool.

#### Scenario: No tool available
- **WHEN** no `tsc` exists in the project or on `PATH`
- **THEN** the `typecheck` capability is skipped with reason `tool_missing` and an install hint, and the review continues

### Requirement: Excluded paths
Adapters and checks SHALL skip vendored directories (`node_modules`, `.venv`, `venv`, `vendor`, `dist`, `build` unless they contain changed source tracked by git), files marked `linguist-generated` in `.gitattributes`, and files matching the profile's exclusion globs (`rio/prepare` `with: { exclude: [<glob>...] }`, resolved relative to the repository root). Excluded files SHALL be listed in the report footer with their reason.

#### Scenario: Generated file skipped
- **WHEN** a changed file matches a `linguist-generated=true` attribute
- **THEN** no check or graph step processes it and the footer lists it as excluded with reason `generated`

### Requirement: Built-in TypeScript adapter
The TypeScript adapter SHALL handle `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs` files. It SHALL implement `parse` (symbol tables for functions, methods, classes, interfaces, type aliases, enums, exported variables, imports and exports), `typecheck` via `tsc --noEmit` using the project root's `tsconfig.json`, `lint` via `eslint` only when an ESLint configuration file is present in the project root or an ancestor, `test` and `coverage` via `vitest` or `jest` as detected from the manifest's dependencies and config files, with coverage output in LCOV or Istanbul JSON, and `scan` via the built-in secrets scanner.

#### Scenario: Lint requires a project config
- **WHEN** `eslint` is installed but no ESLint configuration exists
- **THEN** `lint` is skipped with reason `no_project_config` and the footer explains that style checks only run against the project's own rules

#### Scenario: Type errors reported with locations
- **WHEN** `tsc` reports `src/a.ts(12,5): error TS2322: ...`
- **THEN** a finding with rule id `typecheck/tsc/TS2322`, path `src/a.ts`, line 12, column 5 and the message text is produced

### Requirement: Built-in Python adapter
The Python adapter SHALL handle `.py` and `.pyi` files. It SHALL implement `parse` (modules, functions, methods, classes, assignments at module scope, imports), `typecheck` via `pyright` or `mypy`, choosing the one configured in `pyproject.toml`, `pyrightconfig.json`, `mypy.ini` or `setup.cfg`, and otherwise `pyright` if available else `mypy`, `lint` via `ruff` only when a Ruff configuration is present, `test` and `coverage` via `pytest` with `coverage.py` producing JSON, and `scan` via the built-in secrets scanner.

#### Scenario: Configured type checker wins
- **WHEN** `pyproject.toml` contains a `[tool.mypy]` table and both `pyright` and `mypy` are installed
- **THEN** `mypy` is used for `typecheck`

#### Scenario: Ruff output mapped to findings
- **WHEN** `ruff` reports `F401` on `pkg/mod.py` line 3
- **THEN** a finding with rule id `lint/ruff/F401`, path `pkg/mod.py` and line 3 is produced

### Requirement: Output normalization
Every tool invocation SHALL be converted into findings with repository-relative path, line, optional column, rule id and message, and into one check outcome as defined by the deterministic-checks capability. When a tool's output cannot be parsed, the outcome SHALL be `failed` with reason `unparseable_output`, and the raw output SHALL be kept in the review cache for inspection. Tool exit codes alone SHALL NOT be treated as findings, and they SHALL NOT be ignored either: the adapter's exit-code contract decides whether the run completed.

#### Scenario: Unexpected tool output
- **WHEN** `tsc` prints output in a format the adapter does not recognize
- **THEN** the `typecheck` outcome is `failed` with reason `unparseable_output`, the raw output is saved under the review cache, and the run continues

#### Scenario: Diagnostic exit code without parsed findings
- **WHEN** `ruff` exits 1, which its contract defines as "completed with diagnostics", but its JSON output contains no diagnostics
- **THEN** the `lint` outcome is `failed` with reason `unparseable_output`, not `passed`
