local themes = require("themes")

local M = {}

function M.run(ctx)
  ctx.interface:reset()
  while true do
    local theme = ctx.interface:input("THEME", "what theme do you want to use?", {
      placeholder = "catppuccin-mocha",
      default = "catppuccin-mocha",
    })

    if themes.is_valid(theme) then
      ctx.user_data.theme = theme
      return
    end
  end
end

return M
