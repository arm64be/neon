use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{Lua, Result, Table, UserData, UserDataMethods, Value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Column, Row, SqlitePool,
};

use crate::runtime;

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct SqliteConnection {
    id: String,
}

impl SqliteConnection {
    pub fn from_id(id: String) -> Self {
        Self { id }
    }

    pub fn connect(lua: &Lua, path: String) -> Result<Self> {
        if path.trim().is_empty() {
            return Err(mlua::Error::RuntimeError(
                "sqlite path cannot be empty".into(),
            ));
        }
        let path_buf = PathBuf::from(path);
        if let Some(parent) = path_buf.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
            }
        }
        let options = SqliteConnectOptions::new()
            .filename(path_buf)
            .create_if_missing(true);
        Self::open_with_options(lua, options)
    }

    pub fn memory(lua: &Lua) -> Result<Self> {
        let options = SqliteConnectOptions::new().in_memory(true);
        Self::open_with_options(lua, options)
    }

    fn open_with_options(lua: &Lua, options: SqliteConnectOptions) -> Result<Self> {
        let id = format!(
            "sqlite-{:x}",
            NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
        );
        let pool = runtime::block_on(lua, async move {
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))
        })?;
        runtime::register_sqlite_connection(lua, id.clone(), pool);
        Ok(Self { id })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn resolve(lua: &Lua, value: Value) -> Result<Self> {
        match value {
            Value::UserData(ud) => {
                if let Ok(conn) = ud.borrow::<SqliteConnection>() {
                    return Ok(conn.clone());
                }
                Err(mlua::Error::RuntimeError(
                    "expected sqlite connection userdata".into(),
                ))
            }
            Value::String(s) => {
                let id = s.to_str()?.to_owned();
                if runtime::sqlite_connection(lua, &id).is_none() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "sqlite connection `{id}` is not registered"
                    )));
                }
                Ok(Self { id })
            }
            other => Err(mlua::Error::RuntimeError(format!(
                "expected sqlite connection or id string, got {other:?}"
            ))),
        }
    }

    fn pool(&self, lua: &Lua) -> Result<SqlitePool> {
        runtime::sqlite_connection(lua, &self.id).ok_or_else(|| {
            mlua::Error::RuntimeError(format!("sqlite connection `{}` is not registered", self.id))
        })
    }

    fn exec(&self, lua: &Lua, sql: String, params: Option<Table>) -> Result<u64> {
        let pool = self.pool(lua)?;
        let values = extract_params(params)?;
        runtime::block_on(lua, async move {
            let mut query = sqlx::query(&sql);
            for value in values {
                query = bind_value(query, value);
            }
            query
                .execute(&pool)
                .await
                .map(|result| result.rows_affected())
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))
        })
    }

    fn query_all(&self, lua: &Lua, sql: String, params: Option<Table>) -> Result<Table> {
        let pool = self.pool(lua)?;
        let values = extract_params(params)?;
        runtime::block_on(lua, async move {
            let mut query = sqlx::query(&sql);
            for value in values {
                query = bind_value(query, value);
            }
            let rows = query
                .fetch_all(&pool)
                .await
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

            lua.create_sequence_from(
                rows.into_iter()
                    .map(|row| sqlite_row_to_lua(lua, &row))
                    .collect::<Result<Vec<_>>>()?,
            )
        })
    }

    fn query_one(&self, lua: &Lua, sql: String, params: Option<Table>) -> Result<Value> {
        let pool = self.pool(lua)?;
        let values = extract_params(params)?;
        runtime::block_on(lua, async move {
            let mut query = sqlx::query(&sql);
            for value in values {
                query = bind_value(query, value);
            }
            let row = query
                .fetch_optional(&pool)
                .await
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
            match row {
                Some(row) => Ok(Value::Table(sqlite_row_to_lua(lua, &row)?)),
                None => Ok(Value::Nil),
            }
        })
    }

    fn schema(&self, lua: &Lua, definition: Table) -> Result<Table> {
        let pool = self.pool(lua)?;
        let mut models = HashMap::new();
        for pair in definition.pairs::<String, Table>() {
            let (table_name, columns) = pair?;
            let ddl = build_create_table_sql(&table_name, columns)?;
            let pool = pool.clone();
            runtime::block_on(lua, async move {
                sqlx::query(&ddl)
                    .execute(&pool)
                    .await
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))
            })?;
            models.insert(
                table_name.clone(),
                SqliteModel {
                    connection_id: self.id.clone(),
                    table_name,
                },
            );
        }

        let out = lua.create_table()?;
        for (name, model) in models {
            out.set(name, model)?;
        }
        Ok(out)
    }
}

impl UserData for SqliteConnection {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("id", |_, this, ()| Ok(this.id.clone()));
        methods.add_method(
            "exec",
            |lua, this, (sql, params): (String, Option<Table>)| this.exec(lua, sql, params),
        );
        methods.add_method(
            "query",
            |lua, this, (sql, params): (String, Option<Table>)| this.query_all(lua, sql, params),
        );
        methods.add_method(
            "one",
            |lua, this, (sql, params): (String, Option<Table>)| this.query_one(lua, sql, params),
        );
        methods.add_method("schema", |lua, this, definition: Table| {
            this.schema(lua, definition)
        });
    }
}

#[derive(Clone)]
pub struct SqliteModel {
    connection_id: String,
    table_name: String,
}

impl SqliteModel {
    fn connection(&self) -> SqliteConnection {
        SqliteConnection {
            id: self.connection_id.clone(),
        }
    }

