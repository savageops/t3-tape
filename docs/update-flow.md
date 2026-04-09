# Update Flow

## Overview

`t3-tape update` is a stateful migration process. It does not mutate the current branch and hope the result is good enough.

The design goals are:
- keep the operator's current branch safe by default
- make every decision inspectable
- persist enough state for a later UI without inventing a second model
- let external agents help without becoming the only source of truth

## Phase Map

### Phase 0: Snapshot

Inputs:
- resolved repo root
- resolved state dir
- current `HEAD`
- active PatchMD records

Actions:
- require a valid git repo
- resolve current fork commit sha
- append a `STARTED` entry to `.t3/migration.log`
- run `hooks.pre-update` when configured

Failure rules:
- invalid state fails before sandbox creation
- hook failure aborts before sandbox mutation

### Phase 1: Fetch Upstream

Inputs:
- `.t3/config.json` `upstream`
- `--ref <new-ref>`

Actions:
- fetch the requested target
- resolve it to a concrete commit sha
- keep the current branch untouched

Persisted fields later recorded in triage state:
- `to-ref`
- `to-ref-resolved`

### Phase 2: Sandbox Preparation

Actions:
- create `.t3/sandbox/<timestamp>/`
- create a linked worktree at `.t3/sandbox/<timestamp>/worktree`
- check out a migration branch named `t3-tape/migrate/<timestamp>`
- base the worktree on the fetched upstream commit

Safety boundary:
- worktree creation happens away from the current branch
- current branch `HEAD` must stay unchanged through triage and approval flows

### Phase 3: Clean Apply Attempt

Each active patch is examined in canonical order.

Primary classifications:
- `CLEAN`
- `CONFLICT`
- `MISSING-SURFACE`

Supporting signal:
- `merged-upstream-candidate`

How they are produced:
- `MISSING-SURFACE`: a diff-referenced path is missing from the sandbox worktree
- `CLEAN`: `git apply --check` succeeds in the sandbox worktree
- `CONFLICT`: `git apply --check` fails for a present surface
- `merged-upstream-candidate`: `git apply --reverse --check` succeeds when forward apply does not

Artifacts persisted after triage:
- `.t3/sandbox/<timestamp>/triage.json`
- `.t3/triage.json`
- `TRIAGED` entry appended to `.t3/migration.log`

## `triage.json`

`triage.json` is the latest-cycle read model.

Expected top-level shape:

```json
{
  "schema-version": "0.1.0",
  "from-ref": "<fork-head-sha>",
  "to-ref": "<user-ref>",
  "to-ref-resolved": "<resolved-sha>",
  "upstream": "<repo-url>",
  "timestamp": "<utc-ts>",
  "sandbox": {
    "path": ".t3/sandbox/<timestamp>",
    "worktree-branch": "t3-tape/migrate/<timestamp>",
    "worktree-path": ".t3/sandbox/<timestamp>/worktree"
  },
  "patches": [
    {
      "id": "PATCH-001",
      "title": "example",
      "detected-status": "CONFLICT",
      "triage-status": "pending-review"
    }
  ]
}
```

Ordering rules:
- patch order follows `patch.md`
- human output groups counts in a stable order
- identical inputs produce stable JSON ordering and stable stderr truncation

## Agent-Assisted Phases

### Phase 4: Conflict Resolution

For `CONFLICT` patches T3 Tape sends:
- the original diff
- the upstream overlap diff
- the patch intent
- the behavior assertions
- the current source snapshot for the touched files

Possible outcomes:
- confidence at or above threshold -> `pending-review`
- confidence below threshold -> `NEEDS-YOU`
- provider unavailable -> `NEEDS-YOU`

### Phase 5: Re-derivation

For `MISSING-SURFACE` patches or explicit `t3-tape rederive PATCH-001`:
- T3 Tape sends the intent, behavior assertions, current source, and surface hint
- the agent returns a newly derived diff plus optional scope update

This is the differentiator of the protocol: the system can rebuild from intent when the original surface disappears.

## Sandbox Staging and Preview

Resolved state is staged on the sandbox migration branch.

Apply rules:
- `CLEAN` and `pending-review` patches are applied in canonical patch order
- each successfully staged patch gets its own sandbox commit
- the commit sha is recorded in `triage.json`

Agent artifact directory:

```text
.t3/sandbox/<timestamp>/resolved/
  PATCH-001.diff
  PATCH-001.notes.txt
  PATCH-001.json
```

Preview command behavior:
- `sandbox.preview-command` runs inside the sandbox worktree when configured
- stdout and stderr are captured under `.t3/sandbox/<timestamp>/preview/`
- preview failure blocks approval but does not delete sandbox artifacts

## Approval

`t3-tape triage approve PATCH-001` is the state rewrite point.

Approval does all of the following:
- loads the latest triage summary
- refuses approval when preview failed
- extracts the staged sandbox commit diff for the selected patch
- rewrites `.t3/patches/PATCH-001.diff`
- updates `.t3/patches/PATCH-001.meta.json`
- rewrites the root `> base-ref:` line in `.t3/patch.md`
- updates the approved patch block to `active` unless the user already marked it `deprecated` or `merged-upstream`

Approval does not:
- reset or switch the current branch
- auto-merge the sandbox branch into the operator's branch

## Migration Log

`.t3/migration.log` is append-only.

It records:
- cycle start
- triage counts
- completion when all patches are approved or otherwise terminal

A cycle becomes `COMPLETE` only when:
- every `CLEAN` patch is approved
- every `pending-review` patch is approved
- unresolved `CONFLICT`, `MISSING-SURFACE`, or `NEEDS-YOU` entries are gone

`hooks.post-update` runs only after the `COMPLETE` entry is appended.

## CI Mode

`t3-tape update --ci` still writes sandbox and triage artifacts.

Behavior difference:
- clean-only cycles can succeed
- any non-clean patch exits with code `3`
- no hidden auto-approval occurs

This keeps CI useful as an early-warning surface without letting CI rewrite PatchMD state by itself.

## Failure Modes

Important blocked states that must stay diagnosable from CLI output plus artifacts:
- invalid PatchMD state before update
- missing git or fetch failure
- sandbox path collision
- worktree creation failure
- dependency cycle
- missing agent configuration
- confidence below threshold
- preview command failure
- stale or missing triage data at approval time

## References

- [PatchMD protocol](patchmd.md)
- [Agent contract](agent-contract.md)
- [Git worktree documentation](https://git-scm.com/docs/git-worktree)
- [Git apply documentation](https://git-scm.com/docs/git-apply)
