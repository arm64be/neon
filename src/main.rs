use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

use neon::Neon;

fn resolve_config_root() -> Option<PathBuf> {
    if let Ok(root) = env::var("NEON_CONFIG_ROOT") {
        return Some(PathBuf::from(root));
    }

    if cfg!(debug_assertions) {
        return Some(PathBuf::from("."));
    } else {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            let candidate = PathBuf::from(xdg).join("neon");
            if candidate.join("config.lua").exists() {
                return Some(candidate);
            }
        }

        if let Ok(home) = env::var("HOME") {
            let candidate = PathBuf::from(home).join(".config").join("neon");
            if candidate.join("config.lua").exists() {
                return Some(candidate);
            }
        }

        None
    }
}

fn apply_env_file(root: &Path) -> mlua::Result<()> {
    let path = root.join(".env");
    if !path.exists() {
        return Ok(());
    }

    let source = fs::read_to_string(&path).map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;
    for (line_no, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            return Err(mlua::Error::RuntimeError(format!(
                "{}:{}: expected KEY=VALUE",
                path.display(),
                line_no + 1
            )));
        };

        let key = key.trim();
        if key.is_empty() {
            return Err(mlua::Error::RuntimeError(format!(
                "{}:{}: env key cannot be empty",
                path.display(),
                line_no + 1
            )));
        }

        if env::var_os(key).is_none() {
            env::set_var(key, value.trim());
        }
    }

    Ok(())
}

fn main() -> mlua::Result<()> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let (script, config_args) = match raw_args.first() {
        Some(first) if !first.starts_with('-') && PathBuf::from(first).exists() => {
            (PathBuf::from(first), raw_args[1..].to_vec())
        }
        _ => {
            let config_root = resolve_config_root().ok_or_else(|| {
                mlua::Error::RuntimeError(
                    "unable to find config root; set NEON_CONFIG_ROOT or create config.lua under $XDG_CONFIG_HOME/neon or $HOME/.config/neon"
                        .into(),
                )
            })?;
            (config_root.join("config.lua"), raw_args)
        }
    };

    if !script.exists() {
        eprintln!("usage: neon <script.lua>");
        eprintln!("or set NEON_CONFIG_ROOT to a config root containing config.lua");
        std::process::exit(2);
    }

    let source = fs::read_to_string(&script)
        .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

    let neon = Neon::new()?;
    neon.set_args(&config_args)?;
    if let Some(config_root) = script.parent() {
        let config_root = if config_root.as_os_str().is_empty() {
            Path::new(".")
        } else {
            config_root
        };
        apply_env_file(config_root)?;
        neon.set_config_root(config_root)?;
    }
    let exec_result = neon.exec_source(&source, script.to_string_lossy().as_ref());
    let shutdown_result = neon.shutdown();

    match (exec_result, shutdown_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(exec_err), Err(_shutdown_err)) => Err(exec_err),
    }
}

#[cfg(test)]
mod tests {
    use super::apply_env_file;
    use std::{env, fs};
    use tempfile::tempdir;

    #[test]
    fn dotenv_file_sets_missing_env() {
        let dir = tempdir().expect("dir");
        let path = dir.path().join(".env");
        fs::write(&path, "NEON_DOTENV_TEST=value\n").expect("write");
        env::remove_var("NEON_DOTENV_TEST");
        apply_env_file(dir.path()).expect("apply");
        assert_eq!(env::var("NEON_DOTENV_TEST").as_deref(), Ok("value"));
    }
}
