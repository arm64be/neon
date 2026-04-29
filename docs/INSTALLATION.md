# Installation

## Requirements

- Rust 2021 edition toolchain
- A working `bash` shell for the built-in `neon.tools.bash(...)` helper

## Build

From the repository root:

```bash
cargo build
```

## Install Latest Release

For a curl-pipe install:

```bash
curl -fsSL https://raw.githubusercontent.com/arm64be/neon/main/install.sh | bash
```

The installer downloads the latest GitHub release binary, installs it to `~/.local/bin/neon`, prepares onboarding config, and runs Neon once to start onboarding.

Config is installed at the first available location:

1. `$NEON_APP`, if set
2. `$XDG_CONFIG_HOME/neon`, if set
3. `$HOME/.config/neon`

To use a checked-out repository instead of GitHub releases, set `NEON_LOCAL_REPO=/path/to/neon` before piping the installer. In that mode, the script copies config files from the local repo and runs `cargo run` from that checkout.

## Run

To run Neon with the default config discovery:

```bash
cargo run
```

To run the guided first-time setup:

```bash
NEON_CONFIG_ROOT=configs/onboarding cargo run
```

To run the default ready-to-build preset:

```bash
NEON_CONFIG_ROOT=configs/preset cargo run
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

Repository starter configs live under `configs/`:

- `configs/onboarding` contains the first-time setup flow.
- `configs/preset` contains the default agentic harness configuration.
- `configs/themes` contains shared theme definitions used by onboarding and the preset.

Onboarding writes generated local files after setup:

- `.env` for selected provider secrets.
- `user_data.lua` for non-secret preferences such as name, theme, and custom instructions.
- `providers/selected.lua` for selected provider schemas.

## `.env` Support

If a `.env` file exists beside `config.lua`, Neon reads it before executing the config.

- Blank lines and comment lines are ignored.
- `export KEY=VALUE` is accepted.
- Existing environment variables are not overwritten.
