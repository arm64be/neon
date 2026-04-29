local themes = require("themes")

local M = {}

local function is_test()
  local neon = require("neon")
  return neon.env_or("NEON_ONBOARDING_TEST_MODE", "0") == "1"
end

local function clamp_index(index, items)
  if index < 1 then
    return #items
  end
  if index > #items then
    return 1
  end
  return index
end

local function option_line(option, selected)
  if selected then
    return "› " .. option
  end
  return "  " .. option
end

function M.new(opts)
  opts = opts or {}

  local ok_blessing, blessing = pcall(require, "blessing")
  local use_tui = ok_blessing and blessing.available and not is_test()
  local theme = themes.require(opts.theme or "catppuccin-mocha")

  local state = {
    core = use_tui and blessing.new() or nil,
    theme = theme,
    history = {},
  }

  local api = {}

  function api:close()
    if self.core then
      self.core:finish()
    end
  end

  function api:reset()
    self.history = {}
  end

  function api:push(question, answer, secret)
    self.history[#self.history + 1] = {
      question = question,
      answer = secret and "***" or answer,
    }
  end

  function api:test_answer(stage, default)
    local neon = require("neon")
    return neon.env_or("NEON_ONBOARDING_" .. stage, default or "")
  end

  function api:layout(question, input, placeholder, options, selected, secret)
    local ui = self
    return {
      id = "root",
      direction = "vertical",
      constraints = { "ratio:1:1", "length:9", "ratio:1:1" },
      children = {
        { id = "top" },
        {
          id = "center",
          direction = "vertical",
          constraints = { "min:1", "length:3", "length:3" },
          margin = 2,
          children = {
            {
              id = "history",
              render = function(_ctx)
                local lines = {}
                for i = math.max(1, #ui.history - 3), #ui.history do
                  local age = #ui.history - i + 1
                  local prefix = string.rep("·", age)
                  lines[#lines + 1] = prefix .. " " .. ui.history[i].question .. " " .. tostring(ui.history[i].answer)
                end
                return {
                  kind = "paragraph",
                  text = table.concat(lines, "\n"),
                  style = { fg = ui.theme.ui.muted, modifiers = { "dim" } },
                }
              end,
            },
            {
              id = "question",
              render = function(_ctx)
                return {
                  kind = "paragraph",
                  text = question,
                  wrap = true,
                  style = { fg = ui.theme.ui.text, modifiers = { "bold" } },
                }
              end,
            },
            {
              id = "input",
              render = function(_ctx)
                local text = input
                local style = { fg = ui.theme.ui.text, bg = ui.theme.ui.input }
                if input == "" then
                  text = placeholder or ""
                  style = { fg = ui.theme.ui.muted, bg = ui.theme.ui.input }
                elseif secret then
                  text = string.rep("*", math.min(24, #input))
                end

                if options then
                  local rows = {}
                  for idx, option in ipairs(options) do
                    rows[#rows + 1] = option_line(option, idx == selected)
                  end
                  text = table.concat(rows, "    ")
                end

                return {
                  kind = "paragraph",
                  text = text,
                  style = style,
                  block = { borders = "all" },
                }
              end,
            },
          },
        },
        { id = "bottom" },
      },
    }
  end

  function api:plain_input(stage, question, opts)
    opts = opts or {}
    if is_test() then
      return self:test_answer(stage, opts.default or "")
    end

    io.write(question)
    if opts.placeholder and opts.placeholder ~= "" then
      io.write(" [" .. opts.placeholder .. "]")
    end
    io.write("\n> ")
    io.flush()
    local value = io.read("*l") or ""
    if value == "" then
      value = opts.default or ""
    end
    return value
  end

  function api:input(stage, question, opts)
    opts = opts or {}
    if not self.core then
      local value = self:plain_input(stage, question, opts)
      self:push(question, value, opts.secret)
      return value
    end

    local value = opts.default or ""
    while true do
      self.core:set_input(value)
      self.core:set_layout(self:layout(question, value, opts.placeholder, nil, nil, opts.secret))
      self.core:render()
      local key = self.core:read_key(100)
      if key then
        if key.name == "enter" then
          if value == "" then
            value = opts.default or ""
          end
          self:push(question, value, opts.secret)
          return value
        elseif key.name == "backspace" then
          value = value:sub(1, -2)
        elseif key.name == "esc" then
          value = opts.default or ""
        elseif key.char then
          value = value .. key.char
        end
      end
    end
  end

  function api:select(stage, question, options, opts)
    opts = opts or {}
    local selected = opts.default_index or 1

    if not self.core then
      local raw = self:plain_input(stage, question, { default = options[selected], placeholder = options[selected] })
      for idx, option in ipairs(options) do
        if raw == option or raw == tostring(idx) then
          self:push(question, option, false)
          return option, idx
        end
      end
      self:push(question, options[selected], false)
      return options[selected], selected
    end

    while true do
      self.core:set_layout(self:layout(question, "", nil, options, selected, false))
      self.core:render()
      local key = self.core:read_key(100)
      if key then
        if key.name == "right" or key.name == "tab" or key.char == "l" then
          selected = clamp_index(selected + 1, options)
        elseif key.name == "left" or key.char == "h" then
          selected = clamp_index(selected - 1, options)
        elseif key.name == "enter" then
          self:push(question, options[selected], false)
          return options[selected], selected
        end
      end
    end
  end

  return setmetatable(state, { __index = api })
end

return M
