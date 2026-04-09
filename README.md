# T3 Tape

T3 Tape is a Rust CLI that implements PatchMD, an intent-preserving patch workflow for long-lived software forks.

It solves one specific problem: a plain diff is not enough to keep a customization alive when upstream refactors the code around it. T3 Tape keeps the executable patch, the human-readable intent, and the operational metadata together under one canonical state root so updates can be validated, triaged, re-applied, and approved without losing why the customization exists.

This is not duct tape. It is not Gorilla Tape either. It is T3 Tape.

## Why T3 Tape Exists

The product thesis came from Theo's PatchMD framing. The origin stays here on purpose because it explains the actual worldview behind the tool, not just the mechanics.

> The idea is a piece of software whose behavior characteristics and way of usage are defined by a text file that a normal person can edit in plain English. My proposal, and I genuinely hope this becomes a standard-ish thing, is PatchMD.
>
> Hear me out. I talked in my previous open source video about how much [redacted-user]'s way of building broke my brain. Watching him treat every package in our project as just additional code that he could edit however he wanted, and then use `patch-package` to apply his changes, was wild. It meant we could adjust a package to our needs and our use case.
>
> There are issues with this, though. If the package gets updated in the location of the code we changed, or something shifts, you have to rewrite the patch.
>
> So what would it look like to fix that? Say we added a feature in T3 code. We had a button in the corner that was customized toward the plugin. It would run a copy of T3 code separately on your machine. You could make changes to the source, it would act like a pseudo-dev mode, and you would see the new UI as the changes happened.
>
> Then upstream ships another hundred or thousand lines of code across five hundred commits, and all your stuff is broken. You can have an agent try to resolve the merge conflict, sure. But what happens when they fail?
>
> What if every time you made a customization, it didn't just change things in one place. It changed them in two. Obviously it would edit the code to do the thing you want. But it would also encode the intent of the change you made in a `patch.md` file. The `patch.md` simply describes all of the features you have added to the app.
>
> Then in the future, an update is not just downloading the new binary and hoping for the best. The future is: you pull down the changes from main. If it applies cleanly, awesome, go straight to it. If it doesn't, you can hit a button that says run update with agent. It will try to resolve the merge conflicts.
>
> And if it fails and has any issues, it will tell you: we weren't able to resolve this cleanly. If you want, we will run another instance in the background and reapply these features that you've added in the past. Then you can check and make sure they work how you want, and we'll migrate you when that's done.
>
> What if the update button was no longer just an update button? What if update was a process? It would clone the latest code, see which changes cleanly apply and which ones don't, and then let the agent guide you through getting the new version where you want it to be with your specific stuff added.
>
> In the `.t3` directory, you have this file that describes everything you wanted T3 code to be. What does a future look like where all software is self-forking, self-customizing, and self-healing when merge conflicts happen?
>
> Since I thought about this, I haven't been able to stop, and I'm too busy to build it myself. This is on the roadmap for things we want T3 code. It is a lot of little pieces we have to get right for this to work. But I wanted to coin the term PatchMD, because I firmly believe this is the path to allow normal people to edit their software to their liking.

## Start Here

T3 Tape gives a fork one durable state machine:

- `t3-tape init` creates one canonical state root at `.t3/`
- `t3-tape patch add` records a customization as diff plus intent plus metadata
- `t3-tape validate` enforces the two-layer rule before commits or automation steps
- `t3-tape update --ref <ref>` runs a sandboxed migration process against a new upstream ref
- `t3-tape triage` exposes the persisted review state for humans, bots, and UIs
- `t3-tape triage approve PATCH-001` rewrites PatchMD state only after review

If you are adopting this in a repo, CI job, internal platform, or agent workflow, the main idea is simple:

1. Keep `.t3/` in version control.
2. Treat `.t3/patch.md` as the human-readable contract.
3. Treat `.t3/patches/*.diff` and `.t3/patches/*.meta.json` as the machine-operational layer.
4. Treat `.t3/triage.json` and `.t3/migration.log` as the automation and audit surface.
5. Let agents help, but never let them become the source of truth.

## What T3 Tape Actually Owns

One T3 Tape-managed fork has one canonical state root:

```text
.t3/
  patch.md
  patches/
    PATCH-001.diff
    PATCH-001.meta.json
  sandbox/
    <timestamp>/
  triage.json
  migration.log
  config.json
```

