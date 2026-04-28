local M = {}

function M.new(opts)
  opts = opts or {}

  local core = require("blessing").new()

  local state = {
    core = core,
    title = opts.title or "Neon / blessing",
    status = opts.status or "ready",
    prompt = opts.prompt or "you>",
    lines = {},
    max_lines = opts.max_lines or 250,
    tabs = { "Chat", "Tools", "Meta" },
    tab = 1,
  }

  local api = {}

  function api:trim_lines()
    while #self.lines > self.max_lines do
      table.remove(self.lines, 1)
    end
  end

  function api:progress_ratio()
    return math.min(1.0, (#self.lines % 100) / 100)
  end

  function api:layout()
    local ui = self
    return {
      id = "root",
      direction = "vertical",
      constraints = { "length:3", "min:4", "length:4", "length:3" },
      children = {
        {
          id = "header",
          render = function(ctx)
            return {
              {
                kind = "paragraph",
                text = ui.status .. "  [frame " .. tostring(ctx.frame) .. "]",
                style = { fg = "cyan", modifiers = { "bold" } },
                block = { title = ui.title, borders = "all" },
              },
            }
          end,
        },
        {
          id = "body",
          direction = "horizontal",
          constraints = { "ratio:3:1" },
          children = {
            {
              id = "transcript",
              render = function(_ctx)
                return {
                  kind = "paragraph",
                  text = table.concat(ui.lines, "\n"),
                  wrap = true,
                  block = { title = "Transcript", borders = "all" },
                }
              end,
            },
            {
              id = "side",
              direction = "vertical",
              constraints = { "length:3", "min:3", "length:3" },
              children = {
                {
                  render = function(_ctx)
                    return {
                      kind = "tabs",
                      titles = ui.tabs,
                      selected = ui.tab - 1,
                      block = { title = "Views", borders = "all" },
                      style = { fg = "yellow" },
                    }
                  end,
                },
                {
                  render = function(_ctx)
                    local tail = {}
                    for i = math.max(1, #ui.lines - 9), #ui.lines do
                      tail[#tail + 1] = ui.lines[i]
                    end
                    return {
                      kind = "list",
                      items = tail,
                      block = { title = "Recent", borders = "all" },
                    }
                  end,
                },
                {
                  render = function(_ctx)
                    return {
                      kind = "gauge",
                      ratio = ui:progress_ratio(),
                      label = "history",
                      block = { title = "Load", borders = "all" },
                      style = { fg = "light_green", modifiers = { "bold" } },
                    }
                  end,
                },
              },
            },
          },
        },
        {
          id = "footer",
          render = function(ctx)
            return {
              kind = "paragraph",
              text = (ui.prompt or "you>") .. " " .. (ctx.input or ""),
              block = { title = "Input", borders = "all" },
              style = { fg = "white" },
            }
          end,
        },
        {
          id = "hint",
          render = function(_ctx)
            return {
              kind = "paragraph",
              text = "Enter=submit Esc=empty /quit=exit",
              block = { title = "Keys", borders = { "top", "left", "right" } },
              style = { fg = "gray" },
            }
          end,
        },
      },
    }
  end

  function api:refresh()
    self.core:set_layout(self:layout())
    self.core:render()
  end

  function api:set_status(text)
    self.status = text
    self:refresh()
  end

  function api:set_title(text)
    self.title = text
    self:refresh()
  end

  function api:add_line(text)
    self.lines[#self.lines + 1] = text
    self:trim_lines()
    self:refresh()
  end

  function api:add_user(text)
    self:add_line("you> " .. text)
  end

  function api:add_assistant(text)
    self:add_line("assistant> " .. text)
  end

  function api:ask()
    self.core:set_layout(self:layout())
    local line = self.core:read_line()
    return line
  end

  function api:close()
    self.core:finish()
  end

  return setmetatable(state, { __index = api })
end

return M
