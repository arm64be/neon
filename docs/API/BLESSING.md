# Blessing Module

`blessing` is a feature-gated Lua module that exposes a declarative, stateful TUI renderer backed by ratatui.

- Module name: `require("blessing")`
- Cargo feature: `blessing`
- Default: enabled
- If built without the feature, `require("blessing")` fails (intended for graceful fallback in Lua).

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
  - Sets the retained declarative layout tree used for subsequent renders.
- `ui:render()`
  - Renders one frame from the current layout.
- `ui:size() -> width, height`
  - Returns current terminal size.

### Input State

- `ui:set_input(text)`
  - Sets the internal input string exposed to render callbacks as `ctx.input`.
- `ui:input() -> text`
  - Reads current internal input string.

### Event APIs

- `ui:poll_event([timeout_ms]) -> event|nil`
  - Polls and reads one terminal event.
  - Returns `nil` when no event before timeout.
- `ui:read_key([timeout_ms]) -> key_event|nil`
  - Polls/reads one key press event only.
  - Returns `nil` for timeout or non-key events.
- `ui:read_line() -> string`
  - Built-in line editor loop.
  - Behavior:
    - character keys append
    - backspace removes
    - enter returns line and clears input
    - esc returns empty string and clears input
  - Re-renders while editing.

## Layout Tree Schema

Each node is a Lua table.

- `id: string?`
- `direction: "vertical"|"horizontal"` (default: `vertical`)
- `constraints: table?`
- `margin: integer?`
- `children: {node, ...}?`
- `render: function(ctx) -> widget_spec|{widget_spec,...}?`

Rust owns splitting/traversal:

1. Apply node margin.
2. Call node `render(ctx)` for this area.
3. Render returned widget(s) into this exact area.
4. Split area with `direction + constraints`.
5. Recurse into `children` with computed sub-areas.

### Constraints

Supported string forms:

- `"length:N"` / `"len:N"`
- `"min:N"`
- `"max:N"`
- `"percentage:N"` / `"pct:N"`
- `"ratio:A:B"`

Supported table forms:

- `{ kind = "length", value = N }`
- `{ kind = "min", value = N }`
- `{ kind = "max", value = N }`
- `{ kind = "percentage", value = N }`
- `{ kind = "ratio", numerator = A, denominator = B }`

If omitted/invalid, fallback is `min:1`.

## Render Callback Context

`ctx` fields passed into `render(ctx)`:

- `ctx.x`, `ctx.y`, `ctx.width`, `ctx.height`
- `ctx.input`
- `ctx.frame` (incrementing frame number)
- `ctx.path` (node path like `root.0.1`)
- `ctx.id` (only when node has `id`)

## Widget Spec

`render(ctx)` returns either:

- one widget table
- array of widget tables (layered draw order)

Common widget fields:

- `kind: string` (defaults to `"paragraph"`)
- `style: style_table?`
- `block: block_table?`
- `clear: boolean?` (draw `Clear` before widget)

### `paragraph`

- `kind = "paragraph"`
- `text: string`
- `wrap: boolean?` (default: `true`)

### `list`

- `kind = "list"`
- `items: {string, ...}`

### `tabs`

- `kind = "tabs"`
- `titles: {string, ...}`
- `selected: integer` (0-based)

### `gauge`

- `kind = "gauge"`
- `ratio: number` (`0.0..1.0`, clamped)
- `label: string?`

### `sparkline`

- `kind = "sparkline"`
- `values: {number, ...}`
- `bar_set: "braille"?` (currently maps to 9-level bars)

## Block Spec

`block = { ... }`

- `title: string?`
- `borders: ...`

`borders` accepts:

- string: `"all"|"none"|"top"|"bottom"|"left"|"right"|"vertical"|"horizontal"`
- list: `{ "top", "left", ... }`

## Style Spec

`style = { ... }`

- `fg: color_string?`
- `bg: color_string?`
- `bold: boolean?`
- `modifiers: {modifier_name, ...}?`

Colors:

- named: `black red green yellow blue magenta cyan gray dark_gray light_red light_green light_yellow light_blue light_magenta light_cyan white`
- rgb hex: `"#RRGGBB"`

Modifiers:

- `bold`, `dim`, `italic`, `underlined`, `reversed`, `slow_blink`, `rapid_blink`, `crossed_out`

## Event Tables

### Key Event

`ui:read_key` and keyboard results from `ui:poll_event` return:

- `kind = "key"`
- `name` (e.g. `"enter"`, `"esc"`, `"left"`, `"f1"`, or character)
- `char` (only for char keys)
- `ctrl`, `alt`, `shift` booleans

### Resize Event

- `kind = "resize"`
- `width`, `height`

### Mouse Event

- `kind = "mouse"`
- `name = "down"|"up"|"drag"|"moved"|"scroll_down"|"scroll_up"|"scroll_left"|"scroll_right"`
- `x`, `y`

### Other Event

- `kind = "other"`

## Example

```lua
local blessing = require("blessing")
local ui = blessing.new()

local state = {
  title = "Demo",
  lines = { "hello" },
}

ui:set_layout({
  id = "root",
  direction = "vertical",
  constraints = { "length:3", "min:2", "length:3" },
  children = {
    {
      id = "head",
      render = function(ctx)
        return {
          kind = "paragraph",
          text = "frame=" .. ctx.frame,
          style = { fg = "cyan", modifiers = { "bold" } },
          block = { title = state.title, borders = "all" },
        }
      end,
    },
    {
      id = "body",
      render = function(_ctx)
        return {
          kind = "list",
          items = state.lines,
          block = { title = "Lines", borders = "all" },
        }
      end,
    },
    {
      id = "input",
      render = function(ctx)
        return {
          kind = "paragraph",
          text = "> " .. (ctx.input or ""),
          block = { title = "Input", borders = "all" },
        }
      end,
    },
  },
})

while true do
  ui:render()
  local key = ui:read_key(50)
  if key and key.kind == "key" and key.name == "esc" then
    break
  end
end

ui:finish()
```
