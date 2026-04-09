# CI/CD And Automation

## Core Machine Surfaces

- `t3-tape validate --json`
- `t3-tape triage --json`
- `.t3/patch/triage.json`
- `.t3/patch/migration.log`
- exit codes `0-5`

## Hook Guidance

Use the CLI-generated snippets when possible:

```powershell
t3-tape hooks print pre-commit
t3-tape hooks print gitignore
t3-tape hooks print gitattributes
t3-tape hooks install pre-commit
```

## Recommended Local Gates

```powershell
pnpm install --frozen-lockfile
pnpm -C packages/t3-tape-npm build
pnpm -C packages/t3-tape-npm test
pnpm run test:examples
cargo test -p t3-tape
cargo build --release -p t3-tape
powershell -ExecutionPolicy Bypass -File scripts/e2e.ps1
```

Use `scripts/run-recommended-gates.ps1` to print or run this sequence.

## CI Patterns

### Validation

```powershell
t3-tape validate --json
```

### Scheduled update check

```powershell
t3-tape update --ref <new-ref> --ci --confidence-threshold 0.90
```

Exit code `3` means review is required, not that the tool crashed.

## Launcher Guidance

- `packages/t3-tape-npm` is a thin launcher only.
- The launcher never owns PatchMD state.
- `T3_TAPE_BINARY_PATH` is the preferred local or CI override.