Artifact purpose:

| Path | Purpose | Primary reader |
| --- | --- | --- |
| `.t3/patch.md` | plain-English registry of every customization | humans, agents |
| `.t3/patches/*.diff` | executable diff layer | git, update engine |
| `.t3/patches/*.meta.json` | operational metadata and confidence state | CLI, automation |
| `.t3/triage.json` | latest update-cycle read model | bots, dashboards, CI |
| `.t3/migration.log` | append-only audit trail | operators, audit tooling |
| `.t3/config.json` | local PatchMD config | CLI, hooks, agent bridge |
| `.t3/sandbox/` | ephemeral migration worktrees and staged artifacts | update flow only |

Rules that stay fixed:

- `.t3/` is the default generated state directory
- `.t3/sandbox/` is ephemeral and should be gitignored
- `.t3/reports/` and other foreign subdirectories are tolerated
- T3 Tape does not create a second registry, shadow database, or alternate state root

## How It Works

### 1. `init` resolves where state should live

By default T3 Tape resolves the repo root with `git rev-parse --show-toplevel`. If that fails, it falls back to the current working directory. The default state directory is then:

```text
<repo-root>/.t3
```

You can override either side of that resolution:

```bash
t3-tape --repo-root /path/to/repo init --upstream https://github.com/example/upstream --base-ref HEAD
t3-tape --state-dir /path/to/shared/state init --upstream https://github.com/example/upstream --base-ref HEAD
```

`init` creates missing directories and files without inventing anything extra:

```bash
t3-tape init --upstream https://github.com/example/upstream --base-ref HEAD
```

Initial files:

- `.t3/config.json`
- `.t3/patch.md`
- `.t3/migration.log`
- `.t3/triage.json`
- `.t3/patches/`
- `.t3/sandbox/`

### 2. A customization is recorded in two required layers

When you customize the fork, the change is not considered real until both of these exist:

1. code layer
   `.t3/patches/PATCH-###.diff`
2. intent layer
   `.t3/patch.md`

T3 Tape also writes one companion metadata file:

- `.t3/patches/PATCH-###.meta.json`

That means a patch is not just "some changed lines." It is:

- the diff
- the reason
- the behavior assertions
- the scope
- the refs and confidence metadata used during future migrations

### 3. `validate` enforces the contract

T3 Tape is strict on purpose. `validate` checks:

- required PatchMD files exist
- `patch.md` parses
- ids are unique
- statuses are valid
- diff and meta files match patch entries
- dependency references are real and acyclic
- staged code changes are paired with staged PatchMD updates when `--staged` is used

That is the enforcement point that turns PatchMD into a workflow instead of a note-taking convention.

### 4. `update` runs as a migration process, not a blind merge

`t3-tape update --ref <new-ref>` does not mutate your current branch and hope for the best.

It:

1. validates the current PatchMD state
2. records a started entry in `.t3/migration.log`
3. fetches the requested upstream ref
4. creates `.t3/sandbox/<timestamp>/`
5. creates a linked git worktree at `.t3/sandbox/<timestamp>/worktree`
6. classifies each active patch as `CLEAN`, `CONFLICT`, or `MISSING-SURFACE`
7. writes `.t3/triage.json` plus sandbox triage artifacts
8. optionally asks an external agent to resolve conflicts or re-derive missing surfaces
9. stages successful results on the sandbox migration branch
10. leaves final state changes pending human or policy approval

### 5. `triage` is the machine-readable review surface

After `update`, the current cycle is readable in two ways:

- `t3-tape triage`
- `t3-tape triage --json`

The same summary is also persisted at `.t3/triage.json`.

This is the surface your internal dashboard, CI job, Slack notifier, bot, or SaaS control plane should consume. Do not re-infer state from loose console logs when the structured read model already exists.

### 6. `triage approve` is the rewrite boundary

`t3-tape triage approve PATCH-001` is the point where PatchMD state is rewritten against the migrated upstream base.

Approval:

- rewrites `.t3/patches/PATCH-001.diff`
- updates `.t3/patches/PATCH-001.meta.json`
- rewrites the root `> base-ref:` in `.t3/patch.md`
- keeps the current branch untouched
- does not auto-merge the sandbox branch into your working branch

That separation matters. T3 Tape owns PatchMD state. Your git workflow still owns when and how code lands on your main development branch.

