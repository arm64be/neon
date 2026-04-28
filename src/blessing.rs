use std::{io::stdout, time::Duration};

use mlua::{Function, Lua, RegistryKey, Result, Table, UserData, UserDataMethods, Value};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Rect},
    style::{Color, Modifier, Style},
    symbols,
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Sparkline, Tabs, Wrap,
    },
    Terminal,
};

fn rt_err(err: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(err.to_string())
}

type BlessingTerminal = Terminal<CrosstermBackend<std::io::Stdout>>;

struct BlessingState {
    input: String,
    terminal: Option<BlessingTerminal>,
    layout_key: Option<RegistryKey>,
    frame: u64,
}

impl BlessingState {
    fn new() -> Self {
        Self {
            input: String::new(),
            terminal: None,
            layout_key: None,
            frame: 0,
        }
    }

    fn ensure_terminal(&mut self) -> Result<()> {
        if self.terminal.is_some() {
            return Ok(());
        }

        enable_raw_mode().map_err(rt_err)?;
        execute!(stdout(), EnterAlternateScreen).map_err(rt_err)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend).map_err(rt_err)?;
        if let Err(err) = terminal.clear() {
            let _ = execute!(stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            return Err(rt_err(err));
        }

        self.terminal = Some(terminal);
        Ok(())
    }

