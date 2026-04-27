use std::io::{BufRead, BufReader};

use mlua::{Function, Lua, Result, Table, Value};
use reqwest::{
    blocking::{Client, Response},
    header::{HeaderMap, HeaderName, HeaderValue},
    Method, Url,
};

use crate::util;

fn parse_headers(table: Option<Table>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    if let Some(table) = table {
        for pair in table.pairs::<Value, Value>() {
            let (key, value) = pair?;
            let key = match key {
                Value::String(key) => key
                    .to_str()
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?
                    .to_owned(),
                other => other.to_string()?,
            };
            let value = match value {
                Value::String(value) => value
                    .to_str()
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?
                    .to_owned(),
                Value::Nil => String::new(),
                Value::Boolean(value) => value.to_string(),
                Value::Integer(value) => value.to_string(),
                Value::Number(value) => value.to_string(),
                other => other.to_string()?,
            };
            headers.insert(
                key.parse::<HeaderName>()
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?,
                HeaderValue::from_str(&value)
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?,
            );
        }
    }
    Ok(headers)
}

fn apply_params(mut url: Url, params: Option<Table>) -> Result<Url> {
    if let Some(params) = params {
        let mut qp = url.query_pairs_mut();
        for pair in params.pairs::<Value, Value>() {
            let (key, value) = pair?;
            let key = match key {
                Value::String(key) => key
                    .to_str()
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?
                    .to_owned(),
                other => other.to_string()?,
            };
            let value = match value {
                Value::String(value) => value
                    .to_str()
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?
                    .to_owned(),
                Value::Nil => String::new(),
                Value::Boolean(value) => value.to_string(),
                Value::Integer(value) => value.to_string(),
                Value::Number(value) => value.to_string(),
                other => other.to_string()?,
            };
            qp.append_pair(&key, &value);
        }
    }
    Ok(url)
}

fn body_to_bytes(lua: &Lua, body: Option<Value>) -> Result<Option<Vec<u8>>> {
    match body {
        None | Some(Value::Nil) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_bytes().to_vec())),
        Some(Value::Table(table)) => Ok(Some(util::json_encode(lua, Value::Table(table))?.into_bytes())),
        Some(value) => Ok(Some(util::json_encode(lua, value)?.into_bytes())),
    }
}

fn response_to_table(lua: &Lua, response: Response) -> Result<mlua::Value> {
    let status = response.status().as_u16() as i64;
    let headers = lua.create_table()?;
    for (key, value) in response.headers().iter() {
        headers.set(
            key.as_str(),
            value
                .to_str()
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?
                .to_owned(),
        )?;
    }
    let body = response
        .text()
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
    let table = lua.create_table()?;
    table.set("status", status)?;
    table.set("headers", headers)?;
    table.set("body", body)?;
    Ok(mlua::Value::Table(table))
}

fn request(
    lua: &Lua,
    method: String,
    url: String,
    headers: Option<Table>,
    params: Option<Table>,
    body: Option<Value>,
) -> Result<reqwest::blocking::RequestBuilder> {
    let client = Client::new();
    let url = apply_params(url.parse::<Url>().map_err(|err| mlua::Error::RuntimeError(err.to_string()))?, params)?;
    let headers = parse_headers(headers)?;
    let body = body_to_bytes(lua, body)?;
    let method = Method::from_bytes(method.as_bytes())
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

    let mut request = client.request(method, url).headers(headers);
    if let Some(body) = body {
        request = request.body(body);
    }
    Ok(request)
}

pub fn http(
    lua: &Lua,
    method: String,
    url: String,
    headers: Option<Table>,
    params: Option<Table>,
    body: Option<Value>,
) -> Result<mlua::Value> {
    let response = request(lua, method, url, headers, params, body)?
        .send()
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
    response_to_table(lua, response)
}

pub fn http_stream(
    lua: &Lua,
    method: String,
    url: String,
    headers: Option<Table>,
    params: Option<Table>,
    body: Option<Value>,
    on_line: Function,
) -> Result<mlua::Value> {
    let response = request(lua, method, url, headers, params, body)?
        .send()
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

    let status = response.status().as_u16() as i64;
    let response_headers = lua.create_table()?;
    for (key, value) in response.headers().iter() {
        response_headers.set(
            key.as_str(),
            value
                .to_str()
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?
                .to_owned(),
        )?;
    }

    let mut reader = BufReader::new(response);
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        if read == 0 {
            break;
        }
        let line = line.trim_end_matches(&['\r', '\n'][..]).to_string();
        let keep_going: Option<bool> = on_line.call(line)?;
        if matches!(keep_going, Some(false)) {
            break;
        }
    }

    let table = lua.create_table()?;
    table.set("status", status)?;
    table.set("headers", response_headers)?;
    Ok(mlua::Value::Table(table))
}
