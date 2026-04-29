local neon = require("neon")

local M = {}

M.base_url = neon.env_or("NEON_ONBOARDING_API_BASE", "https://example.com")
M.test_mode = neon.env_or("NEON_ONBOARDING_TEST_MODE", "0") == "1"

local function validate_provider(provider)
  if type(provider) ~= "table" then
    error("provider schema must be a table")
  end
  if not tostring(provider.id or ""):match("^[a-z%-]+$") then
    error("provider id must match [a-z-]+")
  end
  if provider.type ~= "openai" and provider.type ~= "anthropic" then
    error("provider type must be openai or anthropic")
  end
  if provider.authentication ~= "none" and provider.authentication ~= "api_key" then
    error("provider authentication must be none or api_key")
  end
  return provider
end

local function default_providers()
  return {
    {
      id = "openrouter",
      name = "OpenRouter",
      base_url = "https://openrouter.ai/api/v1",
      type = "openai",
      authentication = "api_key",
    },
  }
end

function M.providers(answer, answers)
  if M.test_mode then
    local raw = neon.env("NEON_ONBOARDING_PROVIDERS_JSON")
    if raw and raw ~= "" then
      local decoded = neon.json.decode(raw)
      for _, provider in ipairs(decoded) do
        validate_provider(provider)
      end
      return decoded
    end
    return default_providers()
  end

  local response = neon.tokio.http(
    "POST",
    M.base_url .. "/v1/onboarding/providers",
    { ["Content-Type"] = "application/json" },
    nil,
    {
      stage = "providers",
      answer = answer,
      answers = answers,
    }
  )

  if response.status >= 400 then
    local body = tostring(response.body or ""):gsub("%s+$", "")
    if body ~= "" then
      error(("provider onboarding failed with HTTP %d: %s"):format(response.status, body))
    end
    error(("provider onboarding failed with HTTP %d"):format(response.status))
  end

  local data = neon.json.decode(response.body)
  local providers = data.providers or data
  for _, provider in ipairs(providers) do
    validate_provider(provider)
  end
  return providers
end

return M
