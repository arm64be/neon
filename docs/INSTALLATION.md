# Installation

## Requirements

- Rust 2021 edition toolchain
- A working `bash` shell for the built-in `neon.tools.bash(...)` helper

## Build

From the repository root:

```bash
cargo build
```

## Run

To run Neon with the default config discovery:

```bash
cargo run
```

To point Neon at a specific config root:

```bash
NEON_CONFIG_ROOT=/path/to/config cargo run
```

## Config Root Resolution

`src/main.rs` resolves the config root in this order:

1. `NEON_CONFIG_ROOT`, if set
2. In debug builds, the current directory
3. In non-debug builds, `$XDG_CONFIG_HOME/neon` if it contains `config.lua`
4. In non-debug builds, `$HOME/.config/neon` if it contains `config.lua`

The executable expects a `config.lua` file in the selected root.

## `.env` Support

If a `.env` file exists beside `config.lua`, Neon reads it before executing the config.

- Blank lines and comment lines are ignored.
- `export KEY=VALUE` is accepted.
- Existing environment variables are not overwritten.
