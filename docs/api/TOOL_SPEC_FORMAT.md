# Tool Spec Format

When adding a tool with a table spec, the accepted fields are:

- `name` required
- `description` optional
- `parameters` optional, defaults to an empty object schema

Example:

```lua
session:add_tool({
  name = "lookup",
  description = "Look up a value",
  parameters = {
    type = "object",
    properties = {
      key = { type = "string" },
    },
    required = { "key" },
    additionalProperties = false,
  },
}, function(args)
  return "ok"
end)
```
