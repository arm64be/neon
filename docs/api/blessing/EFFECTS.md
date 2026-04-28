# Blessing Effects API

`blessing.fx` exposes a TachyonFX DSL compiler and effect handles.

## Module Table

- `blessing.fx.new() -> dsl`
- `blessing.fx.compile(source) -> effect`
- `blessing.fx.available == true`
- `blessing.fx.codename == "tachyonfx"`
- `blessing.fx.version == <neon package version>`

## DSL Object

- `dsl:compile(source) -> effect`

`source` is TachyonFX DSL text, for example:

```lua
local blessing = require("blessing")
local effect = blessing.fx.compile("fx::dissolve(500)")
```

## Effect Object

- `effect:name() -> string`
- `effect:done() -> boolean`
- `effect:running() -> boolean`
- `effect:source() -> string|nil`
- `effect:clone() -> effect`

## Notes

- This API compiles and stores effects; Blessing does not yet apply/render these effects in the TUI pipeline automatically.
