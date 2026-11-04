## Purpose

Review profiles are versioned YAML documents that declare what a review runs, with what budgets and policies; the engine interprets them, so review behavior is configurable and shareable rather than hard-coded.

## ADDED Requirements

### Requirement: Profile discovery order
A profile given as an existing file path SHALL be loaded from that path. A profile given by name SHALL be searched in order in `.rio/profiles/<name>.yaml`, `~/.config/rio/profiles/<name>.yaml`, then the built-in set as `rio/<name>`. When no profile is given, the default SHALL be the first of `.rio/profiles/default.yaml`, `~/.config/rio/profiles/default.yaml`, `rio/base`.

#### Scenario: Name resolves to the repo copy first
- **WHEN** `--profile security` is given and both `.rio/profiles/security.yaml` and a built-in `rio/security` exist
- **THEN** the repo file is used

### Requirement: Schema version 1
A profile SHALL declare `version: 1`. The top-level keys SHALL be `version`, `name`, `extends`, `budget`, `models`, `steps`, `suppress`, `publish`, `fail_on`, `min_evidence`, `max_lines`. Unknown top-level keys SHALL be rejected. The schema SHALL be published as `schemas/profile-v1.json` and the engine's validation SHALL accept exactly the documents that schema accepts, plus the semantic rules below.

#### Scenario: Unknown key rejected
- **WHEN** a profile contains a top-level `agents:` key
- **THEN** validation fails naming the key and its location

#### Scenario: Wrong version rejected
- **WHEN** a profile declares `version: 2`
- **THEN** validation fails stating that only version 1 is supported

### Requirement: Single inheritance
A profile MAY declare one `extends` referencing another profile by name or path. Resolution SHALL deep-merge mappings, child over parent, and SHALL replace lists wholesale when the child declares them. Inheritance cycles SHALL be rejected. The resolved profile SHALL record the full chain.

#### Scenario: Child overrides a nested budget value
- **WHEN** the parent sets `budget.fast.wall_time: 4m` and `budget.fast.tokens: 200000`, and the child sets only `budget.fast.wall_time: 2m`
- **THEN** the resolved profile has `wall_time: 2m` and `tokens: 200000`

#### Scenario: Child list replaces the parent list
- **WHEN** the parent declares three `suppress` entries and the child declares one
- **THEN** the resolved profile has exactly the child's one entry

#### Scenario: Inheritance cycle
- **WHEN** profile A extends B and B extends A
- **THEN** validation fails and names the cycle

### Requirement: Semantic validation
Beyond the schema, validation SHALL reject: step ids that are not unique; `needs` referencing unknown steps or forming a cycle; `uses` references that do not resolve to a built-in `rio/*` module (third-party refs are rejected with a message saying they are not supported in this version); `with` blocks that fail the referenced module's own schema; non-positive budgets; `models.verifier` equal to `models.default` when both are set; rule expressions that do not parse; and `min_evidence` below `plausible`.

#### Scenario: Verifier model must differ
- **WHEN** a profile sets `models.default: anthropic/claude-sonnet-5-5` and `models.verifier: anthropic/claude-sonnet-5-5`
- **THEN** validation fails stating the verifier alias must differ from the default

#### Scenario: Module option fails its schema
- **WHEN** a step uses `rio/checks` with `with: { coverage: "yes" }`
- **THEN** validation fails naming the step, the option, and the expected type

### Requirement: Guardrails cannot be overridden
A resolved profile that contains any step using `rio/agents` SHALL also contain a step using `rio/verify` that depends on it; removing or disabling the verifier through inheritance SHALL be rejected. `min_evidence` SHALL never resolve below `plausible`.

#### Scenario: Dropping the verifier is rejected
- **WHEN** a child profile redeclares `steps` with the agents step but no `rio/verify` step
- **THEN** validation fails stating the verifier step is mandatory when agents are configured

### Requirement: Rule expressions are validated syntactically
`rules` entries SHALL use the expression language with predicates `touches(glob)`, `adds_dependency()`, `lines_changed > n`, `risk >= level`, `language == x` combined with `and`, `or`, `not`, and actions `enable(agent)`, `disable(agent)`, `budget(fast|deep)`, `require_human()`. Validation SHALL reject unknown predicates or actions. In this version rules SHALL NOT be evaluated; the report footer SHALL note that triage rules are deferred.

#### Scenario: Unknown predicate
- **WHEN** a rule reads `when: owns("src/**")`
- **THEN** validation fails naming `owns` as an unknown predicate

### Requirement: Local execution subset
When no endpoint is configured, the engine SHALL execute steps whose module is locally executable (`rio/prepare` without its intent agent, `rio/checks`, `rio/graph`, `rio/report`) in `needs` order, and SHALL mark every other step (`rio/triage`, `rio/agents`, `rio/verify`) as deferred in the report without failing the run.

#### Scenario: Base profile runs locally
- **WHEN** `rio/base` is used with no endpoint
- **THEN** prepare, checks, graph and report execute, and triage, agents and verify appear in the footer as deferred

### Requirement: Suppressions
`suppress` entries SHALL be one of `{ path: <glob> }`, `{ rule: <glob over rule ids> }`, `{ finding: <finding id> }`. Findings matching any entry SHALL be removed from the report and counted in the footer as suppressed.

#### Scenario: Rule glob suppresses a whole family
- **WHEN** the profile has `suppress: [ { rule: "lint/eslint/*" } ]`
- **THEN** no `lint/eslint/...` finding appears and the footer shows the suppressed count

### Requirement: Fail policy
`fail_on` SHALL have `evidence` (minimum evidence level that fails, default `static_proven`), `categories` (default `[defect]`), `states` (default `[new]`), and optional `rules` (globs over rule ids that fail regardless of the other criteria). The review exit code SHALL follow this policy as defined by the review CLI capability.

#### Scenario: Risks do not fail by default
- **WHEN** the only findings are `coverage/untested_change` risks and `fail_on` is not customized
- **THEN** the review exits 0

#### Scenario: Explicit rule fails regardless of category
- **WHEN** `fail_on.rules` contains `coverage/*` and an `untested_change` risk is present
- **THEN** the review exits 1

### Requirement: Resolved profile stored with the review
The resolved profile, including the chain, the pinned versions of every referenced built-in module, and the engine version, SHALL be written to `profile.resolved.yaml` in the review cache, and its hash SHALL be part of the review id.

#### Scenario: Changing a suppression changes the review id
- **WHEN** a suppression is added to the profile and the same change is reviewed again
- **THEN** the review id differs from the previous run
