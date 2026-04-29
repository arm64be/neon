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

local function text_len(value)
  local s = tostring(value or "")
  local ok, count = pcall(utf8.len, s)
  if ok and count then
    return count
  end
  return #s
end

local function wrapped_lines(text, width)
  width = math.max(1, width or 1)
  text = tostring(text or "")
  local lines = 0
  local has_content = false
  for line in (text .. "\n"):gmatch("(.-)\n") do
    if line:find("%S") then
      has_content = true
      lines = lines + math.ceil(text_len(line) / width)
    end
  end
  if not has_content then
    return 0
  end
  return lines
end

local function max_line_width(lines)
  local width = 0
  for _, line in ipairs(lines) do
    local value = tostring(line or "")
    if value:find("%S") then
      width = math.max(width, text_len(value))
    end
  end
  return width
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

  function api:set_theme(name)
    self.theme = themes.require(name)
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

  function api:layout(question, input, placeholder, options, selected, secret, opts)
    local ui = self
    opts = opts or {}
    local max_content_width = 48
    local term_width = 80
    local term_height = 24
    local visible_input = input
    local input_len = text_len(input)
    local input_style = { fg = ui.theme.ui.text, bold = true }

    if ui.core then
      local ok_size, width, height = pcall(ui.core.size, ui.core)
      if ok_size then
        term_width = tonumber(width) or term_width
        term_height = tonumber(height) or term_height
      end
    end

    if input == "" then
      visible_input = placeholder or ""
      input_style = { fg = ui.theme.ui.subtle }
    elseif secret then
      visible_input = string.rep("*", #input)
    end

    local history_lines = {}
    for i = math.max(1, #ui.history - 2), #ui.history do
      local age = #ui.history - i + 1
      local prefix = string.rep("·", age)
      history_lines[#history_lines + 1] = prefix .. " " .. ui.history[i].question
      local answer = tostring(ui.history[i].answer or "")
      if answer:find("%S") then
        history_lines[#history_lines + 1] = "  " .. answer
      end
    end
    local history_text = table.concat(history_lines, "\n")

    local question_text = "· " .. question
    local vertical_options = opts.direction == "vertical"
    local input_text
    if options then
      if vertical_options then
        local option_lines = {}
        for _, option in ipairs(options) do
          option_lines[#option_lines + 1] = option
        end
        input_text = table.concat(option_lines, "\n")
      else
        input_text = table.concat(options, "   ")
      end
    else
      input_text = visible_input
    end
    local content_width = math.min(max_content_width, math.max(
      1,
      max_line_width(history_lines),
      text_len(question_text),
      text_len(input_text)
    ))

    local history_height = 0
    for _, line in ipairs(history_lines) do
      history_height = history_height + wrapped_lines(line, content_width)
    end
    if #history_lines == 0 then
      history_height = 0
    end

    local question_height = math.max(1, wrapped_lines(question_text, content_width))
    local input_height = vertical_options and #options or math.max(1, wrapped_lines(input_text, content_width))
    local between_history_height = history_height > 0 and 1 or 0
    local content_height = history_height + between_history_height + question_height + input_height
    local left_width = math.max(0, math.floor((term_width - content_width) / 2))
    local right_width = math.max(0, term_width - left_width - content_width)
    local top_height = math.max(0, math.floor((term_height - content_height) / 2))
    local bottom_height = math.max(0, term_height - top_height - content_height)
    local cursor_x = input_len % content_width
    local cursor_y = math.floor(input_len / content_width)
    local show_cursor = opts.show_cursor ~= false and not options

    local content_constraints = {}
    local content_children = {}

    if history_height > 0 then
      content_constraints[#content_constraints + 1] = "length:" .. tostring(history_height)
      content_children[#content_children + 1] = {
        id = "history",
        render = function(_ctx)
          return {
            kind = "paragraph",
            text = history_text,
            wrap = true,
            style = {
              fg = ui.theme.ui.muted,
              bg = ui.theme.ui.background,
              modifiers = { "dim" },
            },
          }
        end,
      }

      content_constraints[#content_constraints + 1] = "length:1"
      content_children[#content_children + 1] = {
        id = "history_gap",
        render = function(_ctx)
          return {
            kind = "paragraph",
            text = "",
            wrap = false,
            style = { bg = ui.theme.ui.background },
          }
        end,
      }
    end

    content_constraints[#content_constraints + 1] = "length:" .. tostring(question_height)
    content_children[#content_children + 1] = {
      id = "question",
      render = function(_ctx)
        return {
          kind = "paragraph",
          text = question_text,
          wrap = true,
          style = {
            fg = ui.theme.ui.text,
            bg = ui.theme.ui.background,
            bold = true,
          },
        }
      end,
    }

    local input_child
    if vertical_options then
      local option_constraints = {}
      local option_children = {}
      for idx, option in ipairs(options) do
        option_constraints[#option_constraints + 1] = "length:1"
        option_children[#option_children + 1] = {
          id = "option_" .. tostring(idx),
          render = function(_ctx)
            return {
              kind = "paragraph",
              text = option,
              wrap = false,
              style = idx == selected and {
                fg = ui.theme.ui.accent,
                bg = ui.theme.ui.background,
                bold = true,
              } or {
                fg = ui.theme.ui.subtle,
                bg = ui.theme.ui.background,
              },
            }
          end,
        }
      end

      input_child = {
        id = "input",
        direction = "vertical",
        constraints = option_constraints,
        children = option_children,
      }
    else
      input_child = {
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
              style = { fg = ui.theme.ui.text, bg = ui.theme.ui.background },
            }
          end

          return {
            kind = "paragraph",
            text = visible_input,
            wrap = true,
            style = {
              fg = input_style.fg,
              bg = ui.theme.ui.background,
              bold = input_style.bold,
            },
            cursor = show_cursor,
            cursor_x = cursor_x,
            cursor_y = cursor_y,
            cursor_offset_x = 0,
            cursor_offset_y = 0,
          }
        end,
      }
    end

    content_constraints[#content_constraints + 1] = "length:" .. tostring(input_height)
    content_children[#content_children + 1] = input_child

    return {
      id = "root",
      render = function(_ctx)
        return {
          kind = "paragraph",
          text = "",
          wrap = false,
          clear = true,
          style = { bg = ui.theme.ui.background },
        }
      end,
      direction = "vertical",
      constraints = {
        "length:" .. tostring(top_height),
        "length:" .. tostring(content_height),
        "length:" .. tostring(bottom_height),
      },
      children = {
        { id = "top" },
        {
          id = "middle",
          direction = "horizontal",
          constraints = {
            "length:" .. tostring(left_width),
            "length:" .. tostring(content_width),
            "length:" .. tostring(right_width),
          },
          children = {
            { id = "left" },
            {
              id = "content",
              direction = "vertical",
              constraints = content_constraints,
              children = content_children,
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

  function api:working(question, message)
    message = message or "working..."
    if not self.core then
      io.write(message .. "\n")
      io.flush()
      return
    end

    self.core:set_input("")
    self.core:set_layout(self:layout(message, "", "", nil, nil, false, { show_cursor = false }))
    self.core:render()
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

    if opts.preview then
      opts.preview(options[selected], selected)
    end

    while true do
      self.core:set_layout(self:layout(question, "", nil, options, selected, false, opts))
      self.core:render()
      local key = self.core:read_key(100)
      if key then
        if is_abort_key(key) then
          error("onboarding aborted", 0)
        end
        if key.name == "right" or key.name == "tab" or key.char == "l" or key.name == "down" or key.char == "j" then
          selected = clamp_index(selected + 1, options)
          if opts.preview then
            opts.preview(options[selected], selected)
          end
        elseif key.name == "left" or key.char == "h" or key.name == "up" or key.char == "k" then
          selected = clamp_index(selected - 1, options)
          if opts.preview then
            opts.preview(options[selected], selected)
          end
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
