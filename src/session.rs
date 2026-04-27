use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use mlua::{Function, Lua, Result, Table, UserData, UserDataMethods, Value};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};

use crate::runtime;
use crate::tools;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_SESSION_NAME: AtomicU64 = AtomicU64::new(1);

fn generate_session_name() -> String {
    let seq = NEXT_SESSION_NAME.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("session-{nanos:x}-{seq:x}")
}

fn normalize_session_name(name: Option<String>) -> String {
    match name {
        Some(name) if !name.trim().is_empty() => name,
        _ => generate_session_name(),
    }
}

fn sqlite_url(path: &Path) -> Result<SqliteConnectOptions> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    Ok(options)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

struct SessionInner {
    id: u64,
    name: Option<String>,
    history: Vec<Message>,
    context: Table,
    model: Option<Function>,
    interface: Option<Function>,
    tools: HashMap<String, ToolEntry>,
    context_hooks: Vec<Function>,
    action_hooks: Vec<Function>,
}

struct ToolEntry {
    name: String,
    description: Option<String>,
    parameters: Table,
    func: Function,
}

#[derive(Clone)]
pub struct Session {
    inner: Rc<RefCell<SessionInner>>,
}

impl Session {
    pub fn new(lua: &Lua, name: Option<String>) -> Result<Self> {
        let name = normalize_session_name(name);
        let mut session = Session {
            inner: Rc::new(RefCell::new(SessionInner {
                id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
                name: Some(name.clone()),
                history: Vec::new(),
                context: lua.create_table()?,
                model: None,
                interface: None,
                tools: HashMap::new(),
                context_hooks: Vec::new(),
                action_hooks: Vec::new(),
            })),
        };

        session.load_persisted_history(lua, &name)?;
        session.install_default_tools(lua)?;
        Ok(session)
    }

    fn install_default_tools(&mut self, lua: &Lua) -> Result<()> {
        self.add_tool_spec(
            lua,
            "read_file",
            Some("Read a file from disk".to_string()),
            tool_schema(lua, &[("path", "string")])?,
            lua.create_function(|lua, args: Table| {
                let path = required_tool_string(&args, "read_file", "path")?;
                tools::read_file(lua, path)
            })?,
        )?;
        self.add_tool_spec(
            lua,
            "write_file",
            Some("Write a file to disk".to_string()),
            tool_schema(lua, &[("path", "string"), ("content", "string")])?,
            lua.create_function(|lua, args: Table| {
                let path = required_tool_string(&args, "write_file", "path")?;
                let content = required_tool_string(&args, "write_file", "content")?;
                tools::write_file(lua, path, content)
            })?,
        )?;
        self.add_tool_spec(
            lua,
            "bash",
            Some("Run a shell command".to_string()),
            tool_schema(lua, &[("command", "string")])?,
            lua.create_function(|lua, args: Table| {
                let command = required_tool_string(&args, "bash", "command")?;
                tools::bash(lua, command)
            })?,
        )?;
        Ok(())
    }