    fn shutdown_terminal(&mut self) {
        if self.terminal.take().is_some() {
            let _ = execute!(stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
        }
    }

    fn set_layout(&mut self, lua: &Lua, table: Table) -> Result<()> {
        if let Some(key) = self.layout_key.take() {
            lua.remove_registry_value(key)?;
        }
        self.layout_key = Some(lua.create_registry_value(table)?);
        Ok(())
    }

    fn render(&mut self, lua: &Lua) -> Result<()> {
        self.ensure_terminal()?;
        let Some(layout_key) = &self.layout_key else {
            return Err(mlua::Error::RuntimeError(
                "blessing layout is not set; call ui:set_layout(layout)".into(),
            ));
        };

        let root: Table = lua.registry_value(layout_key)?;
        let input = self.input.clone();
        let frame_no = self.frame;

        let terminal = self
            .terminal
            .as_mut()
            .ok_or_else(|| rt_err("blessing terminal is unavailable"))?;

        terminal
            .draw(|frame| {
                let area = frame.area();
                let _ = render_node(lua, frame, root.clone(), area, &input, frame_no, "root");
            })
            .map_err(rt_err)?;
        self.frame = self.frame.saturating_add(1);

        Ok(())
    }

    fn size(&mut self) -> Result<(u16, u16)> {
        self.ensure_terminal()?;
        let terminal = self
            .terminal
            .as_ref()
            .ok_or_else(|| rt_err("blessing terminal is unavailable"))?;
        terminal.size().map(|r| (r.width, r.height)).map_err(rt_err)
    }
}

impl Drop for BlessingState {
    fn drop(&mut self) {
        self.shutdown_terminal();
    }
}

fn parse_direction(value: Option<String>) -> Direction {
    match value.as_deref() {
        Some("horizontal") => Direction::Horizontal,
        _ => Direction::Vertical,
    }
}

fn parse_ratio(text: &str) -> Option<Constraint> {
    let mut parts = text.split(':');
    let a = parts.next()?.parse::<u32>().ok()?;
    let b = parts.next()?.parse::<u32>().ok()?;
    if a == 0 || b == 0 {
        return None;
    }
    Some(Constraint::Ratio(a, b))
}

fn parse_constraints(table: Option<Table>) -> Result<Vec<Constraint>> {
    let Some(table) = table else {
        return Ok(vec![Constraint::Min(1)]);
    };

    let mut constraints = Vec::new();
    for value in table.sequence_values::<Value>() {
        match value? {
            Value::String(s) => {
                let text = s.to_str()?;
                if let Some(rest) = text.strip_prefix("ratio:") {
                    constraints.push(parse_ratio(rest).unwrap_or(Constraint::Min(1)));
                    continue;
                }
                let mut parts = text.splitn(2, ':');
                let kind = parts.next().unwrap_or_default();
                let raw = parts.next().unwrap_or("1").parse::<u16>().unwrap_or(1);
                let c = match kind {
                    "len" | "length" => Constraint::Length(raw),
                    "min" => Constraint::Min(raw),
                    "max" => Constraint::Max(raw),
                    "pct" | "percentage" => Constraint::Percentage(raw),
                    _ => Constraint::Min(1),
                };
                constraints.push(c);
            }
            Value::Table(t) => {
                let kind: String = t.get("kind").unwrap_or_else(|_| "min".into());
                let value: u16 = t.get("value").unwrap_or(1);
                let c = match kind.as_str() {
                    "length" => Constraint::Length(value),
                    "min" => Constraint::Min(value),
                    "max" => Constraint::Max(value),
                    "percentage" => Constraint::Percentage(value),
                    "ratio" => {
                        let numerator: u32 = t.get("numerator").unwrap_or(1);
                        let denominator: u32 = t.get("denominator").unwrap_or(1);
                        Constraint::Ratio(numerator.max(1), denominator.max(1))
                    }
                    _ => Constraint::Min(value),
                };
                constraints.push(c);
            }
            _ => constraints.push(Constraint::Min(1)),
        }
    }

    if constraints.is_empty() {
        constraints.push(Constraint::Min(1));
    }
    Ok(constraints)
}

fn parse_color(name: &str) -> Option<Color> {
    match name {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" => Some(Color::Gray),
        "dark_gray" => Some(Color::DarkGray),
        "light_red" => Some(Color::LightRed),
        "light_green" => Some(Color::LightGreen),
        "light_yellow" => Some(Color::LightYellow),
        "light_blue" => Some(Color::LightBlue),
        "light_magenta" => Some(Color::LightMagenta),
        "light_cyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ if name.starts_with('#') && name.len() == 7 => {
            let r = u8::from_str_radix(&name[1..3], 16).ok()?;
            let g = u8::from_str_radix(&name[3..5], 16).ok()?;
            let b = u8::from_str_radix(&name[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

fn apply_modifier(style: Style, name: &str) -> Style {
    match name {
        "bold" => style.add_modifier(Modifier::BOLD),
        "dim" => style.add_modifier(Modifier::DIM),
        "italic" => style.add_modifier(Modifier::ITALIC),
        "underlined" => style.add_modifier(Modifier::UNDERLINED),
        "reversed" => style.add_modifier(Modifier::REVERSED),
        "slow_blink" => style.add_modifier(Modifier::SLOW_BLINK),
        "rapid_blink" => style.add_modifier(Modifier::RAPID_BLINK),
        "crossed_out" => style.add_modifier(Modifier::CROSSED_OUT),
        _ => style,
    }
}

fn parse_style(style_table: Option<Table>) -> Result<Style> {
    let Some(style_table) = style_table else {
        return Ok(Style::default());
    };

    let mut style = Style::default();
    if let Ok(fg) = style_table.get::<String>("fg") {
        if let Some(color) = parse_color(&fg) {
            style = style.fg(color);
        }
    }
    if let Ok(bg) = style_table.get::<String>("bg") {
        if let Some(color) = parse_color(&bg) {
            style = style.bg(color);
        }
    }
    if style_table.get::<bool>("bold").unwrap_or(false) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if let Ok(modifiers) = style_table.get::<Table>("modifiers") {
        for modifier in modifiers.sequence_values::<String>() {
            style = apply_modifier(style, &modifier?);
        }
    }
    Ok(style)
}

fn parse_borders(spec: Option<Value>) -> Borders {
    match spec {
        Some(Value::Boolean(false)) => Borders::NONE,
        Some(Value::String(s)) => match s.to_str().ok().as_deref() {
            Some("none") => Borders::NONE,
            Some("top") => Borders::TOP,
            Some("bottom") => Borders::BOTTOM,
            Some("left") => Borders::LEFT,
            Some("right") => Borders::RIGHT,
            Some("vertical") => Borders::LEFT | Borders::RIGHT,
            Some("horizontal") => Borders::TOP | Borders::BOTTOM,
            _ => Borders::ALL,
        },
        Some(Value::Table(t)) => {
            let mut borders = Borders::NONE;
            for side in t.sequence_values::<String>().flatten() {
                borders |= match side.as_str() {
                    "top" => Borders::TOP,
                    "bottom" => Borders::BOTTOM,
                    "left" => Borders::LEFT,
                    "right" => Borders::RIGHT,
                    _ => Borders::NONE,
                };
            }
            if borders == Borders::NONE {
                Borders::ALL
            } else {
                borders
            }
        }
        _ => Borders::ALL,
    }
}

fn build_block(spec: Option<Table>) -> Option<Block<'static>> {
    let spec = spec?;
    let mut block = Block::default();
    if let Ok(title) = spec.get::<String>("title") {
        block = block.title(title);
    }
    block = block.borders(parse_borders(spec.get::<Value>("borders").ok()));
    Some(block)
}

fn render_widget(frame: &mut ratatui::Frame, area: Rect, widget: Table) -> Result<()> {
    let kind: String = widget.get("kind").unwrap_or_else(|_| "paragraph".into());
    let block = build_block(widget.get("block").ok());
    let style = parse_style(widget.get("style").ok())?;

    if widget.get::<bool>("clear").unwrap_or(false) {
        frame.render_widget(Clear, area);
    }

    match kind.as_str() {
        "paragraph" => {
            let text: String = widget.get("text").unwrap_or_default();
            let mut paragraph = Paragraph::new(text).style(style);
            if let Some(block) = block {
                paragraph = paragraph.block(block);
            }
            if widget.get::<bool>("wrap").unwrap_or(true) {
                paragraph = paragraph.wrap(Wrap { trim: false });
            }
            frame.render_widget(paragraph, area);
        }
        "list" => {
            let items_table: Option<Table> = widget.get("items").ok();
            let mut items = Vec::new();
            if let Some(items_table) = items_table {
                for item in items_table.sequence_values::<String>() {
                    items.push(ListItem::new(item?));
                }
            }
            let mut list = List::new(items).style(style);
            if let Some(block) = block {
                list = list.block(block);
            }
            frame.render_widget(list, area);
        }
        "gauge" => {
            let ratio = widget.get::<f64>("ratio").unwrap_or(0.0).clamp(0.0, 1.0);
            let label: String = widget.get("label").unwrap_or_default();
            let mut gauge = Gauge::default().ratio(ratio).label(label).style(style);
            if let Some(block) = block {
                gauge = gauge.block(block);
            }
            frame.render_widget(gauge, area);
        }
        "sparkline" => {
            let values_table: Option<Table> = widget.get("values").ok();
            let mut values = Vec::new();
            if let Some(values_table) = values_table {
                for value in values_table.sequence_values::<u64>() {
                    values.push(value?);
                }
            }
            let mut sparkline = Sparkline::default().data(&values).style(style).max(100);
            if widget.get::<String>("bar_set").ok().as_deref() == Some("braille") {
                sparkline = sparkline.bar_set(symbols::bar::NINE_LEVELS);
            }
            if let Some(block) = block {
                sparkline = sparkline.block(block);
            }
            frame.render_widget(sparkline, area);
        }
        "tabs" => {
            let titles_table: Option<Table> = widget.get("titles").ok();
            let mut titles = Vec::new();
            if let Some(titles_table) = titles_table {
                for title in titles_table.sequence_values::<String>() {
                    titles.push(title?);
                }
            }
            let selected = widget.get::<usize>("selected").unwrap_or(0);
            let mut tabs = Tabs::new(titles).select(selected).style(style);
            if let Some(block) = block {
                tabs = tabs.block(block);
            }
            frame.render_widget(tabs, area);
        }
        _ => {}
    }

    Ok(())
}

fn render_spec(frame: &mut ratatui::Frame, area: Rect, spec: Value) -> Result<()> {
    match spec {
        Value::Table(t) => {
            if t.contains_key("kind")? {
                render_widget(frame, area, t)?;
            } else {
                for entry in t.sequence_values::<Value>() {
                    render_spec(frame, area, entry?)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn node_area(node: &Table, area: Rect) -> Rect {
    let margin = node.get::<u16>("margin").unwrap_or(0);
    if margin == 0 || area.width <= margin * 2 || area.height <= margin * 2 {
        return area;
    }
    Rect {
        x: area.x + margin,
        y: area.y + margin,
        width: area.width - (margin * 2),
        height: area.height - (margin * 2),
    }
}

fn render_node(
    lua: &Lua,
    frame: &mut ratatui::Frame,
    node: Table,
    area: Rect,
    input: &str,
    frame_no: u64,
    path: &str,
) -> Result<()> {
    let area = node_area(&node, area);

    if let Ok(render_fn) = node.get::<Function>("render") {
        let ctx = lua.create_table()?;
        ctx.set("x", area.x)?;
        ctx.set("y", area.y)?;
        ctx.set("width", area.width)?;
        ctx.set("height", area.height)?;
        ctx.set("input", input)?;
        ctx.set("frame", frame_no)?;
        ctx.set("path", path)?;
        if let Ok(id) = node.get::<String>("id") {
            ctx.set("id", id)?;
        }

        let spec: Value = render_fn.call(ctx)?;
        render_spec(frame, area, spec)?;
    }

    let children: Option<Table> = node.get("children").ok();
    let Some(children) = children else {
        return Ok(());
    };

    let direction = parse_direction(node.get("direction").ok());
    let constraints = parse_constraints(node.get("constraints").ok())?;
    let chunks = ratatui::layout::Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area);

    for (idx, child) in children.sequence_values::<Table>().enumerate() {
        let child = child?;
        let chunk = if idx < chunks.len() {
            chunks[idx]
        } else {
            *chunks.last().unwrap_or(&area)
        };
        let child_path = format!("{path}.{idx}");
        render_node(lua, frame, child, chunk, input, frame_no, &child_path)?;
    }

    Ok(())
}

fn key_to_name(code: &KeyCode) -> String {
    match code {
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => "backtab".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::Insert => "insert".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::F(n) => format!("f{}", n),
        KeyCode::Char(c) => c.to_string(),
        _ => "unknown".into(),
    }
}

fn key_event_table(lua: &Lua, key: &KeyEvent) -> Result<Table> {
    let table = lua.create_table()?;
    table.set("kind", "key")?;
    table.set("name", key_to_name(&key.code))?;
    if let KeyCode::Char(c) = key.code {
        table.set("char", c.to_string())?;
    }
    table.set("ctrl", key.modifiers.contains(KeyModifiers::CONTROL))?;
    table.set("alt", key.modifiers.contains(KeyModifiers::ALT))?;
    table.set("shift", key.modifiers.contains(KeyModifiers::SHIFT))?;
    Ok(table)
}

fn event_table(lua: &Lua, ev: Event) -> Result<Table> {
    let table = lua.create_table()?;
    match ev {
        Event::Key(key) => return key_event_table(lua, &key),
        Event::Resize(width, height) => {
            table.set("kind", "resize")?;
            table.set("width", width)?;
            table.set("height", height)?;
        }
        Event::Mouse(mouse) => {
            table.set("kind", "mouse")?;
            table.set("x", mouse.column)?;
            table.set("y", mouse.row)?;
            table.set(
                "name",
                match mouse.kind {
                    MouseEventKind::Down(_) => "down",
                    MouseEventKind::Up(_) => "up",
                    MouseEventKind::Drag(_) => "drag",
                    MouseEventKind::Moved => "moved",
                    MouseEventKind::ScrollDown => "scroll_down",
                    MouseEventKind::ScrollUp => "scroll_up",
                    MouseEventKind::ScrollLeft => "scroll_left",
                    MouseEventKind::ScrollRight => "scroll_right",
                },
            )?;
        }
        _ => {
            table.set("kind", "other")?;
        }
    }
    Ok(table)
}

impl UserData for BlessingState {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("set_layout", |lua, this, table: Table| this.set_layout(lua, table));

        methods.add_method_mut("set_input", |_, this, input: String| {
            this.input = input;
            Ok(())
        });

        methods.add_method("input", |_, this, ()| Ok(this.input.clone()));

        methods.add_method_mut("size", |_, this, ()| this.size());

        methods.add_method_mut("render", |lua, this, ()| this.render(lua));

        methods.add_method_mut("poll_event", |lua, _this, timeout_ms: Option<u64>| {
            let timeout = Duration::from_millis(timeout_ms.unwrap_or(0));
            if !event::poll(timeout).map_err(rt_err)? {
                return Ok(Value::Nil);
            }
            let ev = event::read().map_err(rt_err)?;
            Ok(Value::Table(event_table(lua, ev)?))
        });

        methods.add_method_mut("read_key", |lua, _this, timeout_ms: Option<u64>| {
            let timeout = Duration::from_millis(timeout_ms.unwrap_or(0));
            if !event::poll(timeout).map_err(rt_err)? {
                return Ok(Value::Nil);
            }
            let ev = event::read().map_err(rt_err)?;
            if let Event::Key(key) = ev {
                if key.kind == KeyEventKind::Press {
                    return Ok(Value::Table(key_event_table(lua, &key)?));
                }
            }
            Ok(Value::Nil)
        });

        methods.add_method_mut("finish", |_, this, ()| {
            this.shutdown_terminal();
            Ok(())
        });

        methods.add_method_mut("read_line", |lua, this, ()| {
            this.render(lua)?;
            this.input.clear();

            loop {
                if event::poll(Duration::from_millis(50)).map_err(rt_err)? {
                    match event::read().map_err(rt_err)? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                            KeyCode::Char(c) => {
                                this.input.push(c);
                                this.render(lua)?;
                            }
                            KeyCode::Backspace => {
                                this.input.pop();
                                this.render(lua)?;
                            }
                            KeyCode::Enter => {
                                let line = this.input.clone();
                                this.input.clear();
                                this.render(lua)?;
                                return Ok(line);
                            }
                            KeyCode::Esc => {
                                this.input.clear();
                                this.render(lua)?;
                                return Ok(String::new());
                            }
                            _ => {}
                        },
                        Event::Resize(_, _) => {
                            this.render(lua)?;
                        }
                        _ => {}
                    }
                }
            }
        });
    }
}

pub fn create_module(lua: &Lua) -> Result<mlua::Table> {
    let module = lua.create_table()?;
    module.set("new", lua.create_function(|_, ()| Ok(BlessingState::new()))?)?;
    module.set("available", true)?;
    module.set("codename", "blessing")?;
    module.set("version", "0.2")?;
    Ok(module)
}
