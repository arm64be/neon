pub mod session;
pub mod net;
pub mod util;
pub mod tools;

use mlua::{Lua, LuaOptions, Result, StdLib, Table};

pub fn create_lua() -> Result<Lua> {
    let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
    register_module(&lua)?;
    Ok(lua)
}

pub fn register_module(lua: &Lua) -> Result<Table> {
    let module = lua.create_table()?;

    let new_session = lua.create_function(|lua, name: Option<String>| {
        session::Session::new(lua, name)
    })?;
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

    let net = lua.create_table()?;
    net.set(
        "http",
        lua.create_function(|lua, (method, url, headers, params, body): (
            String,
            String,
            Option<Table>,
            Option<Table>,
            Option<mlua::Value>,
        )| crate::net::http(lua, method, url, headers, params, body))?,
    )?;
    net.set(
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
    module.set("net", net)?;

    module.set("read_file", lua.create_function(|_, path: String| tools::read_file(path))?)?;
    module.set(
        "write_file",
        lua.create_function(|_, (path, content): (String, String)| tools::write_file(path, content))?,
    )?;
    module.set("bash", lua.create_function(|_, command: String| tools::bash(command))?)?;
    module.set(
        "env",
        lua.create_function(|_, name: String| Ok(std::env::var(name).ok()))?,
    )?;
    module.set(
        "env_or",
        lua.create_function(|_, (name, default): (String, String)| {
            Ok(std::env::var(name).unwrap_or(default))
        })?,
    )?;

    let globals = lua.globals();
    globals.set("neon", module.clone())?;
    let args = lua.create_table()?;
    globals.set("arg", args.clone())?;
    module.set("args", args.clone())?;

    if let Ok(package) = globals.get::<Table>("package") {
        if let Ok(preload) = package.get::<Table>("preload") {
            let module_clone = module.clone();
            preload.set(
                "neon",
                lua.create_function(move |_, ()| Ok(module_clone.clone()))?,
            )?;
        }
    }

    Ok(module)
}

pub fn set_args(lua: &Lua, args: &[String]) -> Result<()> {
    let module: Table = lua.globals().get("neon")?;
    let table = lua.create_table()?;
    for (idx, arg) in args.iter().enumerate() {
        table.set(idx + 1, arg.as_str())?;
    }
    lua.globals().set("arg", table.clone())?;
    module.set("args", table)?;
    Ok(())
}