    pub fn set_session_db(lua: &Lua, path: String) -> Result<()> {
        let path = PathBuf::from(path);
        if path.as_os_str().is_empty() {
            return Err(mlua::Error::RuntimeError(
                "session database path cannot be empty".into(),
            ));
        }

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
            }
        }

        runtime::block_on(lua, Self::ensure_session_schema(&path))?;
        runtime::set_session_db_path(lua, path);

        Ok(())
    }

    async fn open_session_pool(path: &Path) -> Result<SqlitePool> {
        let options = sqlite_url(path)?;
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))
    }

    async fn ensure_session_schema(path: &Path) -> Result<()> {
        let pool = Self::open_session_pool(path).await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                name TEXT PRIMARY KEY NOT NULL,
                history_json TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        Ok(())
    }

    async fn load_history(path: &Path, name: &str) -> Result<Option<Vec<Message>>> {
        let pool = Self::open_session_pool(path).await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                name TEXT PRIMARY KEY NOT NULL,
                history_json TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

        let row = sqlx::query(
            r#"
            SELECT history_json
            FROM sessions
            WHERE name = ?1
            "#,
        )
        .bind(name)
        .fetch_optional(&pool)
        .await
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let history_json: String = row
            .try_get("history_json")
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        let history: Vec<Message> = serde_json::from_str(&history_json)
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        Ok(Some(history))
    }

    async fn save_history(path: &Path, name: &str, history: &[Message]) -> Result<()> {
        let pool = Self::open_session_pool(path).await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                name TEXT PRIMARY KEY NOT NULL,
                history_json TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

        let history_json = serde_json::to_string(history)
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO sessions (name, history_json)
            VALUES (?1, ?2)
            ON CONFLICT(name) DO UPDATE SET
                history_json = excluded.history_json
            "#,
        )
        .bind(name)
        .bind(history_json)
        .execute(&pool)
        .await
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        Ok(())
    }

    fn load_persisted_history(&self, lua: &Lua, name: &str) -> Result<()> {
        let Some(path) = runtime::session_db_path(lua) else {
            return Ok(());
        };

        let history = runtime::block_on(lua, async move { Self::load_history(&path, name).await })?;
        if let Some(history) = history {
            self.inner.borrow_mut().history = history;
        }
        Ok(())
    }

    fn persist_history(&self, lua: &Lua) -> Result<()> {
        let Some(path) = runtime::session_db_path(lua) else {
            return Ok(());
        };

        let inner = self.inner.borrow();
        let name = inner
            .name
            .as_ref()
            .ok_or_else(|| mlua::Error::RuntimeError("session name is missing".into()))?
            .clone();
        let history = inner.history.clone();
        drop(inner);

        runtime::block_on(lua, async move {
            Self::save_history(&path, &name, &history).await
        })
    }

    fn add_tool_spec(
        &mut self,
        _lua: &Lua,
        name: impl Into<String>,
        description: Option<String>,
        parameters: Table,
        func: Function,
    ) -> Result<()> {
        let name = name.into();
        self.register_tool(name, description, parameters, func);
        Ok(())
    }

    fn register_tool(
        &self,
        name: String,
        description: Option<String>,
        parameters: Table,
        func: Function,
    ) {
        self.inner.borrow_mut().tools.insert(
            name.clone(),
            ToolEntry {
                name,
                description,
                parameters,
                func,
            },
        );
    }

    fn build_history_table(&self, lua: &Lua) -> Result<Table> {
        let history = self.inner.borrow();
        let table = lua.create_table()?;
        for (idx, message) in history.history.iter().enumerate() {
            let item = lua.create_table()?;
            item.set("role", message.role.as_str())?;
            item.set("content", message.content.as_str())?;
            table.set(idx + 1, item)?;
        }
        Ok(table)
    }

    fn build_context_table(&self, lua: &Lua) -> Result<Table> {
        let _ = lua;
        Ok(self.inner.borrow().context.clone())
    }

    fn build_payload(&self, lua: &Lua) -> Result<Table> {
        let inner = self.inner.borrow();
        let payload = lua.create_table()?;
        payload.set("session_id", inner.id)?;
        if let Some(name) = &inner.name {
            payload.set("name", name.as_str())?;
        }
        payload.set("history", self.build_history_table(lua)?)?;
        payload.set("context", self.build_context_table(lua)?)?;

        payload.set("tools", self.build_tool_specs_table(lua)?)?;
        Ok(payload)
    }

    fn build_tool_specs_table(&self, lua: &Lua) -> Result<Table> {
        let inner = self.inner.borrow();
        let table = lua.create_table()?;
        let mut tools: Vec<&ToolEntry> = inner.tools.values().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        for (idx, tool) in tools.into_iter().enumerate() {
            let spec = lua.create_table()?;
            spec.set("type", "function")?;
            let function = lua.create_table()?;
            function.set("name", tool.name.as_str())?;
            if let Some(description) = &tool.description {
                function.set("description", description.as_str())?;
            }
            function.set("parameters", tool.parameters.clone())?;
            spec.set("function", function)?;
            table.set(idx + 1, spec)?;
        }

        Ok(table)
    }

    fn push_history(&self, role: impl Into<String>, content: impl Into<String>) {
        self.inner.borrow_mut().history.push(Message {
            role: role.into(),
            content: content.into(),
        });
    }

    fn run_context_hooks(&self, lua: &Lua) -> Result<()> {
        let hooks = {
            let inner = self.inner.borrow();
            inner.context_hooks.clone()
        };
        let payload = self.build_payload(lua)?;
        for hook in hooks {
            hook.call::<()>(payload.clone())?;
        }
        Ok(())
    }

    fn run_action_hooks(&self, lua: &Lua, action: Table, result: Value) -> Result<()> {
        let hooks = {
            let inner = self.inner.borrow();
            inner.action_hooks.clone()
        };
        let payload = self.build_payload(lua)?;
        let event = lua.create_table()?;
        event.set("action", action)?;
        event.set("result", result)?;
        event.set("payload", payload)?;
        for hook in hooks {
            hook.call::<()>(event.clone())?;
        }
        Ok(())
    }

    fn call_model(&self, lua: &Lua) -> Result<Value> {
        let model = {
            let inner = self.inner.borrow();
            inner.model.clone()
        }
        .ok_or_else(|| mlua::Error::RuntimeError("session model provider is not set".into()))?;
        let provider = model;
        let payload = self.build_payload(lua)?;
        provider.call(payload)
    }

    fn invoke_tool(&self, _lua: &Lua, name: &str, args: Value) -> Result<Value> {
        let tool = {
            let inner = self.inner.borrow();
            inner.tools.get(name).map(|entry| entry.func.clone())
        }
        .ok_or_else(|| mlua::Error::RuntimeError(format!("tool `{name}` is not registered")))?;
        tool.call(args)
    }

    fn call_tool_with_hooks(&self, lua: &Lua, name: &str, args: Value) -> Result<Value> {
        let result = self.invoke_tool(lua, name, args)?;
        let action = lua.create_table()?;
        action.set("kind", "tool")?;
        action.set("name", name)?;
        self.run_action_hooks(lua, action, result.clone())?;
        Ok(result)
    }

    fn interpret_model_output(&self, lua: &Lua, value: Value) -> Result<StepResult> {
        match value {
            Value::String(s) => {
                let content = s.to_str()?.to_owned();
                self.push_history("assistant", content.clone());
                self.persist_history(lua)?;
                Ok(StepResult::Final(content))
            }
            Value::Table(table) => {
                let kind: Option<String> = table.get("kind").ok();
                match kind.as_deref() {
                    Some("final") => {
                        let content: String = table.get("content")?;
                        self.push_history("assistant", content.clone());
                        self.persist_history(lua)?;
                        Ok(StepResult::Final(content))
                    }
                    Some("tool") => {
                        let name: String = table.get("name")?;
                        let args: Value = table.get("args").unwrap_or(Value::Nil);
                        let result = self.call_tool_with_hooks(lua, &name, args)?;
                        let result_text = value_to_text(lua, &result)?;
                        self.push_history(format!("tool:{name}"), result_text.clone());
                        self.persist_history(lua)?;
                        Ok(StepResult::Tool {
                            name,
                            result: result_text,
                        })
                    }
                    _ => {
                        let content = value_to_text(lua, &Value::Table(table))?;
                        self.push_history("assistant", content.clone());
                        self.persist_history(lua)?;
                        Ok(StepResult::Final(content))
                    }
                }
            }
            other => {
                let content = value_to_text(lua, &other)?;
                self.push_history("assistant", content.clone());
                self.persist_history(lua)?;
                Ok(StepResult::Final(content))
            }
        }
    }

    fn tool_specs(&self, lua: &Lua) -> Result<Table> {
        self.build_tool_specs_table(lua)
    }

    fn add_tool_from_lua(&self, lua: &Lua, spec: ToolSpecInput, func: Function) -> Result<()> {
        let _ = lua;
        self.register_tool(spec.name, spec.description, spec.parameters, func);
        Ok(())
    }

    fn tool_name_from_value(&self, value: Value) -> Result<String> {
        match value {
            Value::String(name) => Ok(name.to_str()?.to_owned()),
            Value::Integer(name) => Ok(name.to_string()),
            Value::Number(name) => Ok(name.to_string()),
            other => Err(mlua::Error::RuntimeError(format!(
                "tool call name must be a string, got {other:?}"
            ))),
        }
    }
}

