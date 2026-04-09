# PatchMD
> project: upstream-app
> upstream: https://github.com/example/upstream-app
> base-ref: abc1234
> protocol: 0.1.0
> state-root: patch

---

## [PATCH-001] toolbar-settings-button

**status:** active
**surface:** src/app.txt
**added:** 2026-04-09
**author:** example-user

### Intent

Keep the fork-specific toolbar affordance visible in the app shell after upstream updates.

### Behavior Contract

- the customized app surface still renders the patched line
- the change remains attributable to a named PatchMD record

### Scope

- **files:** ["src/app.txt"]
- **components:** ["AppShell"]
- **entry-points:** ["src/app.txt"]

### Dependencies

- **requires:** []
- **conflicts-with:** []

### Notes

This fixture is illustrative and intentionally compact.

---