## Developer Quick Start

### Install from Rust

Inside this repo:

```bash
cargo build -p t3-tape
target/debug/t3-tape --help
```

Install into another machine or repo:

```bash
cargo install --path crates/t3-tape
t3-tape --help
```

### Install through npm

The launcher package lives in `packages/t3-tape-npm/` and is designed to ship as `@t3-tape/t3-tape`.

The launcher contract is strict:

- it resolves packaged platform binaries from `optionalDependencies`
- it does not download binaries over HTTP at runtime
- `T3_TAPE_BINARY_PATH` can override the binary for local development, CI, and repository tests

Local launcher usage in this repo:

```bash
pnpm install
pnpm -C packages/t3-tape-npm build
T3_TAPE_BINARY_PATH=target/debug/t3-tape node packages/t3-tape-npm/dist/cli.js --help
```

On Windows, point the override at `target\\debug\\t3-tape.exe`.

## Daily Workflow

### Greenfield fork

```bash
t3-tape init --upstream https://github.com/example/upstream --base-ref HEAD

# make your code changes

t3-tape patch add \
  --title "settings-toolbar-button" \
  --intent "Add a toolbar button that opens plugin settings." \
  --assert "toolbar button is visible" \
  --assert "clicking the button opens plugin settings"

t3-tape validate
```

### Import an existing manual diff

```bash
git diff <base-ref> HEAD > existing.diff
t3-tape patch import --diff existing.diff --title "legacy-customization"
t3-tape validate
```

### Migrate to a new upstream version

```bash
t3-tape update --ref v1.2.3
t3-tape triage
t3-tape triage approve PATCH-001
```

### Force re-derivation for one patch

```bash
t3-tape rederive PATCH-001
t3-tape triage
```

### Export a human summary

```bash
t3-tape export --format markdown --output CUSTOMIZATIONS.md
```

## Command Surface

```text
t3-tape init --upstream <repo-url> --base-ref <ref>
t3-tape patch add --title <text> [--intent <text>|--intent-file <path>] [--staged] [--surface <text>] [--assert <text>...]
t3-tape patch list
t3-tape patch show PATCH-001 [--diff]
t3-tape patch import --diff <path> [--title <text>] [--intent <text>|--intent-file <path>] [--surface <text>]
t3-tape hooks print <pre-commit|gitignore|gitattributes>
t3-tape hooks install pre-commit [--force]
t3-tape validate [--staged] [--json]
t3-tape update --ref <new-ref> [--ci] [--confidence-threshold <float>]
t3-tape triage [--json]
t3-tape triage approve PATCH-001
t3-tape rederive PATCH-001
t3-tape export --format markdown --output CUSTOMIZATIONS.md
```

Stable exit-code categories:

- `0` success
- `1` usage or malformed input
- `2` validation failure
- `3` blocked workflow or review-required state
- `4` git failure
- `5` agent failure

Those categories are intentionally useful for automation. A CI system can treat exit code `3` as "human review required" instead of "tool crashed."

## Automation

T3 Tape fits anywhere that can run a command and read files.

Most integrations only need three moves:

1. run a `t3-tape` command
2. read JSON or files under `.t3/`
3. branch on the exit code

### Core contract

Stable machine surfaces:

- `t3-tape validate --json`
- `t3-tape triage --json`
- `.t3/triage.json`
- `.t3/migration.log`
- `.t3/sandbox/<timestamp>/resolved/`
- exit codes `0-5`

Exit code meaning:

- `0` success
- `2` validation failed
- `3` review or operator action required
- `4` git failed
- `5` agent provider failed

### Fastest setup

If you just want this working:

| If you are building | Run | Read |
| --- | --- | --- |
| pre-commit hook | `t3-tape validate --staged` | exit code |
| PR validation | `t3-tape validate --json` | JSON output |
| nightly drift check | `t3-tape update --ref "$UPSTREAM_REF" --ci --confidence-threshold 0.90` | `.t3/triage.json` |
| agent app or bot | `t3-tape triage --json` | JSON output |
| review UI or SaaS control plane | `t3-tape triage --json` | `.t3/triage.json`, `.t3/migration.log` |

That is enough to integrate T3 Tape without touching Rust internals.

### Integration patterns

