use std::{io::stdout, time::Duration};

use mlua::{Function, Lua, RegistryKey, Result, Table, UserData, UserDataMethods, Value};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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
}

impl BlessingState {
    fn new() -> Self {
        Self {
            input: String::new(),
            terminal: None,
            layout_key: None,
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

        let terminal = self
            .terminal
            .as_mut()
            .ok_or_else(|| rt_err("blessing terminal is unavailable"))?;

        terminal
            .draw(|frame| {
                let area = frame.area();
                if let Err(err) = render_node(lua, frame, root.clone(), area, &input) {
                    let _ = err;
                }
            })
            .map_err(rt_err)?;

        Ok(())
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

fn parse_constraints(table: Option<Table>) -> Result<Vec<Constraint>> {
    let Some(table) = table else {
        return Ok(vec![Constraint::Min(1)]);
    };

    let mut constraints = Vec::new();
    for value in table.sequence_values::<Value>() {
        match value? {
            Value::String(s) => {
                let text = s.to_str()?;
                let mut parts = text.splitn(2, ':');
                let kind = parts.next().unwrap_or_default();
                let raw = parts.next().unwrap_or("1").parse::<u16>().unwrap_or(1);
                let c = match kind {
                    "len" | "length" => Constraint::Length(raw),
                    "min" => Constraint::Min(raw),
                    "max" => Constraint::Max(raw),
                    "pct" | "percentage" => Constraint::Percentage(raw),
                    "ratio" => Constraint::Ratio(raw.max(1) as u32, 100),
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
        "white" => Some(Color::White),
        _ => None,
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
        _ => Borders::ALL,
    }
}

fn render_widget(
    frame: &mut ratatui::Frame,
    area: Rect,
    widget: Table,
) -> Result<()> {
    let kind: String = widget.get("kind").unwrap_or_else(|_| "paragraph".into());
    let block_spec: Option<Table> = widget.get("block").ok();
    let style = parse_style(widget.get("style").ok())?;

    if widget.get::<bool>("clear").unwrap_or(false) {
        frame.render_widget(Clear, area);
    }

    let block = if let Some(block_spec) = block_spec {
        let mut block = Block::default();
        if let Ok(title) = block_spec.get::<String>("title") {
            block = block.title(title);
        }
        block = block.borders(parse_borders(block_spec.get::<Value>("borders").ok()));
        Some(block)
    } else {
        None
    };

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
        _ => {}
    }

    Ok(())
}

fn render_node(
    lua: &Lua,
    frame: &mut ratatui::Frame,
    node: Table,
    area: Rect,
    input: &str,
) -> Result<()> {
    if let Ok(render_fn) = node.get::<Function>("render") {
        let ctx = lua.create_table()?;
        ctx.set("x", area.x)?;
        ctx.set("y", area.y)?;
        ctx.set("width", area.width)?;
        ctx.set("height", area.height)?;
        ctx.set("input", input)?;

        let widget_value: Value = render_fn.call(ctx)?;
        if let Value::Table(widget) = widget_value {
            render_widget(frame, area, widget)?;
        }
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
        render_node(lua, frame, child, chunk, input)?;
    }

    Ok(())
}

impl UserData for BlessingState {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("set_layout", |lua, this, table: Table| this.set_layout(lua, table));

        methods.add_method_mut("set_input", |_, this, input: String| {
            this.input = input;
            Ok(())
        });

        methods.add_method("input", |_, this, ()| Ok(this.input.clone()));

        methods.add_method_mut("render", |lua, this, ()| this.render(lua));

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
    Ok(module)
}
