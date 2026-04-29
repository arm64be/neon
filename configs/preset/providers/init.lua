local neon = require("neon")

local M = {}

local ok_selected, selected = pcall(require, "providers.selected")
if not ok_selected then
  selected = {
    {
      id = "openrouter",
      name = "OpenRouter",
      base_url = "https://openrouter.ai/api/v1",
      type = "openai",
      authentication = "api_key",
    },
  }
end

function M.load()
  local providers = {}
  for _, provider in ipairs(selected) do
    local env_key = "NEON_PROVIDER_" .. provider.id:gsub("%-", "_"):upper() .. "_API_KEY"
    provider.api_key = neon.env(env_key) or neon.env("OPENROUTER_API_KEY") or neon.env("OPENAI_API_KEY") or ""
    provider.model = neon.env_or("OPENROUTER_MODEL", "arcee-ai/trinity-mini")
    provider.adapter = require("providers." .. provider.type)
    providers[#providers + 1] = provider
  end
  return providers
end

return M
