# Dev Env Doctor

`dev-env-doctor` is a small CLI that checks whether a developer machine or CI worker is ready for a project profile and emits a remediation loop.

Pain point it solves:
- onboarding drift across local machines
- CI workers missing required tools or secrets
- hidden setup steps that slow down delivery

What it does:
- checks tool versions
- checks required and optional environment variables
- checks required files
- checks service readiness
- prints either text or JSON
- emits a staged remediation workflow for blocked checks, warnings, and rerun verification

Run it:

```bash
pnpm -C examples/dev-env-doctor test
node examples/dev-env-doctor/src/cli.js --profile examples/dev-env-doctor/sample/profile.json --snapshot examples/dev-env-doctor/sample/snapshot.json --json
```

Test coverage:
- `72` Vitest cases covering version comparison, diagnostics, remediation workflow generation, and CLI behavior
