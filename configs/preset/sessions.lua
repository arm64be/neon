local neon = require("neon")
local sqlite = require("sqlite")

local M = {}

function M.new(name)
  local db = sqlite.connect(neon.config_root .. "/sessions.sqlite3")
  neon.set_session_db(db)
  return neon.new_session(name or "default")
end

return M
