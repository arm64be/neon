use std::collections::HashMap;
use std::sync::Arc;

use mlua::{Function, Lua, RegistryKey, Result};
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

pub struct NeonState {
    pub runtime: Arc<Runtime>,
    pub shutdown_hooks: Vec<RegistryKey>,
    pub sqlite_connections: HashMap<String, SqlitePool>,
    pub default_session_db: Option<String>,
}

impl NeonState {
    pub fn new(runtime: Runtime) -> Self {
        Self {
            runtime: Arc::new(runtime),
            shutdown_hooks: Vec::new(),
            sqlite_connections: HashMap::new(),
            default_session_db: None,
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

pub fn register_sqlite_connection(lua: &Lua, id: String, pool: SqlitePool) {
    let mut state = lua
        .app_data_mut::<NeonState>()
        .expect("Neon runtime is not installed on this Lua state");
    state.sqlite_connections.insert(id, pool);
}

pub fn sqlite_connection(lua: &Lua, id: &str) -> Option<SqlitePool> {
    let state = lua
        .app_data_ref::<NeonState>()
        .expect("Neon runtime is not installed on this Lua state");
    state.sqlite_connections.get(id).cloned()
}

pub fn set_default_session_db(lua: &Lua, id: String) {
    let mut state = lua
        .app_data_mut::<NeonState>()
        .expect("Neon runtime is not installed on this Lua state");
    state.default_session_db = Some(id);
}

pub fn default_session_db(lua: &Lua) -> Option<String> {
    let state = lua
        .app_data_ref::<NeonState>()
        .expect("Neon runtime is not installed on this Lua state");
    state.default_session_db.clone()
}
