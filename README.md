# Neon

Minimal Rust + Lua agent harness.

Run `cargo run` from the repo root to load [`config.lua`](/home/mel/Projects/ML/neon/config.lua).

## Shape

- Rust owns the session state, tool registry, and execution loop.
- Lua owns the model provider and the user-facing interface.
- Sessions carry history, mutable context, tools, and hooks.
- Default tools are `read_file`, `write_file`, and `bash`.
- `neon.util.trim_string(s)` trims outer whitespace.
- `neon.util.arg_flag(name)`, `neon.util.arg_value(name)`, and `neon.util.arg_value_or(name, default)` parse CLI args.
- `neon.util.arg_glob()` returns the positional tail after flags.
- `neon.json.encode(value)` and `neon.json.decode(text)` handle JSON.
- `neon.env(name)` and `neon.env_or(name, default)` read environment variables.
- `neon.net.http(...)` and `neon.net.http_stream(...)` cover basic HTTP use cases.
- `neon.args` exposes CLI arguments passed after the script path.

## Lua API

```lua
local neon = require("neon")

local session = neon.new_session("demo")

session:set_model(function(state)
  local last = state.history[#state.history]
  if last and last.role == "user" then
    return { kind = "final", content = "echo: " .. last.content }
  end
  return { kind = "final", content = "ready" }
end)

session:push("user", "hello")
print(session:step().content)
```

## Notes

- `session:context()` returns the session context table.
- `session:history()` returns the full message history.
- `session:add_tool(name, fn)` overrides or adds a tool for that session.
- `session:remove_tool(name)` removes a tool for that session.
- `session:add_context_hook(fn)` runs before each model call.
- `session:add_action_hook(fn)` runs after each tool call.
- `session:run(max_steps)` loops until the model returns a final response.
