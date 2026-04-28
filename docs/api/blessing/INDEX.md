# Blessing Docs

`blessing` is Neon’s feature-gated TUI module, built on top of:

- `ratatui` + `crossterm` for terminal UI/layout/render/input
- `tachyonfx` for effect DSL compilation (exposed as `blessing.fx`)

If Neon is built without the `blessing` feature, `require("blessing")` fails by design.

## Pages

- [TUI API](./TUI.md)
- [Effects API](./EFFECTS.md)