struct ToolSpecInput {
    name: String,
    description: Option<String>,
    parameters: Table,
}

impl ToolSpecInput {
    fn from_table(lua: &Lua, table: Table) -> Result<Self> {
        let name: String = table
            .get("name")
            .map_err(|_| mlua::Error::RuntimeError("tool spec missing `name`".into()))?;
        let description: Option<String> = table.get("description").ok();
        let parameters = match table.get::<Option<Table>>("parameters").ok().flatten() {
            Some(parameters) => parameters,
            None => tool_schema(lua, &[])?,
        };
        Ok(Self {
            name,
            description,
            parameters,
        })
    }
}

fn tool_schema(lua: &Lua, fields: &[(&str, &str)]) -> Result<Table> {
    let schema = lua.create_table()?;
    schema.set("type", "object")?;
    let properties = lua.create_table()?;
    let required = lua.create_table()?;

    for (idx, (name, ty)) in fields.iter().enumerate() {
        let field = lua.create_table()?;
        field.set("type", *ty)?;
        properties.set(*name, field)?;
        required.set(idx + 1, *name)?;
    }

    schema.set("properties", properties)?;
    schema.set("required", required)?;
    schema.set("additionalProperties", false)?;
    Ok(schema)
}

fn required_tool_string(args: &Table, tool: &str, field: &str) -> Result<String> {
    args.get::<Option<String>>(field)?.ok_or_else(|| {
        mlua::Error::RuntimeError(format!(
            "tool `{tool}` is missing required string argument `{field}`"
        ))
    })
}

