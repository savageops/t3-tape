# Agent Handoff Builder

`agent-handoff-builder` turns live `.t3` state into external agent jobs.

Pain point it solves:
- unresolved patches are hard to hand off cleanly to agent runners
- teams lose the intent and behavior contract when they build jobs by hand
- schedulers and control planes need stable commands and payload hints, not raw triage JSON only

What it does:
- reads `.t3/patch/config.json`, `.t3/patch/triage.json`, and `.t3/patch.md`
- selects unresolved or review-stage patches
- emits agent mode, intent, assertions, artifact paths, and follow-up commands
- gives schedulers and runners a ready queue without rewriting PatchMD state

Run it:

```bash
pnpm -C examples/agent-handoff-builder test
node examples/agent-handoff-builder/src/cli.js --state-dir examples/fixtures/agent-demo/.t3 --format markdown
```

Test coverage:
- `62` Vitest cases covering PatchMD plugin parsing, mode selection, handoff packet construction, and CLI behavior
