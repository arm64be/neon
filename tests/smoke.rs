use neon::{create_lua, set_args};

#[test]
fn lua_module_loads() {
    let lua = create_lua().expect("lua");
    set_args(&lua, &["--oneshot".to_string(), "ping".to_string()]).expect("args");
    lua.load(
        r#"
        local neon = require("neon")
        local session = neon.new_session("smoke")
        assert(neon.args[1] == "--oneshot")
        assert(neon.util.arg_flag("oneshot"))
        assert(neon.util.arg_value("prompt") == nil)
        assert(neon.util.arg_value_or("prompt", "fallback") == "fallback")
        local glob = neon.util.arg_glob()
        assert(#glob == 1 and glob[1] == "ping")
        assert(neon.util.trim_string("  hi \n") == "hi")
        local encoded = neon.json.encode({ a = 1, b = "x" })
        local decoded = neon.json.decode(encoded)
        assert(decoded.a == 1 and decoded.b == "x")
        session:set_model(function(state)
          return { kind = "final", content = "ok" }
        end)
        session:push("user", "ping")
        assert(session:run() == "ok")
    "#,
    )
    .exec()
    .expect("script");
}
