---
name: t3-tape
description: Operate the T3 Tape PatchMD workflow in a real repo using the official filesystem contract, command surface, validation gates, and migration flow. Use when Codex needs to initialize PatchMD in a repo, record or import patches, validate `.t3/patch.md` plus `.t3/patch/**`, run updates and triage, approve or rederive patches, wire CI/hooks, or teach another agent how to use `t3-tape` correctly without creating parallel state or hand-editing owned artifacts.
---

# T3 Tape

## Overview

Use this skill to operate `t3-tape` as a real PatchMD state machine, not as a loose patch-note convention.

Treat the Rust CLI as the only writer for PatchMD-owned state. The authoritative ownership boundary is:

```text
.t3/patch.md
.t3/patch/**
```

Everything else under `.t3/*` is foreign unless the repo explicitly says otherwise.

## Workflow

### 1. Recon the workspace first

- Confirm repo root, current branch, and whether `.t3/patch.md` already exists.
- Read the nearest `AGENTS.md` and the official `t3-tape` references before changing owned state.
- If the repo already contains `.t3/*` siblings, treat them as foreign content unless they are `.t3/patch.md` or inside `.t3/patch/`.

### 2. Choose the right entry command

- Use `init` for first-time PatchMD setup.
- Use `patch add` after real source changes already exist.
- Use `patch import` when a diff already exists outside PatchMD.
- Use `validate` before commits, CI transitions, and after any migration rewrite.
- Use `update` to fetch, sandbox, triage, and stage a migration cycle.
- Use `triage` and `triage approve` to inspect and accept migrated patches.
- Use `rederive` when a patch must be rebuilt from intent instead of a direct diff application.
- Use `export` when a human-readable customization summary is needed.

### 3. Preserve the ownership rules

- Never hand-edit `.t3/patch/patches/*.diff` or `.t3/patch/patches/*.meta.json` if the CLI can perform the write.
- Never treat foreign `.t3/*` siblings as validation drift.
- Never record `.t3/patch.md` or `.t3/patch/**` into feature diffs.
- Never create a second registry, alternate state directory, or Node-owned PatchMD implementation.
- Never claim success without running at least the contract-specific validation gate.

### 4. Use the agent-friendly step pipeline

For a normal repo task:

1. Read `references/quickstart.md`.
2. If the task involves updates or review, also read `references/agent-pipeline.md`.
3. If the task involves hooks, CI, or release automation, also read `references/ci-cd.md`.
4. If there is any ambiguity about a rule, read the matching file under `references/official/`.
5. Run the minimum command that changes the canonical state through `t3-tape`.
6. Run `validate` or the full recommended gates before closing the task.

### 5. Use the helper scripts when they save time

- `scripts/invoke-t3-tape.ps1`
  Resolve and run a local `t3-tape` binary without guessing paths.
- `scripts/run-recommended-gates.ps1`
  Print or run the frozen gate suite for a repo using T3 Tape.
- `scripts/bundle-local-binary.ps1`
  Copy a locally built Windows binary into the skill's asset bundle for reuse.

## References

Read only what matches the task:

- `references/quickstart.md`
  Daily workflow, command selection, and common repo states.
- `references/agent-pipeline.md`
  Agent-safe update, triage, approval, and rederivation flow.
- `references/ci-cd.md`
  Hook, CI, launcher, and release automation guidance.
- `references/patterns-and-antipatterns.md`
  Best practices, anti-patterns, and state-integrity rules.
- `references/official/README.md`
  Official operator guide.
- `references/official/patchmd.md`
  Protocol and filesystem contract.
- `references/official/update-flow.md`
  Phase-by-phase migration model.
- `references/official/agent-contract.md`
  External agent request/response contract.
- `references/official/implementation-status.md`
  What is shipped now.
- `references/official/architecture.md`
  System architecture and ownership model.
- `references/official/npm-launcher.md`
  Launcher package notes and packaged-binary behavior.

## Closing Rules

- Prefer the Rust CLI over direct file edits for owned PatchMD state.
- Prefer source-of-truth docs over memory if there is any contract doubt.
- Prefer the repo's current canonical layout over older `.untrack/` or root-owned `.t3/*` mental models.
- Prefer real verification over promises. Run commands and report what passed, what failed, and why.
