# PatchMD
> project: editor-shell
> upstream: https://github.com/acme/editor-shell
> base-ref: abc1234
> protocol: 0.1.0
> state-root: patch

---

## [PATCH-001] plugin-settings-toolbar-button

**status:** active
**surface:** src/toolbar.tsx
**added:** 2026-03-01
**author:** savage

### Intent

Add a settings button to the top-right toolbar so operators can open plugin configuration without leaving the editor shell.

### Behavior Contract

- toolbar renders a plugin settings button
- clicking the button opens the plugin settings panel

### Scope

- **files:** [src/toolbar.tsx, src/plugin-settings.tsx]
- **components:** [Toolbar, PluginSettingsPanel]
- **entry-points:** [toolbar]

### Dependencies

- **requires:** []
- **conflicts-with:** []

---

## [PATCH-002] command-palette-plugin-bridge

**status:** active
**surface:** src/editor/commands.ts
**added:** 2026-03-04
**author:** savage

### Intent

Keep the plugin action reachable from the command palette even when upstream rearranges the editor command registry.

### Behavior Contract

- command palette shows the plugin action
- selecting the command opens the plugin workflow

### Scope

- **files:** [src/editor/commands.ts, src/editor/palette.ts]
- **components:** [CommandRegistry]
- **entry-points:** [command palette]

### Dependencies

- **requires:** [PATCH-001]
- **conflicts-with:** []

### Notes

This patch is fragile when upstream renames the command registry surface.

---

## [PATCH-003] schema-change-cache-reset

**status:** active
**surface:** scripts/ci-cache-reset.mjs
**added:** 2026-03-10
**author:** savage

### Intent

Reset the workspace cache when the patch schema changes so old artifacts do not leak across update runs.

### Behavior Contract

- schema drift triggers a cache reset
- unchanged schema leaves caches intact

### Scope

- **files:** [scripts/ci-cache-reset.mjs, .github/workflows/ci.yml]
- **components:** [CI cache reset]
- **entry-points:** [ci]

### Dependencies

- **requires:** []
- **conflicts-with:** []

---
