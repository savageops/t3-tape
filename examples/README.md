# Examples

This repo now ships two kinds of examples:

- protocol fixtures
- runnable developer tools

Every listed example surface now ships its own `tests/` directory and is included in `pnpm run test:examples`.

Shared automation layer:
- [`examples/agent-kit/README.md`](./agent-kit/README.md)
  the agentic examples use this helper layer to read real `.t3` state and emit follow-up `t3-tape` commands without inventing a second source of truth

## Protocol fixture

### `basic-fork`

Purpose:
- show the committed PatchMD plugin shape in the smallest readable form

Path:
- [`examples/basic-fork/README.md`](./basic-fork/README.md)

## Runnable apps

### `migration-autopilot`

Pain point:
- shallow demos prove file shapes but still hide the actual migration leverage

What it does:
- creates temp upstream and fork repos
- runs `t3-tape init`, records multiple patches, and introduces upstream churn
- runs `t3-tape update`, then synthesizes agent handoff and review packets from the resulting `.t3` state
- auto-approves ready patches and finishes with `t3-tape validate`

Why it matters:
- this is the closest runnable example to the actual product pitch: PatchMD surviving upstream churn through update, agent help, review, and approval rather than static diff storage alone

Run:

```bash
node examples/migration-autopilot/src/cli.js --format markdown
```

### T3 Tape operator loops

Downstream workflow stack:
- `agent-handoff-builder`, `migration-review-assistant`, and `fleet-upgrade-coordinator` now sit directly under the flagship migration example as config-aware operator loops
- they consume `.t3/patch/config.json`, `.t3/patch/triage.json`, and `.t3/patch.md` to drive agent dispatch, review gating, and fleet scheduling

### `agent-handoff-builder`

Pain point:
- unresolved patches are hard to hand off cleanly to agent runners and ops workflows

What it does:
- reads `.t3/patch/config.json`, `.t3/patch/triage.json`, and `.t3/patch.md`
- builds conflict-resolution and re-derivation job packets with follow-up commands
- emits a four-stage automation loop driven by agent endpoint, preview command, and hook settings from config

Run:

```bash
node examples/agent-handoff-builder/src/cli.js --state-dir examples/fixtures/agent-demo/.t3 --format markdown
```

### `migration-review-assistant`

Pain point:
- migration PR review still takes too much human stitching across triage, preview, and assertion output

What it does:
- turns `.t3` triage state into review findings, comments, and approval candidates
- gives bots and dashboards a compact review packet instead of raw sandbox state
- emits a staged review loop for blockers, guarded approvals, and safe approvals

Run:

```bash
node examples/migration-review-assistant/src/cli.js --state-dir examples/fixtures/agent-demo/.t3 --assertions examples/fixtures/agent-demo/assertions.json --format markdown
```

### `fleet-upgrade-coordinator`

Pain point:
- carrying multiple patched dependencies across many repos makes update scheduling noisy and expensive

What it does:
- chooses which forks should run `t3-tape update` now, later, or not at all
- emits exact commands for schedulers, bots, and control planes
- emits automation waves so immediate, scheduled, and blocked updates are explicit

Run:

```bash
node examples/fleet-upgrade-coordinator/src/cli.js --manifest examples/fleet-upgrade-coordinator/sample/manifest.json --releases examples/fleet-upgrade-coordinator/sample/releases.json --format markdown
```

### Generic automation loops

### `dev-env-doctor`

Pain point:
- environment drift during onboarding and CI setup

What it does:
- checks tools, environment variables, files, and services
- emits a remediation loop for blocked checks, warning cleanup, and rerun verification

Run:

```bash
node examples/dev-env-doctor/src/cli.js --profile examples/dev-env-doctor/sample/profile.json --snapshot examples/dev-env-doctor/sample/snapshot.json --json
```

### `test-impact-planner`

Pain point:
- too many tests for small changes, too few tests for risky changes

What it does:
- maps changed files to commands, owners, labels, and risk
- emits command batches and a staged validation-routing loop for CI or bots

Run:

```bash
node examples/test-impact-planner/src/cli.js --manifest examples/test-impact-planner/sample/manifest.json --changes-file examples/test-impact-planner/sample/changes.txt --json
```

### `release-note-router`

Pain point:
- slow and inconsistent release note generation

What it does:
- parses commits, groups them into release notes, and recommends a bump
- emits a release-train workflow for changelog jobs and release bots

Run:

```bash
node examples/release-note-router/src/cli.js --input examples/release-note-router/sample/commits.txt --format markdown
```

## Run all example tests

```bash
pnpm run test:examples
```
