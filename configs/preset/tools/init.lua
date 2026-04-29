local M = {}

local bash = require("tools.bash")
local web_fetch = require("tools.web_fetch")
local web_text = require("tools.web_text")

M.modules = {
  bash,
  web_fetch,
  web_text,
}

function M.register(session)
  for _, tool in ipairs(M.modules) do
    session:add_tool(tool.spec, function(args)
      return tool.run(args)
    end)
  end
end

return M
