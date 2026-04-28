# Blessing TUI API

## Module Table

- `blessing.new() -> ui`
- `blessing.available == true`
- `blessing.codename == "blessing"`
- `blessing.version == "0.2"`

## UI Object Methods

### Lifecycle

- `ui:finish()`
  - Leaves alternate screen and disables raw mode.
  - Safe to call multiple times.

### Layout and Rendering

- `ui:set_layout(layout_table)`
- `ui:render()`
- `ui:size() -> width, height`

### Input State

- `ui:set_input(text)`
- `ui:input() -> text`

### Event APIs

- `ui:poll_event([timeout_ms]) -> event|nil`
- `ui:read_key([timeout_ms]) -> key_event|nil`
- `ui:read_line() -> string`

`read_line()` behavior:

- character keys append
- backspace removes
- enter returns line and clears input
- esc returns empty string and clears input

## Layout Tree Schema

Each node is a Lua table.

- `id: string?`
- `direction: "vertical"|"horizontal"` (default: `vertical`)
- `constraints: table?`
- `margin: integer?`
- `children: {node, ...}?`
- `render: function(ctx) -> widget_spec|{widget_spec,...}?`

Traversal/rendering flow:

1. Apply node margin.
2. Call node `render(ctx)` for this area.
3. Render returned widget(s) into this exact area.
4. Split area with `direction + constraints`.
5. Recurse into `children` with computed sub-areas.

### Constraints

String forms:

- `"length:N"` / `"len:N"`
- `"min:N"`
- `"max:N"`
- `"percentage:N"` / `"pct:N"`
- `"ratio:A:B"`

Table forms:

- `{ kind = "length", value = N }`
- `{ kind = "min", value = N }`
- `{ kind = "max", value = N }`
- `{ kind = "percentage", value = N }`
- `{ kind = "ratio", numerator = A, denominator = B }`

## Render Callback Context

`ctx` fields:

- `ctx.x`, `ctx.y`, `ctx.width`, `ctx.height`
- `ctx.input`
- `ctx.frame`
- `ctx.path`
- `ctx.id` (when node has `id`)

## Widget Spec

Common widget fields:

- `kind: string` (default: `"paragraph"`)
- `style: style_table?`
- `block: block_table?`
- `clear: boolean?`

Kinds:

- `paragraph`: `text`, `wrap?`
- `list`: `items`
- `tabs`: `titles`, `selected`
- `gauge`: `ratio`, `label?`
- `sparkline`: `values`, `bar_set?`

## Block Spec

`block = { ... }`:

- `title: string?`
- `borders: string|list|false`

Borders string values:

- `"all"|"none"|"top"|"bottom"|"left"|"right"|"vertical"|"horizontal"`

## Style Spec

`style = { ... }`:

- `fg: color_string?`
- `bg: color_string?`
- `bold: boolean?`
- `modifiers: {modifier_name, ...}?`

Colors:

- named colors and `"#RRGGBB"`

Modifiers:

- `bold`, `dim`, `italic`, `underlined`, `reversed`, `slow_blink`, `rapid_blink`, `crossed_out`

## Event Tables

- Key: `kind = "key"`, `name`, optional `char`, `ctrl/alt/shift`
- Resize: `kind = "resize"`, `width`, `height`
- Mouse: `kind = "mouse"`, `name`, `x`, `y`
- Other: `kind = "other"`