| Integration | Command shape | What to do with the result |
| --- | --- | --- |
| git hook | `t3-tape validate --staged` | block commits missing PatchMD state |
| shell script | any command | branch on stdout and exit code |
| CI validation | `t3-tape validate --json` | fail on `2`, surface JSON in logs or artifacts |
| scheduled updater | `t3-tape update --ref ... --ci` | succeed on `0`, open review flow on `3` |
| agent worker | `t3-tape triage --json` | inspect pending patches and resolved artifacts |
| review dashboard | read `.t3/triage.json` | render patch status, confidence, notes, unresolved assertions |
| control plane | `update`, `triage`, `triage approve` | orchestrate review without rewriting PatchMD files directly |
| chatops bot | `triage --json` plus file reads | post summaries to Slack, Discord, Jira, Linear |

### Drop-in recipes

Shell:

```bash
t3-tape validate --json
```

Cron or scheduler:

```bash
t3-tape update --ref "$UPSTREAM_REF" --ci --confidence-threshold 0.90
```

Agent app:

```bash
t3-tape triage --json
```

Human approval:

```bash
t3-tape triage approve PATCH-001
```

### CI/CD examples

GitHub Actions:

```yaml
- name: Validate PatchMD state
  run: t3-tape validate --json

- name: Check upstream drift
  run: t3-tape update --ref ${{ env.UPSTREAM_REF }} --ci --confidence-threshold 0.90
  env:
    T3_TAPE_AGENT_AUTH_TOKEN: ${{ secrets.T3_TAPE_AGENT_AUTH_TOKEN }}
```

GitLab CI:

```yaml
patchmd_validate:
  script:
    - t3-tape validate --json

patchmd_update:
  script:
    - t3-tape update --ref "$UPSTREAM_REF" --ci --confidence-threshold 0.90
```

Jenkins, Buildkite, or any shell-based runner:

```bash
t3-tape update --ref "$UPSTREAM_REF" --ci --confidence-threshold 0.90
case $? in
  0) echo "clean" ;;
  3) echo "review required" ;;
  *) echo "failed" ; exit 1 ;;
esac
```

### Agentic and SaaS use

T3 Tape works well as the local state engine under:

- agent workers that resolve conflicts through `exec` or `http`
- internal developer platforms
- review UIs backed by `.t3/triage.json`
- SaaS control planes that trigger update cycles and expose approval flows
- chatops and notification bots

The important boundary is fixed:

- external systems may run commands, read artifacts, and call approval
- external systems should not rewrite `.t3/patch.md`, patch diffs, or metadata directly

T3 Tape stays the source of truth.

## Runnable Agentic Examples

The repo includes runnable example apps that show how to put T3 Tape inside real automation and review loops by reading `.t3` state and invoking `t3-tape` commands, not by custom scripts mutating `.t3/patch.md`, patch diffs, or metadata directly.

Shared helper layer:

- [`examples/agent-kit/README.md`](examples/agent-kit/README.md)
  small example-side adapter that reads real `.t3` state and builds follow-up `t3-tape` commands

Agentic examples:

- [`examples/agent-handoff-builder/README.md`](examples/agent-handoff-builder/README.md)
  turns unresolved patches into agent job packets for conflict resolution and re-derivation queues
- [`examples/migration-review-assistant/README.md`](examples/migration-review-assistant/README.md)
  turns triage plus assertion output into review findings, approval candidates, and operator comments
- [`examples/fleet-upgrade-coordinator/README.md`](examples/fleet-upgrade-coordinator/README.md)
  plans which patched forks should update now, later, or not at all across a fleet

Other workflow examples:

- [`examples/dev-env-doctor/README.md`](examples/dev-env-doctor/README.md)
  catches onboarding and CI environment drift
- [`examples/test-impact-planner/README.md`](examples/test-impact-planner/README.md)
  maps changed files to the right test commands and owners
- [`examples/release-note-router/README.md`](examples/release-note-router/README.md)
  converts commit streams into grouped release notes and version bump guidance

See [`examples/README.md`](examples/README.md) for run commands and test counts.

### Hook and env contract

Hooks are configured in `.t3/config.json`:

- `pre-patch`
- `post-patch`
- `pre-update`
- `post-update`
- `on-conflict`

Hook environment variables:

