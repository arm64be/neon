use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use mlua::{Function, Lua, Result, Table, UserData, UserDataMethods, Value};

use crate::tools;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
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
    tools: HashMap<String, Function>,
    context_hooks: Vec<Function>,
    action_hooks: Vec<Function>,
}

#[derive(Clone)]
pub struct Session {
    inner: Rc<RefCell<SessionInner>>,
}

impl Session {
    pub fn new(lua: &Lua, name: Option<String>) -> Result<Self> {
        let mut session = Session {
            inner: Rc::new(RefCell::new(SessionInner {
                id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
                name,
                history: Vec::new(),
                context: lua.create_table()?,
                model: None,
                interface: None,
                tools: HashMap::new(),
                context_hooks: Vec::new(),
                action_hooks: Vec::new(),
            })),
        };

        session.install_default_tools(lua)?;
        Ok(session)
    }

    fn install_default_tools(&mut self, lua: &Lua) -> Result<()> {
        self.add_tool_value(
            lua,
            "read_file",
            lua.create_function(|_, args: Table| {
                let path: String = args.get("path")?;
                tools::read_file(path)
            })?,
        )?;
        self.add_tool_value(
            lua,
            "write_file",
            lua.create_function(|_, args: Table| {
                let path: String = args.get("path")?;
                let content: String = args.get("content")?;
                tools::write_file(path, content)
            })?,
        )?;
        self.add_tool_value(
            lua,
            "bash",
            lua.create_function(|_, args: Table| {
                let command: String = args.get("command")?;
                tools::bash(command)
            })?,
        )?;
        Ok(())
    }

    fn add_tool_value(&mut self, _lua: &Lua, name: impl Into<String>, func: Function) -> Result<()> {
        self.inner.borrow_mut().tools.insert(name.into(), func);
        Ok(())
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

        let tools = lua.create_table()?;
        for key in inner.tools.keys() {
            tools.set(key.as_str(), true)?;
        }
        payload.set("tools", tools)?;
        Ok(payload)
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

    fn call_tool(&self, lua: &Lua, name: &str, args: Value) -> Result<Value> {
        let tool = {
            let inner = self.inner.borrow();
            inner.tools.get(name).cloned()
        }
        .ok_or_else(|| mlua::Error::RuntimeError(format!("tool `{name}` is not registered")))?;
        let _ = lua;
        tool.call(args)
    }

    fn interpret_model_output(&self, lua: &Lua, value: Value) -> Result<StepResult> {
        match value {
            Value::String(s) => {
                let content = s.to_str()?.to_owned();
                self.push_history("assistant", content.clone());
                Ok(StepResult::Final(content))
            }
            Value::Table(table) => {
                let kind: Option<String> = table.get("kind").ok();
                match kind.as_deref() {
                    Some("final") => {
                        let content: String = table.get("content")?;
                        self.push_history("assistant", content.clone());
                        Ok(StepResult::Final(content))
                    }
                    Some("tool") => {
                        let name: String = table.get("name")?;
                        let args: Value = table.get("args").unwrap_or(Value::Nil);
                        let result = self.call_tool(lua, &name, args)?;
                        let result_text = value_to_text(lua, &result)?;
                        self.push_history(format!("tool:{name}"), result_text.clone());
                        let action = lua.create_table()?;
                        action.set("kind", "tool")?;
                        action.set("name", name.as_str())?;
                        self.run_action_hooks(lua, action, result)?;
                        Ok(StepResult::Tool {
                            name,
                            result: result_text,
                        })
                    }
                    _ => {
                        let content = value_to_text(lua, &Value::Table(table))?;
                        self.push_history("assistant", content.clone());
                        Ok(StepResult::Final(content))
                    }
                }
            }
            other => {
                let content = value_to_text(lua, &other)?;
                self.push_history("assistant", content.clone());
                Ok(StepResult::Final(content))
            }
        }
    }
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

        methods.add_method_mut("add_tool", |_lua, this, (name, func): (String, Function)| {
            this.inner.borrow_mut().tools.insert(name, func);
            Ok(())
        });

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

        methods.add_method("push", |_, this, (role, content): (String, String)| {
            this.push_history(role, content);
            Ok(())
        });

        methods.add_method("step", |lua, this, ()| this.step_once(lua));

        methods.add_method("run", |lua, this, max_steps: Option<u32>| -> Result<String> {
            let max_steps = max_steps.unwrap_or(32);
            for _ in 0..max_steps {
                let step = this.step_once(lua)?;
                let kind: Option<String> = step.get("kind").ok();
                if kind.as_deref() == Some("final") {
                    return step.get("content");
                }
            }
            Err(mlua::Error::RuntimeError(
                "session did not finish within the configured step limit".into(),
            ))
        });
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
