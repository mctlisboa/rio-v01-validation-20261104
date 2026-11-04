## Purpose

Change resolution turns a selection mode into an immutable pair of code versions, a diff with precise hunk ranges, and a content-addressed snapshot, so every review is reproducible and both versions of any file are available to later steps.

## ADDED Requirements

### Requirement: Base and head resolution
The system SHALL resolve a base and a head for each mode. Working-tree mode: base is the `HEAD` tree, head is the working tree including untracked files not ignored by git. Staged mode: base is the `HEAD` tree, head is the index. Branch mode: base is the merge base of the given ref and `HEAD`, head is the `HEAD` commit. An unknown ref or a pair with no merge base SHALL be an error that stops the review with exit code 3.

#### Scenario: Unknown base ref
- **WHEN** branch mode is requested with a ref that does not exist
- **THEN** the system reports the ref as unknown and exits with code 3

#### Scenario: Repository with no commits
- **WHEN** working-tree mode is run in a repository whose `HEAD` is unborn
- **THEN** the base is treated as an empty tree and every file is reported as added

### Requirement: Diff with hunk ranges
The system SHALL produce, for each changed file, its status (`added`, `modified`, `deleted`, `renamed` with the old path), and for text files the list of hunks with base and head line ranges, so later steps can map any line to "added", "modified", "removed" or "context". Binary files SHALL be listed with status but no hunks. Files under excluded paths (vendored directories, files marked generated, lockfiles) SHALL be listed with an `excluded` reason and SHALL NOT be sent to checks other than the dependency audit.

#### Scenario: Rename with edit
- **WHEN** a file is renamed and one line inside it is changed
- **THEN** the diff lists one entry with status `renamed`, both paths, and a single hunk covering the edited line

#### Scenario: Lockfile excluded from code checks
- **WHEN** `package-lock.json` is modified
- **THEN** it is listed as excluded with reason `lockfile` and is available to the dependency audit only

### Requirement: Content-addressed snapshot
The system SHALL compute a snapshot key as the BLAKE3 hash over a canonical encoding of the base identifier plus one entry per changed path, sorted by path: for a path present in the head, its path, mode and head content; for a path absent from the head (a deleted file, or the old path of a rename), its path and an explicit deletion marker. Identical inputs SHALL yield identical keys regardless of when or where the review runs. The snapshot archive SHALL be written under the review cache as `snapshots/<key>.tar.zst` and SHALL contain the changed files' head content and a manifest naming the base commit and listing every deleted path, so that applying the archive to the base tree reconstructs the head tree exactly.

#### Scenario: Same content, same key
- **WHEN** the same staged change is snapshotted twice on two machines with the same repository state
- **THEN** both produce the same snapshot key

#### Scenario: Whitespace-only edit changes the key
- **WHEN** a single trailing space is added to one changed file
- **THEN** the snapshot key changes

#### Scenario: Deletion-only changes are distinct
- **WHEN** one change deletes `a.ts` and another change on the same base deletes `b.ts`
- **THEN** the two snapshot keys differ, and each archive's manifest lists its deleted path

#### Scenario: Head reconstruction drops deleted files
- **WHEN** a snapshot whose change deletes `a.ts` and renames `b.ts` to `c.ts` is applied to the base tree
- **THEN** the reconstructed head tree contains `c.ts` and contains neither `a.ts` nor `b.ts`

### Requirement: Both versions available
The system SHALL be able to supply the base and head content of any changed file, and a materialization of the whole base tree and of the whole head tree on demand, outside the work tree. The base tree SHALL be cached by base identifier; the head tree SHALL be the base tree with the snapshot applied and SHALL be cached by snapshot key. Every check that executes a tool (type check, lint, tests and coverage) SHALL run against these materializations and never in the user's work tree, so the head that is checked is exactly the selected, immutable head: in staged mode it excludes unstaged edits, and in branch mode it excludes uncommitted changes. Materializations SHALL never modify the user's working tree or index, and tool writes inside a materialization SHALL not alter the cached tree used by later reviews. Tools SHALL run in a copy-on-write clone or an independent copy of a cached tree, never in a hard-linked one.

#### Scenario: Tool writes do not reach the cache
- **WHEN** a tool run rewrites an existing file in its tree in place
- **THEN** the same file in the cached materialization is unchanged afterwards

#### Scenario: Base tree materialized without touching the work tree
- **WHEN** a check requests the full base tree in working-tree mode
- **THEN** a copy of the base tree is provided outside the work tree, and `git status` in the user's repository is unchanged afterwards

#### Scenario: Staged review checks the index, not the work tree
- **WHEN** `rio review --staged` runs while a staged file is type-correct and the same file has an unstaged edit that introduces a type error
- **THEN** the type check runs on the materialized head tree built from the index and reports no error for that file

#### Scenario: Branch review ignores uncommitted changes
- **WHEN** `rio review --base main` runs with an uncommitted edit in the work tree
- **THEN** checks run on the materialized `HEAD` commit tree and no finding refers to the uncommitted edit

### Requirement: Large diffs are flagged, not partitioned
When the total changed line count exceeds the profile's `max_lines`, the system SHALL continue the review and SHALL add a note to the report footer stating the size and recommending a split. Partitioning into logical parts is out of scope for this version.

#### Scenario: Oversized change
- **WHEN** a change touches 2,400 lines and the profile sets `max_lines: 1500`
- **THEN** the review runs and the footer notes that the change exceeds the configured size
