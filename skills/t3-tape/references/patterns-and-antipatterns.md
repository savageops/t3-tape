# Patterns And Anti-Patterns

## Good Patterns

- Keep `.t3/patch.md` human-readable and let `t3-tape` own operational files.
- Keep `.t3/patch/**` as the only machine-operational PatchMD subtree.
- Run `validate` after PatchMD writes and after migration approvals.
- Use `update` plus `triage` instead of blind merges when upstream churn is involved.
- Treat `patch.md` intent as a re-derivation contract, not as decorative prose.
- Keep CI branching on exit codes and structured JSON, not on scraped console text.

## Anti-Patterns

- Do not hand-edit owned diff or meta files if the CLI can perform the write.
- Do not capture `.t3/patch.md` or `.t3/patch/**` into feature diffs.
- Do not reopen old `.untrack/` or root-owned `.t3/config.json` mental models.
- Do not create a second registry, sidecar database, or Node-owned state mirror.
- Do not treat foreign `.t3/*` siblings as drift or cleanup targets.
- Do not claim the update is complete before approval reaches a terminal cycle.

## Truthfulness Rules

- Sell only the features that exist in the current implementation.
- If a contract is only documented but not enforced, fix the code or narrow the docs.
- If a repo already contains `.t3/`, PatchMD must enter as a plugin, not as a hostile takeover.
