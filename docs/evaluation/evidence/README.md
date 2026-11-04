# Acceptance evidence

Independent automated reviewers produced these artifacts on 2026-11-04. No human verdicts are claimed.

The `typescript`, `python`, and `structlog` directories preserve reports, progress, upstream test output, diffs, and semantic reproductions. Python reports use deterministic gzip compression; decompress them before comparing their SHA-256 to the uncompressed digest in `agent-python.json`. Initial runs from before the fixes are labelled `pre-` or `initial`.

Paths under `/tmp/rio-acceptance-*` in the original agent records map to these persistent directories. Exact PR commits, profile settings, source installations and limitations remain in each parent agent record. Tool diagnostics from installed dependencies and existing tests are intentionally preserved as noise evidence.
