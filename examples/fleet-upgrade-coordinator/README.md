# Fleet Upgrade Coordinator

`fleet-upgrade-coordinator` is the scheduled update example for teams carrying multiple long-lived forks.

Pain point it solves:
- one patched dependency is manageable, ten are not
- teams need to know which upstream releases should run now, later, or never
- blanket-updating every fork wastes agent time and creates noisy review queues

What it does:
- reads a fleet manifest and upstream release feed
- picks the best target release for each fork
- decides whether to run, schedule, hold, or skip the update
- emits exact `t3-tape update` commands for schedulers, bots, and control planes

Run it:

```bash
pnpm -C examples/fleet-upgrade-coordinator test
node examples/fleet-upgrade-coordinator/src/cli.js --manifest examples/fleet-upgrade-coordinator/sample/manifest.json --releases examples/fleet-upgrade-coordinator/sample/releases.json --format markdown
```

Test coverage:
- `61` Vitest cases covering release selection, policy routing, update command planning, and CLI behavior