#[derive(Debug, Clone)]
pub enum StepResult {
    Final(String),
    Tool { name: String, result: String },
}

fn value_to_text(lua: &Lua, value: &Value) -> Result<String> {
    Ok(match value {
        Value::Nil => String::new(),
        Value::Boolean(v) => v.to_string(),
        Value::Integer(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.to_str()?.to_owned(),
        Value::Table(_) => {
            let tostring: Function = lua.globals().get("tostring")?;
            tostring.call(value.clone())?
        }
        _ => {
            let tostring: Function = lua.globals().get("tostring")?;
            tostring.call(value.clone())?
        }
    })
}

impl UserData for Session {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("id", |_, this, ()| Ok(this.inner.borrow().id));
        methods.add_method("name", |_, this, ()| Ok(this.inner.borrow().name.clone()));

        methods.add_method("history", |lua, this, ()| this.build_history_table(lua));
        methods.add_method("context", |lua, this, ()| this.build_context_table(lua));
        methods.add_method("tools", |lua, this, ()| {
            let inner = this.inner.borrow();
            let table = lua.create_table()?;
            for key in inner.tools.keys() {
                table.set(key.as_str(), true)?;
            }
            Ok(table)
        });
        methods.add_method("tool_specs", |lua, this, ()| this.tool_specs(lua));

        methods.add_method_mut("set_model", |_lua, this, func: Function| {
            this.inner.borrow_mut().model = Some(func);
            Ok(())
        });

        methods.add_method_mut("set_interface", |_lua, this, func: Function| {
            this.inner.borrow_mut().interface = Some(func);
            Ok(())
        });

        methods.add_method("run_interface", |lua, this, ()| {
            let interface = {
                let inner = this.inner.borrow();
                inner.interface.clone()
            }
            .ok_or_else(|| mlua::Error::RuntimeError("session interface is not set".into()))?;
            let payload = this.build_payload(lua)?;
            interface.call::<()>(payload)
        });

        methods.add_method_mut("add_tool", |lua, this, (spec, func): (Value, Function)| {
            match spec {
                Value::String(name) => {
                    let name = name.to_str()?.to_owned();
                    let parameters = tool_schema(lua, &[])?;
                    this.register_tool(name, None, parameters, func);
                }
                Value::Table(table) => {
                    let spec = ToolSpecInput::from_table(lua, table)?;
                    this.add_tool_from_lua(lua, spec, func)?;
                }
                other => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "add_tool expects a string or table spec, got {other:?}"
                    )));
                }
            }
            Ok(())
        });

        methods.add_method(
            "call_tool",
            |lua, this, (name, args): (Value, Option<Value>)| {
                let name = this.tool_name_from_value(name)?;
                this.call_tool_with_hooks(lua, &name, args.unwrap_or(Value::Nil))
            },
        );

        methods.add_method_mut("remove_tool", |_, this, name: String| {
            this.inner.borrow_mut().tools.remove(&name);
            Ok(())
        });

        methods.add_method_mut("add_context_hook", |_lua, this, func: Function| {
            this.inner.borrow_mut().context_hooks.push(func);
            Ok(())
        });

        methods.add_method_mut("add_action_hook", |_lua, this, func: Function| {
            this.inner.borrow_mut().action_hooks.push(func);
            Ok(())
        });

        methods.add_method("push", |lua, this, (role, content): (String, String)| {
            this.push_history(role, content);
            this.persist_history(lua)?;
            Ok(())
        });

        methods.add_method("step", |lua, this, ()| this.step_once(lua));

        methods.add_method(
            "run",
            |lua, this, max_steps: Option<u32>| -> Result<String> {
                let mut steps = 0u32;
                loop {
                    if let Some(limit) = max_steps {
                        if steps >= limit {
                            return Err(mlua::Error::RuntimeError(
                                "session did not finish within the configured step limit".into(),
                            ));
                        }
                    }

                    let step = this.step_once(lua)?;
                    let kind: Option<String> = step.get("kind").ok();
                    if kind.as_deref() == Some("final") {
                        return step.get("content");
                    }
                    steps += 1;
                }
            },
        );
    }
}

impl Session {
    fn step_once(&self, lua: &Lua) -> Result<Table> {
        self.run_context_hooks(lua)?;
        let output = self.call_model(lua)?;
        let result = self.interpret_model_output(lua, output)?;
        let table = lua.create_table()?;
        match result {
            StepResult::Final(content) => {
                table.set("kind", "final")?;
                table.set("content", content)?;
            }
            StepResult::Tool { name, result } => {
                table.set("kind", "tool")?;
                table.set("name", name)?;
                table.set("result", result)?;
            }
        }
        Ok(table)
    }
}
