local neon = require("neon")

local M = {}

M.spec = {
  name = "web_text",
  description = "Render a URL to plain text with w3m -dump when w3m is installed.",
  parameters = {
    type = "object",
    properties = {
      url = { type = "string", description = "HTTP or HTTPS URL to render." },
    },
    required = { "url" },
    additionalProperties = false,
  },
}

function M.run(args)
  local quoted = "'" .. tostring(args.url):gsub("'", "'\\''") .. "'"
  local output = neon.tools.bash("command -v w3m >/dev/null 2>&1 && w3m -dump " .. quoted)
  if output:match("^exit=0\n") then
    return output
  end
  return "w3m is not installed or failed to render the URL; use web_fetch for raw content."
end

return M