| Hook point | Environment variables |
| --- | --- |
| `pre-patch` | `T3_TAPE_REPO_ROOT`, `T3_TAPE_STATE_DIR` |
| `post-patch` | `T3_TAPE_REPO_ROOT`, `T3_TAPE_STATE_DIR`, `T3_TAPE_PATCH_IDS` |
| `pre-update` | `T3_TAPE_REPO_ROOT`, `T3_TAPE_STATE_DIR`, `T3_TAPE_SANDBOX_PATH` |
| `on-conflict` | `T3_TAPE_REPO_ROOT`, `T3_TAPE_STATE_DIR`, `T3_TAPE_SANDBOX_PATH`, `T3_TAPE_TRIAGE_PATH` |
| `post-update` | `T3_TAPE_REPO_ROOT`, `T3_TAPE_STATE_DIR`, `T3_TAPE_SANDBOX_PATH`, `T3_TAPE_TRIAGE_PATH` |

Common environment variables:

- `T3_TAPE_BINARY_PATH`
- `T3_TAPE_AGENT_AUTH_TOKEN`
- `T3_TAPE_AUTHOR`

### Agent providers

T3 Tape can call an external agent through `.t3/config.json`.

- `exec`
  local script, worker, container entrypoint
- `http`
  internal API, SaaS endpoint, orchestration service

Exec contract:

- stdin gets one JSON request
- stdout returns one JSON response
- exit `0` means success

HTTP contract:

- `POST <agent.endpoint>`
- `Content-Type: application/json`
- `Authorization: Bearer <token>` when `T3_TAPE_AGENT_AUTH_TOKEN` is set

The agent helps with conflict resolution and re-derivation. It does not own patch history, approval, or state rewrites.

## Delivery Status

As of the 2026-04-09 closeout plus the same-day 002 release-readiness reconciliation:

- execution units `001a` through `001j` are the canonical delivery chain for PatchMD v0.1.0
- the shipped CLI surface is complete for `init`, `patch add|list|show|import`, `hooks print|install`, `validate`, `update`, `triage`, `triage approve`, `rederive`, and `export`
- the npm launcher package is publish-ready, frozen-install-safe, and covered with `vitest` (`Test Files 1 passed`, `Tests 9 passed` in the current launcher suite)
- the remaining roadmap after this repo state is UI and hosted-service evolution, not missing CLI fundamentals

The detailed status matrix lives in [`docs/implementation-status.md`](docs/implementation-status.md).

## Validation and Tests

Rust:

```bash
cargo test -p t3-tape
```

Launcher:

```bash
pnpm -C packages/t3-tape-npm test
```

Runnable examples:

```bash
pnpm run test:examples
```

End-to-end churn proof:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/e2e.ps1
```

## Docs Index

- [`ARCHITECTURE.md`](ARCHITECTURE.md) - executive architecture blueprint with Mermaid diagrams for the full T3 Tape system and pipeline
- [`docs/patchmd.md`](docs/patchmd.md) - canonical PatchMD protocol and file contracts
- [`docs/update-flow.md`](docs/update-flow.md) - phase-by-phase update and approval model
- [`docs/agent-contract.md`](docs/agent-contract.md) - external agent provider contract and payload schemas
- [`docs/implementation-status.md`](docs/implementation-status.md) - command matrix, delivery chain, and shipped boundaries
- [`examples/README.md`](examples/README.md) - runnable example apps and the pain points they target
- [`packages/t3-tape-npm/README.md`](packages/t3-tape-npm/README.md) - launcher-focused package notes
- [`examples/basic-fork/README.md`](examples/basic-fork/README.md) - committed example state for adopters

## Security Notes

- Agent endpoints receive source code. Private repos should use self-hosted or private agent infrastructure.
- `T3_TAPE_AGENT_AUTH_TOKEN` is read from the environment for HTTP providers. Credentials are not stored in `.t3/config.json`.
- Sandbox worktrees should never point at production data or credentials.
- `migration.log` is append-only operational history, not a place for secrets.

## References

- [cargo-dist config reference](https://axodotdev.github.io/cargo-dist/book/reference/config.html)
- [Git worktree documentation](https://git-scm.com/docs/git-worktree)
- [Git apply documentation](https://git-scm.com/docs/git-apply)
- [Vitest guide](https://vitest.dev/guide/)
- [actions/setup-node](https://github.com/actions/setup-node)
- [pnpm/action-setup](https://github.com/pnpm/action-setup)
