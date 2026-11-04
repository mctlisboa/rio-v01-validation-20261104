## Purpose

The report is the product: the ordered, evidence-first presentation of a review in the terminal, in Markdown and in JSON, together with the on-disk record that makes a run inspectable and reproducible.

## ADDED Requirements

### Requirement: Section order
Every format SHALL present sections in this order: header (review id, mode, base and head identifiers, profile name and chain, Rio version, partial marker when applicable); deterministic results grouped by check kind; impact summary; findings (new and unclassified defects); risks; summary counts (pre-existing, fixed, suppressed); footer (skipped checks, deferred steps, excluded files, size notes, tool versions, timings). Sections with nothing to show SHALL be omitted except the header, the findings statement and the footer.

#### Scenario: No findings
- **WHEN** a review produces no findings and no risks
- **THEN** the report contains the header, a line stating that no findings were produced, and the footer

### Requirement: Finding ordering
Within the findings section, findings SHALL be ordered by severity, then evidence level, then path and line. Findings matching the `fail_on` policy SHALL be marked as such.

#### Scenario: Errors before warnings
- **WHEN** a review has a lint warning in `a.ts` and a type error in `z.ts`
- **THEN** the type error is listed first

### Requirement: Finding presentation
Each rendered finding SHALL show rule id, state, evidence level, location with enclosing symbol, message, trigger when known, and suggested fix when present. Secret values SHALL appear redacted. Pre-existing findings SHALL be available in full through `--show-pre-existing`.

#### Scenario: Pre-existing on request
- **WHEN** `rio review --show-pre-existing` is run on a change with pre-existing type errors
- **THEN** those errors are listed in a dedicated section after the risks

### Requirement: Terminal renderer
The terminal renderer SHALL stream check results as they complete, use color only when standard output is a terminal and `NO_COLOR` is unset and `--no-color` is absent, and SHALL fit within the terminal width without wrapping finding locations.

#### Scenario: Narrow terminal
- **WHEN** the terminal is 80 columns wide
- **THEN** every location line is at most 80 columns, truncating the message rather than the path

### Requirement: Markdown renderer
The Markdown renderer SHALL produce a document with stable headings for each section, one table or list per check kind, and findings as list items with code-formatted locations, suitable for pasting into a pull request description.

#### Scenario: Stable headings
- **WHEN** two reviews are rendered as Markdown
- **THEN** both use the same heading text for the same sections

### Requirement: JSON renderer
The JSON renderer SHALL emit a single document `{ "schema": "rio-report-v1", "review": {...}, "checks": [...], "impact": {...}, "findings": [...], "risks": [...], "skipped": [...], "deferred": [...], "excluded": [...], "metrics": {...} }` that validates against `schemas/report-v1.json`, with findings in the finding-v1 form and metrics including per-check wall time and tool versions.

#### Scenario: Metrics present
- **WHEN** a review is rendered as JSON
- **THEN** `metrics` contains the wall time of each check that ran and the total wall time

### Requirement: Footer content
The footer SHALL always list: each skipped check with reason and hint; each deferred step with the reason `no_endpoint`; excluded files with reasons; a size note when `max_lines` is exceeded; graph truncation when it occurred; the counts of suppressed findings per suppression entry.

#### Scenario: Deferred steps explained
- **WHEN** a review runs with no endpoint
- **THEN** the footer lists `triage`, `agents` and `verify` as deferred with reason `no_endpoint`

### Requirement: Review cache
Each run SHALL write `.rio/cache/<review-id>/` containing `report.json`, `profile.resolved.yaml`, `graph/{base,head,delta}.json`, and `checks/<kind>/<root>/` with the raw tool output and the parsed findings. Snapshots SHALL be stored under `.rio/cache/snapshots/` and baselines under `.rio/cache/baselines/`. The system SHALL create `.rio/cache/.gitignore` containing `*` so the cache is never committed. Entries older than a configurable age (default 30 days) SHALL be pruned at the start of a run.

#### Scenario: Cache ignored by git
- **WHEN** the first review runs in a repository
- **THEN** `.rio/cache/.gitignore` exists with content `*` and `git status` shows no cache files

#### Scenario: Raw output retained
- **WHEN** a type check runs
- **THEN** its standard output and standard error are stored under `checks/typecheck/<root>/` in the review cache
