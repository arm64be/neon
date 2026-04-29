use neon::Neon;
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tempfile::tempdir;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let to = to.as_ref();
    fs::create_dir_all(to.parent().expect("parent")).expect("create parent");
    fs::copy(from, to).expect("copy file");
}

fn copy_dir(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let from = from.as_ref();
    let to = to.as_ref();
    fs::create_dir_all(to).expect("create dir");
    for entry in fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("entry");
        let file_type = entry.file_type().expect("file type");
        let target = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(entry.path(), target);
        } else if file_type.is_file() {
            copy_file(entry.path(), target);
        }
    }
}

#[test]
fn lua_module_loads() {
    let neon = Neon::new().expect("neon");
    neon.set_args(&["--oneshot".to_string(), "ping".to_string()])
        .expect("args");
    neon.exec_source(
        r#"
        local neon = require("neon")
        local sqlite = require("sqlite")
        local session = neon.new_session("smoke")
        assert(neon.args[1] == "--oneshot")
        assert(neon.tokio ~= nil)
        assert(coroutine == nil)
        assert(neon.util.arg_flag("oneshot"))
        assert(neon.util.arg_value("prompt") == nil)
        assert(neon.util.arg_value_or("prompt", "fallback") == "fallback")
        local glob = neon.util.arg_glob()
        assert(#glob == 1 and glob[1] == "ping")
        assert(neon.util.trim_string("  hi \n") == "hi")
        local encoded = neon.json.encode({ a = 1, b = "x" })
        local decoded = neon.json.decode(encoded)
        assert(decoded.a == 1 and decoded.b == "x")
        assert(neon.tools ~= nil)
        local mem = sqlite.memory()
        assert(type(mem:id()) == "string")
        local models = sqlite.schema(mem, {
          todos = {
            id = { type = "INTEGER", primary_key = true, nullable = false },
            title = { type = "TEXT", nullable = false },
            done = { type = "INTEGER", nullable = false, default = "0" },
          },
        })
        models.todos:insert({ id = 1, title = "first", done = 0 })
        local rows = models.todos:where("id = ?", { 1 })
        assert(#rows == 1 and rows[1].title == "first")
        local specs = session:tool_specs()
        assert(specs[1].type == "function")
        assert(specs[1]["function"].name == "bash")
        session:add_tool({
          name = "double",
          description = "Double a value",
          parameters = {
            type = "object",
            properties = {
              value = { type = "number" },
            },
            required = { "value" },
            additionalProperties = false,
          },
        }, function(args)
          return { doubled = args.value * 2 }
        end)
        local tool_result = session:call_tool("double", { value = 21 })
        assert(tool_result.doubled == 42)
        session:set_model(function(state)
          return { kind = "final", content = "ok" }
        end)
        session:push("user", "ping")
        assert(session:run() == "ok")
    "#,
        "smoke.lua",
    )
    .expect("script");
}

#[test]
fn config_directories_replace_examples() {
    assert!(Path::new("configs/onboarding/config.lua").is_file());
    assert!(Path::new("configs/onboarding/questions/introduction.lua").is_file());
    assert!(Path::new("configs/onboarding/provider.lua").is_file());
    assert!(Path::new("configs/onboarding/interface.lua").is_file());
    assert!(Path::new("configs/preset/config.lua").is_file());
    assert!(Path::new("configs/preset/agent.lua").is_file());
    assert!(Path::new("configs/preset/tools/bash.lua").is_file());
    assert!(Path::new("configs/preset/providers/init.lua").is_file());
    assert!(Path::new("configs/themes/init.lua").is_file());
    assert!(!Path::new("examples").exists());
}

#[test]
fn install_script_is_curl_pipe_ready() {
    let script = fs::read_to_string("install.sh").expect("install script");
    assert!(script.starts_with("#!/usr/bin/env bash"));
    assert!(script.contains("https://api.github.com/repos/${repo}/releases/latest"));
    assert!(script.contains("$HOME/.local/bin"));
    assert!(script.contains("NEON_APP"));
    assert!(script.contains("XDG_CONFIG_HOME"));
    assert!(script.contains("NEON_LOCAL_REPO"));
    assert!(script.contains("cargo run --manifest-path"));
    assert!(script.contains("unsupported platform: macOS releases are not published yet."));
    assert!(script.contains("unsupported Linux architecture: ${arch}."));
    assert!(script.contains("This installer currently supports only Linux x86_64."));
    assert!(script.contains("NEON_CONFIG_ROOT=\"${app_dir}\""));
    assert!(script.contains("NEON_ONBOARDING_PRESET_SOURCE=\"${preset_source}/preset\""));
}

#[test]
fn preset_config_loads_core_setup_without_network() {
    let dir = tempdir().expect("dir");
    let configs = dir.path().join("configs");
    let root = configs.join("preset");
    copy_dir("configs/preset", &root);
    copy_dir("configs/themes", configs.join("themes"));

    let neon = Neon::new().expect("neon");
    neon.set_config_root(&root).expect("config root");
    neon.exec_source(
        r#"
        local neon = require("neon")
        package.path = table.concat({
          neon.config_root .. "/?.lua",
          neon.config_root .. "/?/init.lua",
          neon.config_root .. "/../?.lua",
          neon.config_root .. "/../?/init.lua",
          package.path,
        }, ";")

        local themes = require("themes")
        assert(themes.is_valid("catppuccin-mocha"))
        assert(themes.is_valid("tokyo-night"))

        local sessions = require("sessions")
        local tools = require("tools")
        local session = sessions.new("preset-loads")
        tools.register(session)
        local specs = session:tool_specs()
        local seen = {}
        for _, spec in ipairs(specs) do
          seen[spec["function"].name] = true
        end
        assert(seen.bash)
        assert(seen.web_fetch)
        assert(seen.web_text)
    "#,
        "preset-load.lua",
    )
    .expect("preset core setup");
}

#[test]
fn onboarding_completion_replaces_itself_with_preset() {
    let _guard = env_lock().lock().expect("env lock");
    let old_test_mode = env::var("NEON_ONBOARDING_TEST_MODE").ok();
    let old_repo_root = env::var("NEON_REPOSITORY_ROOT").ok();
    let old_site_name = env::var("NEON_ONBOARDING_SITE_NAME").ok();
    let old_model = env::var("NEON_ONBOARDING_MODEL").ok();
    let old_api_base = env::var("NEON_ONBOARDING_API_BASE").ok();
    let old_api_key = env::var("NEON_ONBOARDING_API_KEY").ok();
    let old_preset_source = env::var("NEON_ONBOARDING_PRESET_SOURCE").ok();
    let old_intro = env::var("NEON_ONBOARDING_INTRODUCTION").ok();
    let old_providers = env::var("NEON_ONBOARDING_PROVIDERS").ok();
    let old_provider_key = env::var("NEON_ONBOARDING_PROVIDER_OPENROUTER_API_KEY").ok();
    let old_instructions_choice = env::var("NEON_ONBOARDING_CUSTOM_INSTRUCTIONS").ok();
    let old_instructions = env::var("NEON_ONBOARDING_INSTRUCTIONS").ok();
    let old_theme = env::var("NEON_ONBOARDING_THEME").ok();
    let old_providers_json = env::var("NEON_ONBOARDING_PROVIDERS_JSON").ok();

    let dir = tempdir().expect("dir");
    let repo = dir.path().join("repo");
    let onboarding = repo.join("configs/onboarding");
    let preset = repo.join("configs/preset");
    copy_dir("configs/onboarding", &onboarding);
    copy_dir("configs/preset", &preset);
    copy_dir("configs/themes", repo.join("configs/themes"));

    env::set_var("NEON_ONBOARDING_TEST_MODE", "1");
    env::set_var("NEON_REPOSITORY_ROOT", repo.to_string_lossy().as_ref());
    env::set_var("NEON_ONBOARDING_INTRODUCTION", "hi");
    env::set_var("NEON_ONBOARDING_PROVIDERS", "OpenRouter");
    env::set_var(
        "NEON_ONBOARDING_PROVIDERS_JSON",
        r#"[{"id":"openrouter","name":"OpenRouter","base_url":"https://openrouter.ai/api/v1","type":"openai","authentication":"api_key"}]"#,
    );
    env::set_var("NEON_ONBOARDING_PROVIDER_OPENROUTER_API_KEY", "test-key");
    env::set_var("NEON_ONBOARDING_CUSTOM_INSTRUCTIONS", "yes");
    env::set_var("NEON_ONBOARDING_INSTRUCTIONS", "Prefer tests.");
    env::set_var("NEON_ONBOARDING_THEME", "tokyo-night");
    env::set_var(
        "NEON_ONBOARDING_PRESET_SOURCE",
        preset.to_string_lossy().as_ref(),
    );

    let neon = Neon::new().expect("neon");
    neon.set_config_root(&onboarding).expect("config root");
    let source = fs::read_to_string(onboarding.join("config.lua")).expect("source");
    neon.exec_source(&source, "onboarding.lua")
        .expect("onboarding completion");

    let installed = fs::read_to_string(onboarding.join("config.lua")).expect("installed config");
    let env_file = fs::read_to_string(onboarding.join(".env")).expect("env file");
    let user_data = fs::read_to_string(onboarding.join("user_data.lua")).expect("user data");
    let selected =
        fs::read_to_string(onboarding.join("providers/selected.lua")).expect("selected providers");
    assert!(installed.contains("local agent = require(\"agent\")"));
    assert!(onboarding.join("tools/bash.lua").is_file());
    assert!(PathBuf::from(format!(
        "{}.onboarding-backup",
        onboarding.to_string_lossy()
    ))
    .join("config.lua")
    .is_file());
    assert!(env_file.contains("NEON_PROVIDER_OPENROUTER_API_KEY=test-key"));
    assert!(user_data.contains("theme = \"tokyo-night\""));
    assert!(user_data.contains("instructions = \"Prefer tests.\""));
    assert!(selected.contains("id = \"openrouter\""));

    restore_env("NEON_ONBOARDING_TEST_MODE", old_test_mode);
    restore_env("NEON_REPOSITORY_ROOT", old_repo_root);
    restore_env("NEON_ONBOARDING_SITE_NAME", old_site_name);
    restore_env("NEON_ONBOARDING_MODEL", old_model);
    restore_env("NEON_ONBOARDING_API_BASE", old_api_base);
    restore_env("NEON_ONBOARDING_API_KEY", old_api_key);
    restore_env("NEON_ONBOARDING_PRESET_SOURCE", old_preset_source);
    restore_env("NEON_ONBOARDING_INTRODUCTION", old_intro);
    restore_env("NEON_ONBOARDING_PROVIDERS", old_providers);
    restore_env(
        "NEON_ONBOARDING_PROVIDER_OPENROUTER_API_KEY",
        old_provider_key,
    );
    restore_env(
        "NEON_ONBOARDING_CUSTOM_INSTRUCTIONS",
        old_instructions_choice,
    );
    restore_env("NEON_ONBOARDING_INSTRUCTIONS", old_instructions);
    restore_env("NEON_ONBOARDING_THEME", old_theme);
    restore_env("NEON_ONBOARDING_PROVIDERS_JSON", old_providers_json);
}

fn restore_env(name: &str, value: Option<String>) {
    if let Some(value) = value {
        env::set_var(name, value);
    } else {
        env::remove_var(name);
    }
}

#[test]
fn shutdown_hook_runs() {
    let neon = Neon::new().expect("neon");
    neon.exec_source(
        r#"
        local neon = require("neon")
        seen = false
        neon.lifecycle.on_shutdown(function()
          seen = true
        end)
    "#,
        "shutdown.lua",
    )
    .expect("script");
    neon.shutdown().expect("shutdown");
    let seen: bool = neon.lua().globals().get("seen").expect("seen");
    assert!(seen);
}

#[test]
fn session_db_resumes_history() {
    let dir = tempdir().expect("dir");
    let db_path = dir.path().join("sessions.sqlite3");
    let db_path_str = db_path.to_string_lossy().replace('\\', "\\\\");

    let neon = Neon::new().expect("neon");
    neon.exec_source(
        &format!(
            r#"
            local neon = require("neon")
            local sqlite = require("sqlite")
            local db = sqlite.connect("{db_path}")
            neon.set_session_db(db)
            local session = neon.new_session("resume-smoke")
            session:push("user", "hello")
        "#,
            db_path = db_path_str
        ),
        "persist-write.lua",
    )
    .expect("persist write");

    let neon = Neon::new().expect("neon");
    neon.exec_source(
        &format!(
            r#"
            local neon = require("neon")
            local sqlite = require("sqlite")
            local db = sqlite.connect("{db_path}")
            neon.set_session_db(db:id())
            local session = neon.new_session("resume-smoke")
            local history = session:history()
            assert(#history == 1)
            assert(history[1].role == "user")
            assert(history[1].content == "hello")
        "#,
            db_path = db_path_str
        ),
        "persist-read.lua",
    )
    .expect("persist read");
}

#[test]
fn sqlite_connection_helpers_work_in_memory() {
    let neon = Neon::new().expect("neon");
    neon.exec_source(
        r#"
        local neon = require("neon")
        local sqlite = require("sqlite")
        local db = sqlite.memory()

        assert(type(db:id()) == "string")
        assert(db:exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT NOT NULL)") == 0)
        assert(db:exec("INSERT INTO notes (id, title) VALUES (?, ?)", { 1, "hello" }) == 1)
        assert(db:exec("INSERT INTO notes (id, title) VALUES (?, ?)", { 2, "world" }) == 1)

        local rows = db:query("SELECT id, title FROM notes WHERE id > ? ORDER BY id ASC", { 0 })
        assert(#rows == 2)
        assert(rows[1].id == 1 and rows[1].title == "hello")
        assert(rows[2].id == 2 and rows[2].title == "world")

        local one = db:one("SELECT id, title FROM notes WHERE id = ?", { 2 })
        assert(one ~= nil and one.title == "world")
        local none = db:one("SELECT id, title FROM notes WHERE id = ?", { 999 })
        assert(none == nil)
    "#,
        "sqlite-helpers-in-memory.lua",
    )
    .expect("sqlite helper smoke");
}

#[test]
fn sqlite_schema_models_work_in_memory() {
    let neon = Neon::new().expect("neon");
    neon.exec_source(
        r#"
        local sqlite = require("sqlite")
        local db = sqlite.memory()
        local models = db:schema({
          todos = {
            id = { type = "INTEGER", primary_key = true, nullable = false },
            title = { type = "TEXT", nullable = false },
            done = { type = "INTEGER", nullable = false, default = "0" },
          },
        })

        models.todos:insert({ id = 1, title = "first", done = 0 })
        models.todos:insert({ id = 2, title = "second", done = 1 })

        local filtered = models.todos:where("done = ?", { 1 })
        assert(#filtered == 1)
        assert(filtered[1].id == 2 and filtered[1].title == "second")

        local all = models.todos:all()
        assert(#all == 2)
    "#,
        "sqlite-schema-in-memory.lua",
    )
    .expect("sqlite schema smoke");
}

#[test]
fn session_db_works_with_in_memory_sqlite_connection() {
    let neon = Neon::new().expect("neon");
    neon.exec_source(
        r#"
        local neon = require("neon")
        local sqlite = require("sqlite")
        local db = sqlite.memory()
        neon.set_session_db(db)

        local first = neon.new_session("memory-session")
        first:push("user", "hello")

        local second = neon.new_session("memory-session")
        local history = second:history()
        assert(#history == 1)
        assert(history[1].role == "user")
        assert(history[1].content == "hello")
    "#,
        "session-db-in-memory.lua",
    )
    .expect("session db memory smoke");
}

#[cfg(feature = "blessing")]
#[test]
fn blessing_module_loads() {
    let neon = Neon::new().expect("neon");
    neon.exec_source(
        r#"
        local blessing = require("blessing")
        assert(blessing.available == true)
        assert(blessing.codename == "blessing")
        assert(blessing.version ~= nil)
        assert(blessing.fx ~= nil)
        assert(blessing.fx.available == true)
        local dsl = blessing.fx.new()
        local effect = dsl:compile("fx::dissolve(100)")
        assert(effect:name() ~= nil)
        local ui = blessing.new()
        ui:set_layout({
          direction = "vertical",
          constraints = { "length:3", "min:1" },
          children = {
            {
              render = function(ctx)
                return {
                  kind = "paragraph",
                  text = "header",
                  block = { title = "title", borders = "all" },
                  style = { fg = "cyan", bold = true },
                }
              end
            },
            {
              render = function(ctx)
                return {
                  kind = "textarea",
                  placeholder = "body",
                  wrap_mode = "word_or_glyph",
                  block = { title = "body", borders = "all" },
                }
              end
            }
          }
        })
        ui:set_input("hello")
        assert(ui:input() == "hello")
        local ok1, ev = pcall(function() return ui:poll_event(0) end)
        assert(ok1 or tostring(ev):find("Failed to initialize input reader") ~= nil)
        local ok2, key = pcall(function() return ui:read_key(0) end)
        assert(ok2 or tostring(key):find("Failed to initialize input reader") ~= nil)
        local ok3, input_key = pcall(function() return ui:read_textarea_key(0, true) end)
        assert(ok3 or tostring(input_key):find("Failed to initialize input reader") ~= nil)
    "#,
        "blessing-smoke.lua",
    )
    .expect("blessing module");
}
