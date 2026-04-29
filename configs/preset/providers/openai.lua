local neon = require("neon")

local M = {}

function M.request(provider, state, messages)
  local response = neon.tokio.http(
    "POST",
    provider.base_url .. "/chat/completions",
    {
      Authorization = "Bearer " .. (provider.api_key or ""),
      ["Content-Type"] = "application/json",
      ["HTTP-Referer"] = state.context.site_url or "http://localhost",
      ["X-Title"] = state.context.site_name or "Neon",
    },
    nil,
    {
      model = state.context.model or provider.model,
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

return M
