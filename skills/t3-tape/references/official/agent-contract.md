# Agent Contract

## Purpose

T3 Tape does not embed a hosted agent service. It talks to an external provider through a narrow, versioned contract so the Rust binary stays the canonical owner of PatchMD behavior.

Supported provider kinds:
- `none`
- `http`
- `exec`

Provider selection happens from `.t3/patch/config.json`:
- if `agent.endpoint` is empty, provider kind is `none`
- if `agent.provider` is `http`, use HTTP
- if `agent.provider` is `exec`, use local command execution
- if `agent.provider` is omitted, endpoints beginning with `http://` or `https://` are treated as HTTP; anything else is treated as an exec command

## Config Surface

```json
{
  "agent": {
    "provider": "exec",
    "endpoint": "powershell -File .t3/patch/agent-stub.ps1",
    "confidence-threshold": 0.8,
    "max-attempts": 3
  }
}
```

Field meaning:
- `provider`: optional explicit provider kind
- `endpoint`: URL for HTTP or shell command for exec
- `confidence-threshold`: approval gate for resolved or re-derived diffs
- `max-attempts`: retry budget persisted into `.meta.json`

Agent execution semantics:
- requests are executed sequentially in the current implementation
- output ordering is deterministic because the update engine plans patches in dependency order

## Authentication

HTTP providers support one built-in auth channel:
- `T3_TAPE_AGENT_AUTH_TOKEN`

When the variable is set and non-empty, T3 Tape sends:

```text
Authorization: Bearer <token>
```

Secrets rules:
- credentials are not written to `.t3/patch/config.json`
- credentials are not written to `triage.json`
- credentials are not written to `.t3/patch/sandbox/<timestamp>/resolved/*.json`
- stderr or notes persisted to disk should never contain the token value

## Request Payloads

All request bodies are JSON with `kebab-case` field names. Protocol version is currently `0.1.0`.

### Conflict Resolution

Sent for patches classified as `CONFLICT`.

```json
{
  "mode": "conflict-resolution",
  "patch-id": "PATCH-001",
  "intent": "Keep the forked line change when upstream rewrites the same line.",
  "behavior-assertions": [
    "the forked line still renders patched text"
  ],
  "original-diff": "diff --git ...",
  "upstream-diff": "diff --git ...",
  "new-source": "FILE: src/app.txt\n..."
}
```

Expected response:

```json
{
  "resolved-diff": "diff --git ...",
  "confidence": 0.93,
  "notes": "Reapplied the fork intent against the upstream rewrite.",
  "unresolved": []
}
```

### Re-derivation

Sent for patches classified as `MISSING-SURFACE` or forced through `t3-tape rederive PATCH-001`.

```json
{
  "mode": "re-derivation",
  "patch-id": "PATCH-001",
  "intent": "Recreate the behavior when the original file disappears upstream.",
  "behavior-assertions": [
    "the replacement file exists",
    "the patched behavior still renders"
  ],
  "new-source": "FILE: src/app.txt\n...",
  "surface-hint": "src/app.txt"
}
```

Expected response:

```json
{
  "derived-diff": "diff --git ...",
  "confidence": 0.92,
  "scope-update": {
    "files": ["src/app.txt"],
    "components": []
  },
  "notes": "Recreated the missing surface from intent.",
  "unresolved": []
}
```

### Intent Assist

The schema is shipped even though the repo's core closeout does not require automatic write-time intent synthesis.

```json
{
  "mode": "intent-assist",
  "diff": "diff --git ...",
  "context": "FILE: src/app.txt\n..."
}
```

Expected response:

```json
{
  "suggested-title": "settings-toolbar-button",
  "suggested-intent": "Add a settings button so plugin configuration is reachable from the toolbar.",
  "suggested-assertions": [
    "button is visible in toolbar",
    "button opens settings panel"
  ],
  "suggested-surface": "src/toolbar.tsx",
  "suggested-scope": {
    "files": ["src/toolbar.tsx"],
    "components": ["Toolbar"]
  }
}
```

## Truncation Rules

Large `new-source` payloads are truncated before the agent call.

Current rule:
- maximum serialized source payload: `64 KiB`

When truncation occurs:
- T3 Tape appends a deterministic marker to the request payload:

```text
[truncated by t3-tape]
```

- the persisted notes file also appends a deterministic explanation:

```text
[t3-tape truncated new-source before sending the request]
```

This keeps operator-visible artifacts honest about what the agent actually saw.

## Exec Provider Contract

Exec mode runs a shell command and pipes the JSON request to stdin.

Runtime behavior:
- Windows: `cmd /C <endpoint>`
- Unix: `sh -lc <endpoint>`

Requirements for the command:
- read or ignore stdin safely
- emit exactly one JSON response object to stdout
- return exit code `0` on success
- emit actionable stderr on failure

Example Windows stub:

```cmd
@echo off
type "%~dp0conflict-response.json"
```

## HTTP Provider Contract

HTTP mode issues:
- `POST <agent.endpoint>`
- `Content-Type: application/json`

When `T3_TAPE_AGENT_AUTH_TOKEN` exists:
- add `Authorization: Bearer <token>`

Failure behavior:
- non-2xx responses are surfaced as agent failures
- invalid JSON responses are surfaced as agent failures

## Persisted Artifacts

When an agent response meets the confidence threshold, T3 Tape stages the result under:

```text
.t3/patch/sandbox/<timestamp>/resolved/
  PATCH-001.diff
  PATCH-001.notes.txt
  PATCH-001.json
```

Persisted files contain:
- the resolved or derived diff
- operator-visible notes
- the raw parsed agent response

These artifacts are inputs to later approval, not a replacement for PatchMD state.

## References

- [PatchMD protocol doc](patchmd.md)
- [Update flow doc](update-flow.md)
- [reqwest blocking client docs](https://docs.rs/reqwest/latest/reqwest/blocking/)
