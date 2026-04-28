# Result Shapes

- Final model output can be a string or a table like `{ kind = "final", content = "..." }`.
- Tool output can be a table like `{ kind = "tool", name = "...", args = ... }`.
- `session:step()` returns `{ kind = "final", content = "..." }` or `{ kind = "tool", name = "...", result = "..." }`.
