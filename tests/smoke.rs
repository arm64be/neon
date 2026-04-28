use neon::Neon;
use tempfile::tempdir;

#[test]
fn lua_module_loads() {
    let neon = Neon::new().expect("neon");
    neon.set_args(&["--oneshot".to_string(), "ping".to_string()])
        .expect("args");
    neon.exec_source(
        r#"
        local neon = require("neon")
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
            neon.set_session_db("{db_path}")
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
            neon.set_session_db("{db_path}")
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

#[cfg(feature = "blessing")]
#[test]
fn blessing_module_loads() {
    let neon = Neon::new().expect("neon");
    neon.exec_source(
        r#"
        local blessing = require("blessing")
        assert(blessing.available == true)
        assert(blessing.codename == "blessing")
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
                  kind = "paragraph",
                  text = "body:" .. (ctx.input or ""),
                  block = { title = "body", borders = "all" },
                }
              end
            }
          }
        })
        ui:set_input("hello")
        "#,
        "blessing-smoke.lua",
    )
    .expect("blessing module");
}
