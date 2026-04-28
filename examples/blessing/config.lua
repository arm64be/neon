local neon = require("neon")

local ok_blessing, blessing_mod = pcall(require, "blessing")
if not ok_blessing then
  error("blessing module not available in this build; recompile with default features or --features blessing")
end

local ui = require("blessing_ui").new({
  title = "Neon / blessing",
  status = "ready",
  prompt = "you>",
  max_lines = 250,
})

neon.set_session_db(neon.config_root .. "/sessions.sqlite3")

local session_name = neon.util.arg_value("resume") or neon.util.arg_value("session")
local session = neon.new_session(session_name)

session:context().model = neon.env_or("OPENROUTER_MODEL", "arcee-ai/trinity-mini")
session:context().api_base = neon.env_or("OPENROUTER_API_BASE", "https://openrouter.ai/api/v1")
session:context().api_key = neon.env("OPENROUTER_API_KEY") or neon.env("OPENAI_API_KEY") or ""
session:context().site_url = neon.env_or("OPENROUTER_SITE_URL", "http://localhost")
session:context().site_name = neon.env_or("OPENROUTER_SITE_NAME", "Neon")
session:context().system_prompt = "You are a concise terminal assistant."

local function build_messages(history)
  local messages = {}

  if session:context().system_prompt ~= "" then
    messages[#messages + 1] = {
      role = "system",
      content = session:context().system_prompt,
    }
  end

  for _, message in ipairs(history) do
    if message.role == "user" or message.role == "assistant" then
      messages[#messages + 1] = {
        role = message.role,
        content = message.content,
      }
    end
  end

  return messages
end

local function tool_result_text(value)
  if value == nil then
    return ""
  end

  if type(value) == "string" then
    return value
  end

  local ok, encoded = pcall(neon.json.encode, value)
  if ok then
    return encoded
  end

  return tostring(value)
end

local function request_completion(state, messages)
  local response = neon.tokio.http(
    "POST",
    state.context.api_base .. "/chat/completions",
    {
      Authorization = "Bearer " .. (state.context.api_key or ""),
      ["Content-Type"] = "application/json",
      ["HTTP-Referer"] = state.context.site_url or "http://localhost",
      ["X-Title"] = state.context.site_name or "Neon",
    },
    nil,
    {
      model = state.context.model or "arcee-ai/trinity-mini",
      messages = messages,
      tools = state.tools,
      tool_choice = "auto",
      temperature = 0.2,
    }
  )

  if response.status >= 400 then
    error(("http %d: %s"):format(response.status, neon.util.trim_string(response.body)))
  end

  local data = neon.json.decode(response.body)
  local choice = data.choices and data.choices[1]
  if not choice or not choice.message then
    error("missing assistant message in completion response")
  end

  return choice.message
end

local function resolve_tool_calls(state, messages, message)
  local tool_calls = message.tool_calls or {}
  if #tool_calls == 0 then
    return message
  end

  local assistant = {
    role = "assistant",
  }
  if message.content and message.content ~= "" then
    assistant.content = message.content
  end
  assistant.tool_calls = tool_calls
  messages[#messages + 1] = assistant

  for _, tool_call in ipairs(tool_calls) do
    local call = tool_call["function"] or tool_call.function_call or {}
    local call_name = call.name or tool_call.name or (tool_call.tool and tool_call.tool.name)
    if not call_name or call_name == "" then
      error("tool call missing name: " .. tool_result_text(tool_call))
    end

    local raw_args = call.arguments or tool_call.arguments or "{}"
    local args
    if type(raw_args) == "string" then
      args = raw_args == "" and {} or neon.json.decode(raw_args)
    elseif type(raw_args) == "table" then
      args = raw_args
    else
      args = {}
    end

    ui:set_status("tool> " .. call_name)
    local ok, result = pcall(function()
      return session:call_tool(call_name, args)
    end)
    if not ok then
      error(("tool `%s` failed for call %s: %s"):format(call_name, tool_result_text(tool_call), result))
    end
    messages[#messages + 1] = {
      role = "tool",
      tool_call_id = tool_call.id,
      content = tool_result_text(result),
    }
  end

  return nil
end

local function compact_error(err)
  local text = tostring(err)
  local runtime = text:match("runtime error: ([^\n]+)")
  if runtime and runtime ~= "" then
    return runtime
  end

  local first_line = text:match("([^\n]+)")
  if first_line and first_line ~= "" then
    return first_line
  end

  return "unknown error"
end

local function main()
  if not blessing_mod.available then
    error("blessing module is present but marked unavailable")
  end

  session:set_model(function(state)
    if not state.context.api_key or state.context.api_key == "" then
      error("missing OPENROUTER_API_KEY or OPENAI_API_KEY")
    end

    local messages = build_messages(state.history)
    while true do
      local message = request_completion(state, messages)
      if message.tool_calls and #message.tool_calls > 0 then
        resolve_tool_calls(state, messages, message)
      else
        return neon.util.trim_string(message.content or message.refusal or "")
      end
    end
  end)

  session:set_interface(function(_state)
    ui:add_line(("session %s ready; press Enter to submit, Esc sends empty line."):format(session_name or "chat"))
    while true do
      ui:set_status("waiting for input")
      local line = ui:ask()
      if not line or line == "" or line == "/quit" then
        break
      end

      ui:add_user(line)
      session:push("user", line)
      ui:set_status("thinking")
      local ok, reply = pcall(session.run, session)
      if not ok then
        error(compact_error(reply), 0)
      end
      ui:add_assistant(neon.util.trim_string(reply))
      ui:set_status("ready")
    end
  end)

  local ok, err = pcall(session.run_interface, session)
  ui:close()
  if not ok then
    error(compact_error(err), 0)
  end
end

local ok, err = xpcall(main, compact_error)
if not ok then
  io.stderr:write(err, "\n")
  os.exit(1)
end
