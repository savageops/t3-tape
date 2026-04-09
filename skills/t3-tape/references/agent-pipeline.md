# Agent Pipeline

## Goal

Use `t3-tape` as a deterministic state machine that an agent can drive without inventing its own patch ledger.

## Recommended Flow

### 1. Recon

- Confirm repo root and current branch.
- Confirm whether `.t3/patch.md` exists.
- Read the official references before mutating state.

### 2. Choose the smallest truthful command

- `init` for first-time setup
- `patch add` or `patch import` for recording a customization
- `validate` for integrity checks
- `update` for migration cycles
- `triage` / `triage approve` for review flow
- `rederive` for intent-first rebuilds

### 3. Respect the write boundary

Let the CLI write PatchMD-owned files. Do not bypass it by rewriting:

- `.t3/patch.md`
- `.t3/patch/patches/*.diff`
- `.t3/patch/patches/*.meta.json`
- `.t3/patch/triage.json`
- `.t3/patch/migration.log`

### 4. Use the migration state machine honestly

The update lifecycle is:

1. validate current state
2. fetch upstream
3. create sandbox
4. classify patches
5. resolve or rederive when needed
6. inspect triage
7. approve patches explicitly

### 5. Report with evidence

Always include:

- commands run
- exit codes when relevant
- whether `validate` passed
- whether triage is terminal or still requires review

## Approval Rules

- `triage approve` is the rewrite boundary.
- Approval rewrites patch diff and metadata against the migrated base.
- Approval is not the same thing as merging the sandbox branch into the operator's current branch.

## Foreign `.t3` Content

PatchMD is one module inside `.t3`. It must not normalize or audit unrelated Theo-owned siblings.
