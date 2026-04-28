# Lua Module

The `neon` module is registered automatically.

Optional modules:

- `blessing` is available as a separate `require("blessing")` module when built with the `blessing` feature (enabled by default). See [Blessing Module](BLESSING.md).

- `neon.new_session([name])` creates a new session. If `name` is omitted, Neon generates a unique name.
- `neon.set_session_db(path)` stores session history in the given SQLite database file. Reusing the same session name resumes its history from that file.
- `neon.util.trim_string(s)` trims outer whitespace.
- `neon.util.arg_flag(name)` checks whether a CLI flag is present.
- `neon.util.arg_value(name)` returns the value for `--name=value`.
- `neon.util.arg_value_or(name, default)` returns a value or default.
- `neon.util.arg_glob()` returns positional arguments after flags.
- `neon.json.encode(value)` serializes Lua values to JSON.
- `neon.json.decode(text)` parses JSON into Lua values.
- `neon.env(name)` reads an environment variable and returns `nil` when absent.
- `neon.env_or(name, default)` reads an environment variable with a fallback.
- `neon.tokio.sleep(ms)` sleeps asynchronously.
- `neon.tokio.http(method, url, headers, params, body)` performs an HTTP request and returns a table with `status`, `headers`, and `body`.
- `neon.tokio.http_stream(method, url, headers, params, body, on_line)` streams response lines into a callback.
- `neon.tools.read_file(path)` reads a file into a string.
- `neon.tools.write_file(path, content)` writes a file and returns a status string.
- `neon.tools.bash(command)` runs `bash -lc <command>` and returns a single formatted string with the exit code, stdout, and stderr.
- `neon.lifecycle.on_shutdown(fn)` registers a shutdown hook.

`neon.args` contains CLI arguments passed after the script path, and `neon.config_root` contains the active config root.
