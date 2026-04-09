# Example Basic Fork

This fixture is a committed example of the canonical PatchMD store shape.

What it demonstrates:
- `.t3/patch.md` committed alongside a PatchMD plugin subtree at `.t3/patch/`
- one patch record stored as `patch.md + diff + meta`
- one sample `migration.log` entry
- one tolerated foreign report artifact under `.t3/reports/`

It is intentionally small and readable so adopters can inspect the protocol without generating a sandbox cycle first.

Run tests:

```bash
pnpm -C examples/basic-fork test
```

Test coverage:
- `86` Vitest cases covering fixture structure, config contract, patch registry content, diff parity, and migration-log integrity
