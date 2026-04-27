use std::collections::BTreeMap;

use mlua::{Lua, Result, Table, Value};

pub fn trim_string(value: String) -> String {
    value.trim().to_string()
}

pub fn env(name: String) -> Option<String> {
    std::env::var(name).ok()
}

pub fn env_or(name: String, default: String) -> String {
    env(name).unwrap_or(default)
}

fn normalized_arg_name(name: &str) -> String {
    if name.starts_with('-') {
        name.to_string()
    } else {
        format!("--{name}")
    }
}

fn load_args(lua: &Lua) -> Result<Vec<String>> {
    let globals = lua.globals();
    if let Ok(neon) = globals.get::<Table>("neon") {
        if let Ok(args) = neon.get::<Table>("args") {
            return args.sequence_values::<String>().collect();
        }
    }
    Ok(Vec::new())
}

pub fn arg_flag(lua: &Lua, name: String) -> Result<bool> {
    let needle = normalized_arg_name(&name);
    Ok(load_args(lua)?.iter().any(|arg| arg == &needle))
}

pub fn arg_value(lua: &Lua, name: String) -> Result<Option<String>> {
    let needle = format!("{}=", normalized_arg_name(&name));
    Ok(load_args(lua)?.into_iter().find_map(|arg| {
        arg.strip_prefix(&needle).map(|value| value.to_string())
    }))
}

pub fn arg_value_or(lua: &Lua, (name, default): (String, String)) -> Result<String> {
    Ok(arg_value(lua, name)?.unwrap_or(default))
}

pub fn arg_glob(lua: &Lua) -> Result<Vec<String>> {
    let args = load_args(lua)?;
    let mut glob = Vec::new();
    let mut seen_positional = false;

    for arg in args {
        if seen_positional {
            glob.push(arg);
            continue;
        }

        if arg.starts_with("--") {
            continue;
        }

        seen_positional = true;
        glob.push(arg);
    }

    Ok(glob)
}

pub fn json_encode(_lua: &Lua, value: Value) -> Result<String> {
    let json_value = lua_to_json(value)?;
    serde_json::to_string(&json_value).map_err(|err| mlua::Error::RuntimeError(err.to_string()))
}

pub fn json_decode(lua: &Lua, text: String) -> Result<Value> {
    let json_value: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
    json_to_lua(lua, json_value)
}

pub fn lua_to_json(value: Value) -> Result<serde_json::Value> {
    use serde_json::Value as Json;

    Ok(match value {
        Value::Nil => Json::Null,
        Value::Boolean(value) => Json::Bool(value),
        Value::Integer(value) => Json::Number(value.into()),
        Value::Number(value) => {
            Json::Number(serde_json::Number::from_f64(value).ok_or_else(|| {
                mlua::Error::RuntimeError("cannot encode non-finite number as JSON".into())
            })?)
        }
        Value::String(value) => Json::String(value.to_str()?.to_owned()),
        Value::Table(table) => table_to_json(table)?,
        other => {
            return Err(mlua::Error::RuntimeError(format!(
                "unsupported value for JSON encoding: {other:?}"
            )))
        }
    })
}

pub fn json_to_lua(lua: &Lua, value: serde_json::Value) -> Result<Value> {
    use serde_json::Value as Json;

    Ok(match value {
        Json::Null => Value::Nil,
        Json::Bool(value) => Value::Boolean(value),
        Json::Number(number) => {
            if let Some(value) = number.as_i64() {
                Value::Integer(value)
            } else if let Some(value) = number.as_u64() {
                if let Ok(value) = i64::try_from(value) {
                    Value::Integer(value)
                } else {
                    Value::Number(value as f64)
                }
            } else if let Some(value) = number.as_f64() {
                Value::Number(value)
            } else {
                Value::Nil
            }
        }
        Json::String(value) => Value::String(lua.create_string(&value)?),
        Json::Array(items) => {
            let table = lua.create_table()?;
            for (idx, item) in items.into_iter().enumerate() {
                table.set(idx + 1, json_to_lua(lua, item)?)?;
            }
            Value::Table(table)
        }
        Json::Object(entries) => {
            let table = lua.create_table()?;
            for (key, item) in entries {
                table.set(key, json_to_lua(lua, item)?)?;
            }
            Value::Table(table)
        }
    })
}

fn table_to_json(table: Table) -> Result<serde_json::Value> {
    use serde_json::{Map, Value as Json};

    let mut pairs = Vec::new();

    for pair in table.pairs::<Value, Value>() {
        let (key, value) = pair?;
        pairs.push((key, value));
    }

    let mut indexed = BTreeMap::new();
    let mut is_array = !pairs.is_empty();

    for (key, value) in &pairs {
        match key {
            Value::Integer(index) if *index > 0 => {
                indexed.insert(*index as usize, value.clone());
            }
            Value::Number(index) if index.fract() == 0.0 && *index > 0.0 => {
                indexed.insert(*index as usize, value.clone());
            }
            _ => {
                is_array = false;
                break;
            }
        }
    }

    if is_array {
        let expected_len = indexed.len();
        if expected_len > 0 && indexed.keys().copied().eq(1..=expected_len) {
            let mut array = Vec::with_capacity(expected_len);
            for (_, value) in indexed {
                array.push(lua_to_json(value)?);
            }
            return Ok(Json::Array(array));
        }
    }

    let mut map = Map::new();
    for (key, value) in pairs {
        let key = match key {
            Value::String(key) => key.to_str()?.to_owned(),
            Value::Integer(index) => index.to_string(),
            Value::Number(index) => index.to_string(),
            Value::Boolean(flag) => flag.to_string(),
            other => format!("{other:?}"),
        };
        map.insert(key, lua_to_json(value)?);
    }
    Ok(Json::Object(map))
}
