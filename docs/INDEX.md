# Documentation Index

Neon is a minimal Rust + Lua agent harness. This documentation is split into a small set of focused pages:

- [Installation](installation.md)
- [API Reference](api/index.md)
- [Troubleshooting](troubleshooting.md)

## Quick Start

1. Install the Rust toolchain.
2. Run `cargo run` from the repository root.
3. Neon loads `config.lua` from the active config root.

If you set `NEON_CONFIG_ROOT`, Neon loads `config.lua` from that directory instead.

If a `.env` file exists alongside `config.lua`, Neon loads additive `KEY=VALUE` pairs from it before running the config.

## Runtime Shape

- Rust owns the session state, tool registry, and execution loop.
- Lua owns the model provider and the user-facing interface.
- `Neon` owns the Lua state, Tokio runtime, and lifecycle hooks.
- Sessions carry history, mutable context, tools, and hooks.
