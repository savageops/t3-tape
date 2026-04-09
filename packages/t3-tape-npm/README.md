# `@t3-tape/t3-tape`

Thin npm launcher for the `t3-tape` Rust binary.

The launcher never downloads binaries at runtime. It resolves a packaged platform binary from `optionalDependencies`, or it uses `T3_TAPE_BINARY_PATH` when you want to point at a locally built binary during development, CI, or repository tests.

Development override:

```bash
T3_TAPE_BINARY_PATH=../../target/debug/t3-tape pnpm exec t3-tape --help
```

Supported packaged targets:
- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`