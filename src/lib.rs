pub mod net;
pub mod runtime;
pub mod session;
pub mod tools;
pub mod util;

use std::path::Path;

use mlua::{Lua, LuaOptions, Result, StdLib, Table};

pub struct Neon {
    lua: Lua,
}

impl Neon {
    pub fn new() -> Result<Self> {
        let lua = Lua::new_with(StdLib::ALL_SAFE ^ StdLib::COROUTINE, LuaOptions::default())?;
        let runtime = runtime::new_runtime()?;
        runtime::install(&lua, runtime);
        register_module(&lua)?;
        Ok(Self { lua })
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    pub fn set_args(&self, args: &[String]) -> Result<()> {
        let module: Table = self.lua.globals().get("neon")?;
        let table = self.lua.create_table()?;
        for (idx, arg) in args.iter().enumerate() {
            table.set(idx + 1, arg.as_str())?;
        }
        module.set("args", table)?;
        Ok(())
    }

    pub fn set_config_root(&self, root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        let module: Table = self.lua.globals().get("neon")?;
        module.set("config_root", root.to_string_lossy().as_ref())?;

        let package: Table = self.lua.globals().get("package")?;
        let current_path: String = package.get("path")?;
        let prefix = format!(
            "{}/?.lua;{}/?/init.lua;",
            root.to_string_lossy(),
            root.to_string_lossy()
        );
        package.set("path", format!("{prefix}{current_path}"))?;
        Ok(())
    }

    pub fn exec_source(&self, source: &str, name: &str) -> Result<()> {
        self.lua.load(source).set_name(name).exec()
    }

    pub fn shutdown(&self) -> Result<()> {
        let hooks = runtime::take_shutdown_hooks(&self.lua);
        for hook_key in hooks {
            let hook: mlua::Function = self.lua.registry_value(&hook_key)?;
            hook.call::<()>(())?;
        }
        Ok(())
    }
}

pub fn register_module(lua: &Lua) -> Result<Table> {
    let module = lua.create_table()?;

    let new_session = lua.create_function(|lua, name: Option<String>| session::Session::new(lua, name))?;
    module.set("new_session", new_session)?;

    let util = lua.create_table()?;
    util.set("trim_string", lua.create_function(|_, value: String| Ok(crate::util::trim_string(value)))?)?;
    util.set(
        "arg_flag",
        lua.create_function(|lua, name: String| crate::util::arg_flag(lua, name))?,
    )?;
    util.set(
        "arg_value",
        lua.create_function(|lua, name: String| crate::util::arg_value(lua, name))?,
    )?;
    util.set(
        "arg_value_or",
        lua.create_function(|lua, args: (String, String)| crate::util::arg_value_or(lua, args))?,
    )?;
    util.set(
        "arg_glob",
        lua.create_function(|lua, ()| crate::util::arg_glob(lua))?,
    )?;
    module.set("util", util)?;

    let json = lua.create_table()?;
    json.set(
        "encode",
        lua.create_function(|lua, value: mlua::Value| crate::util::json_encode(lua, value))?,
    )?;
    json.set(
        "decode",
        lua.create_function(|lua, text: String| crate::util::json_decode(lua, text))?,
    )?;
    module.set("json", json)?;

    let tokio_table = lua.create_table()?;
    tokio_table.set("sleep", lua.create_function(|lua, ms: u64| crate::net::sleep(lua, ms))?)?;
    tokio_table.set(
        "http",
        lua.create_function(|lua, (method, url, headers, params, body): (
            String,
            String,
            Option<Table>,
            Option<Table>,
            Option<mlua::Value>,
        )| crate::net::http(lua, method, url, headers, params, body))?,
    )?;
    tokio_table.set(
        "http_stream",
        lua.create_function(
            |lua,
             (method, url, headers, params, body, on_line): (
                String,
                String,
                Option<Table>,
                Option<Table>,
                Option<mlua::Value>,
                mlua::Function,
            )| crate::net::http_stream(lua, method, url, headers, params, body, on_line),
        )?,
    )?;
    module.set("tokio", tokio_table.clone())?;
    module.set("net", tokio_table.clone())?;

    let tools_table = lua.create_table()?;
    tools_table.set(
        "read_file",
        lua.create_function(|lua, path: String| tools::read_file(lua, path))?,
    )?;
    tools_table.set(
        "write_file",
        lua.create_function(|lua, (path, content): (String, String)| tools::write_file(lua, path, content))?,
    )?;
    tools_table.set("bash", lua.create_function(|lua, command: String| tools::bash(lua, command))?)?;
    module.set("tools", tools_table)?;
    module.set("env", lua.create_function(|_, name: String| Ok(std::env::var(name).ok()))?)?;
    module.set(
        "env_or",
        lua.create_function(|_, (name, default): (String, String)| Ok(std::env::var(name).unwrap_or(default)))?,
    )?;

    let lifecycle = lua.create_table()?;
    lifecycle.set(
        "on_shutdown",
        lua.create_function(|lua, func: mlua::Function| runtime::add_shutdown_hook(lua, func))?,
    )?;
    module.set("lifecycle", lifecycle)?;

    let globals = lua.globals();
    globals.set("neon", module.clone())?;
    let args = lua.create_table()?;
    module.set("args", args.clone())?;

    if let Ok(package) = globals.get::<Table>("package") {
        if let Ok(preload) = package.get::<Table>("preload") {
            let module_clone = module.clone();
            preload.set("neon", lua.create_function(move |_, ()| Ok(module_clone.clone()))?)?;
        }
    }

    Ok(module)
}
