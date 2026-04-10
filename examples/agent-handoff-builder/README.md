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
- emits a four-stage automation loop: read triage, dispatch agent jobs, refresh triage, approve ready patches
- gives schedulers and runners a ready queue without rewriting PatchMD state

Why config matters:
- `agent.provider` and `agent.endpoint` decide how jobs are dispatched
- `agent.confidence-threshold` and `agent.max-attempts` become queue policy
- `sandbox.preview-command` and update hooks show what CI or control planes should run around each migration pass

This is the class of config the example is designed to consume:

```json
{
  "agent": {
    "provider": "exec",
    "endpoint": "node scripts/agent-runner.mjs",
    "confidence-threshold": 0.8,
    "max-attempts": 3
  },
  "sandbox": {
    "preview-command": "pnpm test"
  },
  "hooks": {
    "pre-update": "",
    "post-update": "",
    "on-conflict": ""
  }
}
```

Run it:

```bash
pnpm -C examples/agent-handoff-builder test
node examples/agent-handoff-builder/src/cli.js --state-dir examples/fixtures/agent-demo/.t3 --format markdown
```

Test coverage:
- `63` Vitest cases covering PatchMD plugin parsing, mode selection, workflow construction, and CLI behavior
