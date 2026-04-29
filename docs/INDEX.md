# Documentation Index

Neon is a minimal Rust + Lua agent harness. This documentation is split into a small set of focused pages:

- [Installation](installation.md)
- [API Reference](api/index.md)
- [Troubleshooting](troubleshooting.md)

## Quick Start

1. Install the Rust toolchain.
2. Run `NEON_CONFIG_ROOT=configs/onboarding cargo run` for first-time setup.
3. Run `NEON_CONFIG_ROOT=configs/preset cargo run` to use the ready-to-build harness directly.

If you set `NEON_CONFIG_ROOT`, Neon loads `config.lua` from that directory instead.

If a `.env` file exists alongside `config.lua`, Neon loads additive `KEY=VALUE` pairs from it before running the config.

The repository starter configs live in `configs/onboarding` and `configs/preset`, with shared themes in `configs/themes`.

Onboarding is split into small modules under `configs/onboarding/questions`, and the preset is split by responsibility under `configs/preset/tools`, `configs/preset/providers`, `configs/preset/interface`, `configs/preset/sessions.lua`, `configs/preset/cli.lua`, and `configs/preset/agent.lua`.

## Runtime Shape

- Rust owns the session state, tool registry, and execution loop.
- Lua owns the model provider and the user-facing interface.
- `Neon` owns the Lua state, Tokio runtime, and lifecycle hooks.
- Sessions carry history, mutable context, tools, and hooks.
