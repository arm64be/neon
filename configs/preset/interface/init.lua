local M = {}

function M.new(opts)
  opts = opts or {}

  local ok_blessing, blessing = pcall(require, "blessing")
  if not ok_blessing or not blessing.available then
    return require("interface.plain").new(opts)
  end

  return require("interface.tui").new(opts, blessing)
end

return M
