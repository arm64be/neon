use std::sync::Arc;

use mlua::{Function, Lua, RegistryKey, Result};
use tokio::runtime::Runtime;

pub struct NeonState {
    pub runtime: Arc<Runtime>,
    pub shutdown_hooks: Vec<RegistryKey>,
}

impl NeonState {
    pub fn new(runtime: Runtime) -> Self {
        Self {
            runtime: Arc::new(runtime),
            shutdown_hooks: Vec::new(),
        }
    }
}

pub fn new_runtime() -> Result<Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))
}

pub fn install(lua: &Lua, runtime: Runtime) {
    lua.set_app_data(NeonState::new(runtime));
}

pub fn block_on<F>(lua: &Lua, future: F) -> F::Output
where
    F: std::future::Future,
{
    let state = lua
        .app_data_ref::<NeonState>()
        .expect("Neon runtime is not installed on this Lua state");
    state.runtime.block_on(future)
}

pub fn add_shutdown_hook(lua: &Lua, func: Function) -> Result<()> {
    let key = lua.create_registry_value(func)?;
    let mut state = lua
        .app_data_mut::<NeonState>()
        .expect("Neon runtime is not installed on this Lua state");
    state.shutdown_hooks.push(key);
    Ok(())
}

pub fn take_shutdown_hooks(lua: &Lua) -> Vec<RegistryKey> {
    let mut state = lua
        .app_data_mut::<NeonState>()
        .expect("Neon runtime is not installed on this Lua state");
    std::mem::take(&mut state.shutdown_hooks)
}
