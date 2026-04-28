local M = {}

local function paragraph(text, title, style)
  return {
    kind = "paragraph",
    text = text or "",
    wrap = true,
    style = style,
    block = {
      title = title,
      borders = "all",
    },
  }
end

function M.new(opts)
  opts = opts or {}

  local core = require("blessing").new()

  local state = {
    core = core,
    title = opts.title or "Neon / blessing",
    status = opts.status or "ready",
    prompt = opts.prompt or "you>",
    input = "",
    lines = {},
    max_lines = opts.max_lines or 250,
  }

  local api = {}

  function api:trim_lines()
    while #self.lines > self.max_lines do
      table.remove(self.lines, 1)
    end
  end

  function api:layout()
    local ui = self

    return {
      direction = "vertical",
      constraints = { "length:3", "min:4", "length:3" },
      children = {
        {
          render = function(_ctx)
            return paragraph(ui.status, ui.title, { fg = "cyan", bold = true })
          end,
        },
        {
          render = function(_ctx)
            return paragraph(table.concat(ui.lines, "\n"), "Transcript")
          end,
        },
        {
          render = function(ctx)
            local text = (ui.prompt or "you>") .. " " .. (ctx.input or "")
            return paragraph(text, "Input", { fg = "white" })
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
    self.input = ""
    return line
  end

  function api:close()
    self.core:finish()
  end

  return setmetatable(state, { __index = api })
end

return M
