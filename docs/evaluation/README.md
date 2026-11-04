# Automated acceptance evaluation

The stable v0.1 release requires ten real pull requests from at least three repositories covering TypeScript and Python. Fixture results do not satisfy this gate. The user approved independent sub-agent verdicts in place of the original human gate. Every verdict is labelled automated; agent assessments are not represented as human.

Record reviews in `v0.1.md` and the machine-checked `v0.1.json`. The latter has this structure:

```json
{
  "pull_requests": [
    {
      "repository": "owner/repo",
      "pr": "https://github.com/owner/repo/pull/123",
      "languages": ["typescript"],
      "mode": "base",
      "review_id": "actual review id",
      "wall_time_ms": 1234,
      "findings": [{"id": "finding id", "verdict": "true-positive", "reviewer": "sub-agent name", "reason": "independent reviewer observation"}],
      "misses": [],
      "verdict": "useful",
      "reason": "independent reviewer explanation",
      "reviewer": "sub-agent name",
      "verdict_source": "automated"
    }
  ],
  "conclusion": "go",
  "conclusion_reviewer": "sub-agent name",
  "conclusion_reason": "independent reviewer assessment"
}
```

An empty findings/misses list means an independent reviewer checked and found none. Use `false-positive` or `noise` for other finding verdicts and `not-useful` when appropriate. Preserve report artifacts and per-check timing evidence alongside each entry. Do not represent an automated verdict as a human assessment. `python3 scripts/check-acceptance.py --record-only` validates completeness and accepts an honest no-go. Without that flag, the checker also requires go; stable release CI runs it before publishing.
