use std::path::Path;

use tokio::{fs, process::Command as TokioCommand};

use crate::runtime;

use mlua::Lua;

pub fn read_file(lua: &Lua, path: String) -> mlua::Result<String> {
    runtime::block_on(lua, async move {
        fs::read_to_string(path)
            .await
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))
    })
}

pub fn write_file(lua: &Lua, path: String, content: String) -> mlua::Result<String> {
    runtime::block_on(lua, async move {
        if let Some(parent) = Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
            }
        }
        fs::write(&path, content)
            .await
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        Ok(format!("wrote {path}"))
    })
}

pub fn bash(lua: &Lua, command: String) -> mlua::Result<String> {
    runtime::block_on(lua, async move {
        let output = TokioCommand::new("bash")
            .arg("-lc")
            .arg(command)
            .output()
            .await
            .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!(
            "exit={}\nstdout:\n{}\nstderr:\n{}",
            output.status.code().unwrap_or(-1),
            stdout.trim_end(),
            stderr.trim_end()
        ))
    })
}
