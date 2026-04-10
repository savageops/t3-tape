# Implementation Status

## Snapshot

Original closeout target: 2026-04-09
Release-readiness reconciliation: 002 chain on 2026-04-09
PatchMD plugin hierarchy refactor: 010 chain on 2026-04-10

This document tracks shipped behavior first. It exists to keep README claims, todo-chain state, and actual code ownership aligned.

## Verification Refresh

Refresh date: 2026-04-10

This repo state was re-verified against the closeout gates after a stale execution snapshot claimed the crate still needed recovery work. The shipped repo is already beyond that snapshot. Follow-up chains then reconciled the launcher packaging surface, store/usability truthfulness, plugin hierarchy, and hook/runtime truthfulness so the live repo and the sold workflow stay aligned. Current evidence:

- `pnpm install --frozen-lockfile`: passed after switching the launcher target packages to workspace protocol so `pnpm-lock.yaml` records the target `optionalDependencies` in the `packages/t3-tape-npm` importer.
- `pnpm -C packages/t3-tape-npm build`: passed and emitted `dist/cli.js`, `dist/env.js`, `dist/platform.js`, and `dist/resolve.js`.
- `pnpm -C packages/t3-tape-npm test`: passed with `Test Files 1 passed` and `Tests 9 passed`. Cross-platform optional package warnings are expected on a Windows host.
- `cargo test -p t3-tape`: passed. Rust coverage now resolves to `11` init tests, `15` patch tests, `10` update tests, and `18` validate tests.
- `cargo build --release -p t3-tape`: passed. The release binary still builds cleanly.
- `target/release/t3-tape.exe init ...` then `target/release/t3-tape.exe validate --repo-root ...`: passed in a fresh temp git repo. `validate` returned `OK`.
- `powershell -ExecutionPolicy Bypass -File scripts/e2e.ps1`: passed. Final output included `E2E_STATUS:` then `COMPLETE`.

For future planning, treat this document plus the archived todo chain as the live source of truth when a pasted session prompt disagrees with the checked-in repo state. `todo/pending/` should be empty when the repo is in a completion-clean state.

## Shipped Boundaries

T3 Tape ships:
- a Rust CLI named `t3-tape`
- a hybrid PatchMD state surface: root registry at `.t3/patch.md` plus operational state under `.t3/patch/`
- PatchMD registry parsing and rendering
- validation and staged validation
- git-hook snippet generation and optional pre-commit installation
- update triage, agent-assisted resolution, sandbox preview, approval, and migration logging
- a publish-ready npm launcher package with per-target packaged binaries and a frozen-install-safe workspace setup
- docs, example fixture, CI workflows, and a scripted churn proof

T3 Tape does not ship:
- a GUI triage interface
- a hosted agent backend
- automatic current-branch merges

## Command Matrix

| Surface | Status | Notes |
|---|---|---|
| `t3-tape init` | shipped | idempotent PatchMD initialization inside shared `.t3/`, with repo-root and state-dir overrides |
| `t3-tape patch add` | shipped | atomic diff + meta + patch.md write flow |
| `t3-tape patch list` | shipped | stable text output of recorded patches |
| `t3-tape patch show` | shipped | supports `--diff` for diff surface inspection |
| `t3-tape patch import` | shipped | direct import plus deterministic clustering path for multi-file fixtures |
| `t3-tape hooks print` | shipped | prints pre-commit, `.gitignore`, and `.gitattributes` snippets |
| `t3-tape hooks install pre-commit` | shipped | optional hook installation with overwrite protection |
| `t3-tape validate` | shipped | validates only `.t3/patch.md` plus `.t3/patch/**`, enforces semantic meta parity, resolves refs in git, and tolerates foreign `.t3/*` siblings |
| `t3-tape validate --staged` | shipped | enforces the two-layer staged-write contract |
| `t3-tape update --ref` | shipped | phases 0-7 through sandbox worktree, dependency-aware staging, and triage persistence |
| `t3-tape update --ci` | shipped | writes artifacts and exits `3` when non-clean patches remain |
| `t3-tape triage` | shipped | human and JSON output from `.t3/patch/triage.json` |
| `t3-tape triage approve` | shipped | rewrites approved patch state immediately, refreshes diff-derived metadata, and advances the global PatchMD base only on terminal cycle completion |
| `t3-tape rederive` | shipped | forces intent-first re-derivation against the latest sandbox cycle |
| `t3-tape export` | shipped | emits a compact markdown summary of customizations |

## Delivery Chain

The canonical execution chain is:

| Unit | Role | State |
|---|---|---|
| `001a` | contract lock | archived |
| `001b` | repo bootstrap | archived |
| `001c` | store + init | archived |
| `001d` | patch registry | archived |
| `001e` | validation + hooks | archived |
| `001f` | update triage | archived |
| `001g` | agent + migration | archived |
| `001h` | distribution + vitest | archived |
| `001i` | docs + CI | archived |
| `001j` | e2e verification + closeout | archived |

The parent chain file `001-t3-tape-patchmd.md` is archived last after closeout.

Release-readiness note:
- the original 001 chain delivered the PatchMD CLI and launcher system
- the follow-up 002 chain reconciles launcher packaging parity, fresh-install reproducibility, and readiness evidence without reopening the product contract
- the follow-up 009 chain reconciles init repair and state-dir exclusion so PatchMD does not self-record or leave broken empty registry state behind
- the follow-up 010 chain narrows the filesystem contract so PatchMD behaves as one plugin inside `.t3`, not the owner of the whole namespace
- the follow-up 011 chain reconciles contract truthfulness for untracked patch capture, `state-root` validation, and installed hook portability

## Ownership Rules

Rust owns:
- CLI contracts
- state layout
- patch parsing and writing
- validation
- update orchestration
- migration approval

Node owns only:
- launcher resolution
- process forwarding
- packaged-binary metadata and publish ergonomics

This split is deliberate. There is no duplicate PatchMD implementation in Node.

## Release Readiness

Release-critical surfaces in this repo:
- `dist-workspace.toml` for cargo-dist artifact settings
- `packages/t3-tape-npm/` for the launcher package
- per-target packages under `packages/t3-tape-*`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

## Evidence Surfaces

Primary local gates:

```bash
cargo test -p t3-tape
pnpm -C packages/t3-tape-npm test
powershell -ExecutionPolicy Bypass -File scripts/e2e.ps1
```

Supplemental evidence:
- `examples/basic-fork/`
- `.t3/patch/migration.log` output from the churn script
- dupe-audit output over the final touched surface

Local-only surfaces:
- `.t3/reports/` is tolerated foreign output for ad hoc audits and is not part of the canonical shipped state
- `.docs/log.txt` is a local interaction log and is not part of the release artifact set
- `.docs/todo/` is a local changelog scratch surface and is not part of the release artifact set

## References

- [README](../README.md)
- [PatchMD protocol](patchmd.md)
- [Update flow](update-flow.md)
- [Agent contract](agent-contract.md)
