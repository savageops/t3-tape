# Migration Review Assistant

`migration-review-assistant` is the review-bot style example for T3 Tape migration output.

Pain point it solves:
- raw triage JSON is too low-level for reviewers
- humans have to stitch together confidence, preview status, unresolved assertions, and approval commands
- migration PRs need clear findings, not just a pile of sandbox artifacts

What it does:
- reads `.t3/triage.json`, `.t3/patch.md`, and optional assertion results
- classifies patch findings by operator priority
- emits review comments, approval candidates, and next-step commands
- gives PR bots, dashboards, and chatops flows a compact review packet

Run it:

```bash
pnpm -C examples/migration-review-assistant test
node examples/migration-review-assistant/src/cli.js --state-dir examples/fixtures/agent-demo/.t3 --assertions examples/fixtures/agent-demo/assertions.json --format markdown
```

Test coverage:
- `64` Vitest cases covering assertion normalization, finding classification, review packet generation, and CLI behavior
