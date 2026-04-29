local M = {}

function M.new(opts, blessing)
  opts = opts or {}
  local theme = opts.theme
  local core = blessing.new()
  local state = {
    core = core,
    session = opts.session,
    name = opts.name or "default",
    theme = theme,
    lines = {},
    status = "ready",
  }
  local api = {}

  function api:layout()
    local ui = self
    return {
      direction = "vertical",
      constraints = { "length:3", "min:4", "length:3" },
      children = {
        {
          render = function(ctx)
            return {
              kind = "paragraph",
              text = ui.status .. "  [frame " .. tostring(ctx.frame) .. "]",
              style = { fg = ui.theme.ui.accent, modifiers = { "bold" } },
              block = { title = "Neon", borders = "all" },
            }
          end,
        },
        {
          render = function(_ctx)
            return {
              kind = "paragraph",
              text = table.concat(ui.lines, "\n"),
              wrap = true,
              block = { title = ui.name, borders = "all" },
              style = { fg = ui.theme.ui.text },
            }
          end,
        },
        {
          render = function(ctx)
            return {
              kind = "paragraph",
              text = "you> " .. (ctx.input or ""),
              block = { title = "Input", borders = "all" },
              style = { fg = ui.theme.ui.text, bg = ui.theme.ui.input },
            }
          end,
        },
      },
    }
  end

  function api:add(line)
    self.lines[#self.lines + 1] = line
    while #self.lines > 250 do
      table.remove(self.lines, 1)
    end
  end

  function api:render()
    self.core:set_layout(self:layout())
    self.core:render()
  end

  function api:run()
    self:add("session " .. self.name .. " ready; /quit exits.")
    while true do
      self.status = "waiting"
      self:render()
      local line = self.core:read_line()
      if not line or line == "" or line == "/quit" then
        self.core:finish()
        return
      end
      self:add("you> " .. line)
      self.session:push("user", line)
      self.status = "thinking"
      self:render()
      local ok, reply = pcall(self.session.run, self.session)
      if not ok then
        self.core:finish()
        error(reply, 0)
      end
      self:add("assistant> " .. reply)
    end
  end

  return setmetatable(state, { __index = api })
end

return M
