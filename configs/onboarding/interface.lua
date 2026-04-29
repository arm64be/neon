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

local function is_abort_key(key)
  return key and key.ctrl and (key.char == "c" or key.char == "d" or key.name == "c" or key.name == "d")
end

local function wrapped_lines(text, width)
  width = math.max(1, width or 1)
  text = tostring(text or "")
  local lines = 0
  for line in (text .. "\n"):gmatch("(.-)\n") do
    lines = lines + math.max(1, math.ceil(#line / width))
  end
  return math.max(1, lines)
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
    local content_width = 48
    local visible_input = input
    local input_len = #input
    local input_style = { fg = ui.theme.ui.text, bold = true }

    if input == "" then
      visible_input = placeholder or ""
      input_style = { fg = ui.theme.ui.subtle }
    elseif secret then
      visible_input = string.rep("*", #input)
    end

    local history_lines = {}
    local history_height = 0
    for i = math.max(1, #ui.history - 2), #ui.history do
      local age = #ui.history - i + 1
      local prefix = string.rep("·", age)
      history_lines[#history_lines + 1] = prefix .. " " .. ui.history[i].question
      history_lines[#history_lines + 1] = "  " .. tostring(ui.history[i].answer)
    end
    local history_text = table.concat(history_lines, "\n")
    for _, line in ipairs(history_lines) do
      history_height = history_height + wrapped_lines(line, content_width)
    end
    if #history_lines == 0 then
      history_height = 0
    end

    local question_text = "· " .. question
    local question_height = wrapped_lines(question_text, content_width)
    local input_text = options and table.concat(options, "   ") or visible_input
    local input_height = wrapped_lines(input_text, content_width)
    local spacer_height = history_height > 0 and 1 or 0
    local content_height = history_height + spacer_height + question_height + input_height
    local cursor_x = input_len % content_width
    local cursor_y = math.floor(input_len / content_width)

    return {
      id = "root",
      direction = "vertical",
      constraints = { "ratio:1:1", "length:" .. tostring(content_height), "ratio:1:1" },
      children = {
        { id = "top" },
        {
          id = "middle",
          direction = "horizontal",
          constraints = { "ratio:1:1", "length:48", "ratio:1:1" },
          children = {
            { id = "left" },
            {
              id = "content",
              direction = "vertical",
              constraints = {
                "length:" .. tostring(history_height),
                "length:" .. tostring(spacer_height),
                "length:" .. tostring(question_height),
                "length:" .. tostring(input_height),
              },
              children = {
                {
                  id = "history",
                  render = function(_ctx)
                    return {
                      kind = "paragraph",
                      text = history_text,
                      wrap = true,
                      style = { fg = ui.theme.ui.subtle, modifiers = { "dim" } },
                    }
                  end,
                },
                {
                  id = "spacer",
                  render = function(_ctx)
                    return {
                      kind = "paragraph",
                      text = "",
                      style = { fg = ui.theme.ui.subtle },
                    }
                  end,
                },
                {
                  id = "question",
                  render = function(_ctx)
                    return {
                      kind = "paragraph",
                      text = question_text,
                      wrap = true,
                      style = { fg = ui.theme.ui.text, bold = true },
                    }
                  end,
                },
                {
                  id = "input",
                  render = function(_ctx)
                    if options then
                      local segments = {}
                      for idx, option in ipairs(options) do
                        segments[#segments + 1] = {
                          text = option .. (idx < #options and "   " or ""),
                          style = idx == selected and {
                            fg = ui.theme.ui.accent,
                            bold = true,
                          } or {
                            fg = ui.theme.ui.subtle,
                          },
                        }
                      end

                      return {
                        kind = "inline",
                        segments = segments,
                        wrap = true,
                        style = { fg = ui.theme.ui.text },
                      }
                    end

                    return {
                      kind = "paragraph",
                      text = visible_input,
                      wrap = true,
                      style = input_style,
                      cursor = true,
                      cursor_x = cursor_x,
                      cursor_y = cursor_y,
                      cursor_offset_x = 0,
                      cursor_offset_y = 0,
                    }
                  end,
                },
              },
            },
            { id = "right" },
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
        if is_abort_key(key) then
          error("onboarding aborted", 0)
        end
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
        if is_abort_key(key) then
          error("onboarding aborted", 0)
        end
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
