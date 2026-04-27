use neon::Neon;

#[test]
fn lua_module_loads() {
    let neon = Neon::new().expect("neon");
    neon.set_args(&["--oneshot".to_string(), "ping".to_string()]).expect("args");
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
