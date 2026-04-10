# Migration Review Assistant

`migration-review-assistant` is the review-bot style example for T3 Tape migration output.

Pain point it solves:
- raw triage JSON is too low-level for reviewers
- humans have to stitch together confidence, preview status, unresolved assertions, and approval commands
- migration PRs need clear findings, not just a pile of sandbox artifacts

What it does:
- reads `.t3/patch/triage.json`, `.t3/patch.md`, and optional assertion results
- classifies patch findings by operator priority
- emits review comments, approval candidates, guarded-review queues, and next-step commands
- emits a staged review loop so PR bots or dashboards can separate blockers, guarded approvals, and safe approvals
- gives PR bots, dashboards, and chatops flows a compact review packet

Automation angle:
- consumes the same `.t3/patch/config.json` preview command and confidence threshold that the main update flow uses
- turns that state into a repeatable review gate for CI comments, PR checks, or chatops approval flows

Run it:

```bash
pnpm -C examples/migration-review-assistant test
node examples/migration-review-assistant/src/cli.js --state-dir examples/fixtures/agent-demo/.t3 --assertions examples/fixtures/agent-demo/assertions.json --format markdown
```

Test coverage:
- `65` Vitest cases covering assertion normalization, finding classification, workflow generation, and CLI behavior
