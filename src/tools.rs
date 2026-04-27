use std::path::Path;
use std::process::Command;

pub fn read_file(path: String) -> mlua::Result<String> {
    std::fs::read_to_string(path).map_err(|err| mlua::Error::RuntimeError(err.to_string()))
}

pub fn write_file(path: String, content: String) -> mlua::Result<String> {
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
        }
    }
    std::fs::write(&path, content).map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
    Ok(format!("wrote {path}"))
}

pub fn bash(command: String) -> mlua::Result<String> {
    let output = Command::new("bash")
        .arg("-lc")
        .arg(command)
        .output()
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    Ok(format!(
        "exit={}\nstdout:\n{}\nstderr:\n{}",
        output.status.code().unwrap_or(-1),
        stdout.trim_end(),
        stderr.trim_end()
    ))
}
