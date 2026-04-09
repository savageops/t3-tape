# Test Impact Planner

`test-impact-planner` is a small CLI that maps changed files to the smallest useful validation plan.

Pain point it solves:
- running too many tests for small changes
- missing critical checks when config or infrastructure files change
- routing PRs to the right owners and labels

What it does:
- normalizes changed file paths
- matches them against manifest rules
- returns commands, owners, labels, and risk
- supports shell scripts, CI jobs, bots, and review dashboards

Run it:

```bash
pnpm -C examples/test-impact-planner test
node examples/test-impact-planner/src/cli.js --manifest examples/test-impact-planner/sample/manifest.json --changes-file examples/test-impact-planner/sample/changes.txt --json
```

Test coverage:
- `60+` Vitest cases covering glob matching, plan construction, routing, and CLI behavior
