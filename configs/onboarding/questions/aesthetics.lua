local themes = require("themes")

local M = {}

function M.run(ctx)
  ctx.interface:reset()
  local theme = ctx.interface:select("THEME", "what theme do you want to use?", themes.names(), {
    default_index = 1,
    direction = "vertical",
    preview = function(name)
      ctx.interface:set_theme(name)
    end,
  })
  ctx.user_data.theme = theme
end

return M
