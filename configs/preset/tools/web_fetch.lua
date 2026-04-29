local neon = require("neon")

local M = {}

M.spec = {
  name = "web_fetch",
  description = "Fetch a URL and return status, headers, and body text.",
  parameters = {
    type = "object",
    properties = {
      url = { type = "string", description = "HTTP or HTTPS URL to fetch." },
    },
    required = { "url" },
    additionalProperties = false,
  },
}

function M.run(args)
  return neon.tokio.http("GET", args.url, nil, nil, nil)
end

return M
