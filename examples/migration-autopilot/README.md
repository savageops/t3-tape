# Migration Autopilot

`migration-autopilot` is the flagship automation example for T3 Tape.

Unlike the smaller read-model examples, this one drives a real temp-repo migration pipeline:

- creates upstream and fork repos
- runs `t3-tape init`
- records multiple patches
- introduces upstream churn
- runs `t3-tape update`
- reuses the agent handoff and review helpers to synthesize automation packets
- runs `triage approve` automatically for approval-ready patches
- finishes with `t3-tape validate`

Run:

```bash
pnpm -C examples/migration-autopilot test
node examples/migration-autopilot/src/cli.js --format markdown
```

Use `--keep-temp` when you want to inspect the generated repos after the example finishes.