    fn insert(&self, lua: &Lua, row: Table) -> Result<u64> {
        let mut keys = Vec::new();
        let mut placeholders = Vec::new();
        let mut values = Vec::new();
        for pair in row.pairs::<String, Value>() {
            let (key, value) = pair?;
            keys.push(key);
            placeholders.push("?");
            values.push(value);
        }
        if keys.is_empty() {
            return Err(mlua::Error::RuntimeError(
                "insert requires at least one column".into(),
            ));
        }
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_ident(&self.table_name),
            keys.iter()
                .map(|k| quote_ident(k))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );
        let conn = self.connection();
        let params = lua.create_sequence_from(values)?;
        conn.exec(lua, sql, Some(params))
    }

    fn all(&self, lua: &Lua) -> Result<Table> {
        let sql = format!("SELECT * FROM {}", quote_ident(&self.table_name));
        self.connection().query_all(lua, sql, None)
    }

    fn where_clause(
        &self,
        lua: &Lua,
        (where_sql, params): (String, Option<Table>),
    ) -> Result<Table> {
        let sql = format!(
            "SELECT * FROM {} WHERE {}",
            quote_ident(&self.table_name),
            where_sql
        );
        self.connection().query_all(lua, sql, params)
    }
}

impl UserData for SqliteModel {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("insert", |lua, this, row: Table| this.insert(lua, row));
        methods.add_method("all", |lua, this, ()| this.all(lua));
        methods.add_method(
            "where",
            |lua, this, (where_sql, params): (String, Option<Table>)| {
                this.where_clause(lua, (where_sql, params))
            },
        );
    }
}

fn build_create_table_sql(table_name: &str, columns: Table) -> Result<String> {
    let mut clauses = Vec::new();
    for pair in columns.pairs::<String, Value>() {
        let (name, value) = pair?;
        let definition = match value {
            Value::String(s) => s.to_str()?.to_owned(),
            Value::Table(t) => {
                let ty: String = t.get("type").map_err(|_| {
                    mlua::Error::RuntimeError("schema column missing `type`".into())
                })?;
                let nullable = t
                    .get::<Option<bool>>("nullable")
                    .ok()
                    .flatten()
                    .unwrap_or(true);
                let primary_key = t
                    .get::<Option<bool>>("primary_key")
                    .ok()
                    .flatten()
                    .unwrap_or(false);
                let unique = t
                    .get::<Option<bool>>("unique")
                    .ok()
                    .flatten()
                    .unwrap_or(false);
                let default = t.get::<Option<String>>("default").ok().flatten();
                let mut parts = vec![ty];
                if !nullable {
                    parts.push("NOT NULL".to_string());
                }
                if primary_key {
                    parts.push("PRIMARY KEY".to_string());
                }
                if unique {
                    parts.push("UNIQUE".to_string());
                }
                if let Some(default) = default {
                    parts.push(format!("DEFAULT {default}"));
                }
                parts.join(" ")
            }
            other => {
                return Err(mlua::Error::RuntimeError(format!(
                    "invalid schema column definition for `{name}`: {other:?}"
                )))
            }
        };
        clauses.push(format!("{} {}", quote_ident(&name), definition));
    }
    if clauses.is_empty() {
        return Err(mlua::Error::RuntimeError(format!(
            "schema for table `{table_name}` cannot be empty"
        )));
    }
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {} ({})",
        quote_ident(table_name),
        clauses.join(", ")
    ))
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn sqlite_row_to_lua(lua: &Lua, row: &sqlx::sqlite::SqliteRow) -> Result<Table> {
    let table = lua.create_table()?;
    for col in row.columns() {
        let name = col.name();
        let value = if let Ok(v) = row.try_get::<Option<String>, _>(name) {
            match v {
                Some(v) => Value::String(lua.create_string(&v)?),
                None => Value::Nil,
            }
        } else if let Ok(v) = row.try_get::<Option<i64>, _>(name) {
            match v {
                Some(v) => Value::Integer(v),
                None => Value::Nil,
            }
        } else if let Ok(v) = row.try_get::<Option<f64>, _>(name) {
            match v {
                Some(v) => Value::Number(v),
                None => Value::Nil,
            }
        } else if let Ok(v) = row.try_get::<Option<bool>, _>(name) {
            match v {
                Some(v) => Value::Boolean(v),
                None => Value::Nil,
            }
        } else {
            Value::Nil
        };
        table.set(name, value)?;
    }
    Ok(table)
}

fn extract_params(params: Option<Table>) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    if let Some(params) = params {
        for value in params.sequence_values::<Value>() {
            out.push(value?);
        }
    }
    Ok(out)
}

fn bind_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    value: Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match value {
        Value::Nil => query.bind(Option::<String>::None),
        Value::Boolean(v) => query.bind(v),
        Value::Integer(v) => query.bind(v),
        Value::Number(v) => query.bind(v),
        Value::String(v) => match v.to_str() {
            Ok(s) => query.bind(s.to_owned()),
            Err(_) => query.bind(v.as_bytes().to_vec()),
        },
        other => query.bind(format!("{other:?}")),
    }
}

pub fn create_module(lua: &Lua) -> Result<Table> {
    let module = lua.create_table()?;
    module.set(
        "connect",
        lua.create_function(|lua, path: String| SqliteConnection::connect(lua, path))?,
    )?;
    module.set(
        "memory",
        lua.create_function(|lua, ()| SqliteConnection::memory(lua))?,
    )?;
    module.set(
        "schema",
        lua.create_function(|lua, (conn, definition): (Value, Table)| {
            let conn = SqliteConnection::resolve(lua, conn)?;
            conn.schema(lua, definition)
        })?,
    )?;
    Ok(module)
}
