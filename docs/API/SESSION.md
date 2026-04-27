# Session API

Defined in [`src/session.rs`](../../src/session.rs).

## Construction

- `neon.new_session([name])` returns a session userdata.
- If `name` is omitted, Neon generates a unique session name.
- When a session database is configured with `neon.set_session_db(path)`, creating a session with the same name loads the saved history for that session.

## Read methods

- `session:id()` returns the numeric session id.
- `session:name()` returns the session name.
- `session:history()` returns the message history as `{ role, content }` tables.
- `session:context()` returns the mutable session context table.
- `session:tools()` returns a map of registered tool names.
- `session:tool_specs()` returns OpenAI-compatible tool specs.

## Model and interface

- `session:set_model(fn)` sets the model provider callback.
- `session:set_interface(fn)` sets the interface callback.
- `session:run_interface()` invokes the interface callback with the current payload.

## Tooling

- `session:add_tool(name, fn)` registers a tool with an empty parameter schema.
- `session:add_tool(spec_table, fn)` registers a tool from a table spec.
- `session:call_tool(name, args)` invokes a registered tool and runs action hooks.
- `session:remove_tool(name)` removes a tool from the session.

## Hooks and execution

- `session:add_context_hook(fn)` runs before each model call.
- `session:add_action_hook(fn)` runs after each tool call.
- `session:push(role, content)` appends a message to history.
- `session:step()` runs one model step and returns a table with `kind = "final"` or `kind = "tool"`.
- `session:run(max_steps)` loops until the model returns a final response.
