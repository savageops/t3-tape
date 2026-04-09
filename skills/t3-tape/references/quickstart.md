# T3 Tape Quickstart

## Canonical Ownership

The live PatchMD ownership boundary is:

```text
.t3/patch.md
.t3/patch/**
```

Treat any other `.t3/*` sibling as foreign content.

## Command Decision Guide

### First-time setup

```powershell
t3-tape init --upstream <repo-url> --base-ref <ref>
```

Use when the repo has no PatchMD state yet. `init` may coexist with a pre-existing `.t3/` directory as long as foreign siblings are left untouched.

### Record a new customization

```powershell
t3-tape patch add --title "<title>" --intent "<plain-English intent>" --assert "<behavior>"
```

Use only after the actual source changes already exist in the working tree. The CLI writes:

- `.t3/patch.md`
- `.t3/patch/patches/PATCH-###.diff`
- `.t3/patch/patches/PATCH-###.meta.json`

### Import an existing diff

```powershell
t3-tape patch import --diff <path-to-diff>
```

Use when the change already exists as a diff outside PatchMD.

### Validate integrity

```powershell
t3-tape validate
t3-tape validate --staged
t3-tape validate --json
```

Use `validate` before closing work. Use `--staged` in hooks or pre-commit flows.

### Run an upstream migration

```powershell
t3-tape update --ref <new-ref>
t3-tape triage
t3-tape triage approve PATCH-001
```

Use `rederive` for intent-first rebuilds:

```powershell
t3-tape rederive PATCH-001
```

### Export a summary

```powershell
t3-tape export --format markdown --output CUSTOMIZATIONS.md
```

## Daily Workflow

1. Make the source change in the fork.
2. Run `t3-tape patch add`.
3. Run `t3-tape validate`.
4. Commit code plus PatchMD state together.

## Verification Minimum

For a normal feature or maintenance task, run at least:

```powershell
t3-tape validate
```

For repo-level changes, prefer the full frozen gates from `references/ci-cd.md`.
