# Release Note Router

`release-note-router` is a small CLI that groups commits into release notes and recommends a version bump.

Pain point it solves:
- release notes taking too long to write by hand
- inconsistent commit grouping across teams
- missing breaking changes during release prep

What it does:
- parses conventional and non-conventional commits
- groups them into stable release note sections
- recommends `major`, `minor`, `patch`, or `none`
- prints either JSON or Markdown

Run it:

```bash
pnpm -C examples/release-note-router test
node examples/release-note-router/src/cli.js --input examples/release-note-router/sample/commits.txt --format markdown
```

Test coverage:
- `60+` Vitest cases covering commit parsing, bump selection, note grouping, markdown rendering, and CLI behavior
