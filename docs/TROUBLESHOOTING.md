# Troubleshooting

## `config.lua` is not found

Check the config root resolution order in `src/main.rs`:

1. `NEON_CONFIG_ROOT`
2. Current directory in debug builds
3. `$XDG_CONFIG_HOME/neon/config.lua`
4. `$HOME/.config/neon/config.lua`

If you are launching the binary directly, pass a script path or set `NEON_CONFIG_ROOT` to a directory that contains `config.lua`.

## `.env` parsing fails

Neon accepts `KEY=VALUE` and `export KEY=VALUE` lines only.

Common causes:

- Missing `=`
- Empty key name
- Using shell syntax that is not plain assignment

The error message includes the file path and line number.

## Session model is not set

`session:step()` and `session:run()` require a model provider.

Set one first:

```lua
session:set_model(function(payload)
  return { kind = "final", content = "ready" }
end)
```

## Session interface is not set

`session:run_interface()` requires an interface callback.

Set one first:

```lua
session:set_interface(function(payload)
  return "interface ready"
end)
```

## Tool argument errors

Built-in tools validate required fields:

- `read_file` requires `path`
- `write_file` requires `path` and `content`
- `bash` requires `command`

If the tool call omits a required field, Neon raises a runtime error naming the missing argument.

## HTTP issues

`neon.tokio.http(...)` and `neon.tokio.http_stream(...)` expect:

- A valid HTTP method string
- A valid URL
- Optional header and parameter tables

If the request fails, the returned runtime error contains the underlying `reqwest` message.

## Shutdown hooks do not run

Shutdown hooks only run when Neon reaches `Neon::shutdown()`. If your Lua code exits early or the process is terminated externally, hooks may not execute.
