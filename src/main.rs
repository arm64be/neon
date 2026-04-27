use std::{env, fs, path::PathBuf};

use neon::Neon;

fn main() -> mlua::Result<()> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let (script, config_args) = match raw_args.first() {
        Some(first) if !first.starts_with('-') && PathBuf::from(first).exists() => {
            (PathBuf::from(first), raw_args[1..].to_vec())
        }
        _ => (PathBuf::from("config.lua"), raw_args),
    };

    if !script.exists() {
        eprintln!("usage: neon <script.lua>");
        eprintln!("or place a config.lua in the current directory");
        std::process::exit(2);
    }

    let source = fs::read_to_string(&script)
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

    let neon = Neon::new()?;
    neon.set_args(&config_args)?;
    let exec_result = neon.exec_source(&source, script.to_string_lossy().as_ref());
    let shutdown_result = neon.shutdown();

    match (exec_result, shutdown_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(exec_err), Err(_shutdown_err)) => Err(exec_err),
    }
}
