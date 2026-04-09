# Agent Kit

`examples/agent-kit/` is the shared automation layer for the agentic examples in this repo.

It is intentionally small:

- it reads the real `.t3` files T3 Tape already owns
- it normalizes the state into JavaScript-friendly shapes
- it builds `t3-tape` command strings for follow-up automation

It does not replace T3 Tape. It does not write PatchMD state. It is only a thin example-side helper layer for bots, schedulers, review workers, and control-plane code.

Run tests:

```bash
pnpm -C examples/agent-kit test
```

Test coverage:
- `87` Vitest cases covering PatchMD parsing, state normalization, command generation, and failure handling
