local neon = require("neon")

local session = neon.new_session("chat")

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

local function oneshot_prompt()
  local prompt = neon.util.arg_value("prompt")
  if prompt and prompt ~= "" then
    return prompt
  end

  local tail = neon.util.arg_glob()
  if #tail > 0 then
    return table.concat(tail, " ")
  end

  io.write("prompt> ")
  io.flush()
  return io.read("*l") or ""
end

session:set_model(function(state)
  if not state.context.api_key or state.context.api_key == "" then
    error("missing OPENROUTER_API_KEY or OPENAI_API_KEY")
  end

  local response = neon.net.http(
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
      messages = build_messages(state.history),
      temperature = 0.2,
    }
  )

  if response.status >= 400 then
    error(("http %d: %s"):format(response.status, neon.util.trim_string(response.body)))
  end

  local data = neon.json.decode(response.body)
  local content = data.choices[1].message.content or ""
  return neon.util.trim_string(content)
end)

session:set_interface(function(state)
  print(("Neon session %s ready. Type /quit to exit."):format(state.name or "chat"))

  while true do
    io.write("you> ")
    io.flush()

    local line = io.read("*l")
    if not line or line == "/quit" then
      break
    end

    session:push("user", line)
    local reply = session:run(16)
    print("assistant> " .. neon.util.trim_string(reply))
  end
end)

if neon.util.arg_flag("oneshot") then
  session:push("user", oneshot_prompt())
  print(neon.util.trim_string(session:run(16)))
else
  session:run_interface()
end
