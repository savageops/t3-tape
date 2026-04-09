# PatchMD Protocol

## Purpose

PatchMD is the protocol that T3 Tape implements for long-lived, intent-preserving software forks.

It exists to solve a specific failure mode:
- teams customize upstream software
- the customization is preserved only as a raw patch
- upstream refactors the touched surface
- the patch stops applying cleanly and the original reason for the change is no longer encoded anywhere durable

PatchMD preserves both the executable delta and the human-readable reason for the delta. That makes later migration a re-derivation problem, not only a line-offset problem.

## Canonical Ownership

PatchMD-owned state lives under one canonical directory:

```text
.t3/
  patch.md
  state.lock
  patches/
    PATCH-001.diff
    PATCH-001.meta.json
  sandbox/
    <sandbox-id>/
  triage.json
  migration.log
  config.json
```

Ownership rules:
- `patch.md` is the human-readable registry
- `patches/*.diff` are the executable patch layer
- `patches/*.meta.json` are the machine-operational layer
- `triage.json` is the latest update-cycle read model
- `migration.log` is append-only operational history
- `config.json` is the operator-controlled protocol config
- `state.lock` is an ephemeral exclusive lock file used to prevent concurrent `.t3/` writers

Allowed non-owned content:
- `.t3/reports/`
- other foreign directories the tool does not manage

Validation checks PatchMD-owned files without attempting to normalize unrelated `.t3` content.

## Two-Layer Atomic Rule

Every PatchMD customization must exist in two places at once:

1. code layer
   - unified diff stored at `.t3/patches/PATCH-###.diff`
2. intent layer
   - a plain-English patch block appended to `.t3/patch.md`

A machine-readable companion file joins them:
- `.t3/patches/PATCH-###.meta.json`

Invariants:
- a diff without intent is invalid
- intent without a matching diff is invalid
- metadata must agree with both
- staged code changes without staged PatchMD updates fail `validate --staged`

## `patch.md` Grammar

Required header:

```markdown
# PatchMD
> project: <upstream-name>
> upstream: <upstream-repo-url>
> base-ref: <commit-sha-or-tag>
> protocol: 0.1.0

---
```

Required patch block form:

```markdown
## [PATCH-001] <short-title>

**status:** active
**surface:** <text>
**added:** 2026-04-09
**author:** <text>

### Intent

<plain English explanation>

### Behavior Contract

- <observable rule>
- <observable rule>

### Scope

- **files:** [...]
- **components:** [...]
- **entry-points:** [...]

### Dependencies

- **requires:** [...]
- **conflicts-with:** [...]

### Notes

<optional notes>

---
```

Parser rules:
- patch blocks are discovered by `## [PATCH-###]`
- required fields must exist exactly once per block
- required sections must exist exactly once per block
- unknown sections are preserved for forward compatibility
- status edits are minimal in-place mutations of the relevant patch block, not destructive rewrites of unrelated blocks

## Allowed Status Values

The frozen status set is:
- `active`
- `deprecated`
- `merged-upstream`
- `conflict`
- `pending-review`

Any other status is a validation failure.

## `meta.json` Schema

Each patch has a companion metadata file with kebab-case keys.

Minimum required keys:
- `id`
- `title`
- `status`
- `base-ref`
- `current-ref`
- `diff-file`
- `apply-confidence`
- `last-applied`
- `last-checked`
- `agent-attempts`
- `surface-hash`
- `behavior-assertions`

Semantics:
- `base-ref` and `current-ref` start at the fork head used when the patch was recorded
- different patches can legitimately carry different recorded refs before an update cycle completes
- both values are rewritten to the approved migrated upstream ref when that patch is approved
- `agent-attempts` increments when T3 Tape actually makes a conflict-resolution or re-derivation request
- unknown keys are tolerated but must not break canonical keys

Example:

```json
{
  "id": "PATCH-001",
  "title": "settings-toolbar-button",
  "status": "active",
  "base-ref": "abc1234",
  "current-ref": "abc1234",
  "diff-file": "patches/PATCH-001.diff",
  "apply-confidence": 1.0,
  "last-applied": "2026-04-09T10:00:00Z",
  "last-checked": "2026-04-09T10:00:00Z",
  "agent-attempts": 0,
  "surface-hash": "...",
  "behavior-assertions": [
    "settings button is visible",
    "clicking the button opens settings"
  ]
}
```

## Surface Hash

`surface-hash` summarizes the expected preimage surface of a patch.

The current implementation derives it from the diff itself:
- changed file path
- original hunk headers
- preimage lines and stable context
- removed lines included
- added lines excluded from the preimage signature

This gives T3 Tape a stable signal about what surface was originally customized without depending on a separate snapshot file.

## Validation Semantics

`t3-tape validate` is the protocol enforcer.

It checks:
- required PatchMD-owned files exist
- `config.json` parses
- `patch.md` header fields parse and the stored `base-ref` resolves in git
- patch ids are unique
- required `patch.md` sections exist
- matching diff and meta files exist
- status values are valid
- `behavior-assertions` in meta match the registry entry
- `surface-hash` in meta matches the recomputed preimage signature from the stored diff
- per-patch `base-ref` and `current-ref` resolve in git and stay in sync
- triage-aware ref checks apply when the current update cycle makes a target ref knowable
- dependencies reference real patches
- dependency graph is acyclic
- `triage.json`, when present, parses as the current schema

Cycle-aware ref rules:
- without a live triage cycle, T3 Tape validates that refs are real and internally consistent without assuming every patch shares one recorded ref
- during an open triage cycle, approved patches must point at the cycle target ref while unapproved patches may still point at their historical recorded refs
- once the triage cycle is terminal, the root `patch.md` header and all approved patch refs must agree on the migrated upstream ref

Special tolerances:
- missing `.t3/triage.json` before the first update cycle is allowed
- placeholder `{}` triage content is allowed before the first real update cycle
- foreign `.t3/reports/` content is allowed

`t3-tape validate --staged` adds commit-time enforcement:
- if staged code changes exist outside `.t3/`
- then staged PatchMD updates must also exist

This is what turns PatchMD from a suggestion into an enforceable workflow.

## Hooks

PatchMD supports opt-in hooks:
- `pre-patch`
- `post-patch`
- `pre-update`
- `post-update`
- `on-conflict`

Rules:
- hooks are configured in `.t3/config.json`
- hooks are opt-in, not implicit
- hook failures are surfaced deterministically
- hook execution never creates shadow state or alternate PatchMD ownership

## Non-Goals

PatchMD is not:
- a replacement for contributing upstream
- a secrets manager
- a GUI framework
- a monorepo manager
- a reason to create parallel state directories or alternate registries

## References

- [Update flow](update-flow.md)
- [Agent contract](agent-contract.md)
- [Git apply documentation](https://git-scm.com/docs/git-apply)
