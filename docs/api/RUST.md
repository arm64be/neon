# Rust API

## `Neon`

Defined in [`src/lib.rs`](../../src/lib.rs).

- `Neon::new() -> mlua::Result<Self>` creates a Lua state with the Neon module installed.
- `Neon::lua(&self) -> &Lua` returns the underlying Lua state.
- `Neon::set_args(&self, args: &[String]) -> mlua::Result<()>` populates `neon.args`.
- `Neon::set_config_root(&self, root: impl AsRef<Path>) -> mlua::Result<()>` sets `neon.config_root` and extends `package.path`.
- `Neon::exec_source(&self, source: &str, name: &str) -> mlua::Result<()>` executes Lua source.
- `Neon::shutdown(&self) -> mlua::Result<()>` runs registered shutdown hooks.
