local neon = require("neon")

local M = {}

local function trim_error(err)
  local text = tostring(err)
  return text:match("runtime error: ([^\n]+)") or text:match("([^\n]+)") or "unknown error"
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

function M.build_messages(session, history)
  local messages = {}
  local system_prompt = session:context().system_prompt

  if system_prompt and system_prompt ~= "" then
    messages[#messages + 1] = {
      role = "system",
      content = system_prompt,
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

function M.resolve_tool_calls(session, state, messages, message)
  local tool_calls = message.tool_calls or {}
  if #tool_calls == 0 then
    return message
  end

  local assistant = { role = "assistant" }
  if message.content and message.content ~= "" then
    assistant.content = message.content
  end
  assistant.tool_calls = tool_calls
  messages[#messages + 1] = assistant

  for _, tool_call in ipairs(tool_calls) do
    local call = tool_call["function"] or tool_call.function_call or {}
    local call_name = call.name or tool_call.name or (tool_call.tool and tool_call.tool.name)
    local raw_args = call.arguments or tool_call.arguments or "{}"
    local args = type(raw_args) == "string" and neon.json.decode(raw_args == "" and "{}" or raw_args) or raw_args

    if state.context.debug then
      print("tool> " .. tostring(call_name))
    end

    local ok, result = pcall(function()
      return session:call_tool(call_name, args or {})
    end)
    messages[#messages + 1] = {
      role = "tool",
      tool_call_id = tool_call.id,
      content = ok and tool_result_text(result) or ("tool error: " .. trim_error(result)),
    }
  end

  return nil
end

function M.install(session, providers)
  session:set_model(function(state)
    local provider = providers[1]
    if not provider then
      error("no provider configured")
    end
    if provider.authentication == "api_key" and provider.api_key == "" then
      error("missing API key for provider " .. provider.id)
    end

    local messages = M.build_messages(session, state.history)
    while true do
      local message = provider.adapter.request(provider, state, messages)
      if message.tool_calls and #message.tool_calls > 0 then
        M.resolve_tool_calls(session, state, messages, message)
      else
        return neon.util.trim_string(message.content or message.refusal or "")
      end
    end
  end)
end

return M
