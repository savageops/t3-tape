# Dev Env Doctor

`dev-env-doctor` is a small CLI that checks whether a developer machine or CI worker is ready for a project profile.

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

Run it:

```bash
pnpm -C examples/dev-env-doctor test
node examples/dev-env-doctor/src/cli.js --profile examples/dev-env-doctor/sample/profile.json --snapshot examples/dev-env-doctor/sample/snapshot.json --json
```

Test coverage:
- `60+` Vitest cases covering version comparison, diagnostics, report generation, and CLI behavior
