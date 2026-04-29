local provider_api = require("provider")

local M = {}

function M.run(ctx)
  ctx.interface:reset()
  local answer = ctx.interface:input("PROVIDERS", "what providers do you want to connect?", {
    placeholder = "OpenRouter and Anthropic",
  })

  ctx.answers.providers = answer
  ctx.interface:working("what providers do you want to connect?", "working...")
  local providers = provider_api.providers(answer, ctx.answers)
  ctx.user_data.providers = providers
  ctx.secrets.providers = {}

  for _, provider in ipairs(providers) do
    if provider.authentication == "api_key" then
      local key = ctx.interface:input("PROVIDER_" .. provider.id:gsub("%-", "_"):upper() .. "_API_KEY", provider.name .. " API key", {
        placeholder = "sk-...",
        secret = true,
      })
      if key ~= "" then
        ctx.secrets.providers[provider.id] = key
      end
    end
  end
end

return M
