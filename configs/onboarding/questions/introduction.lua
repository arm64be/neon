local M = {}

local function username()
  local neon = require("neon")
  return neon.env("USER") or neon.env("USERNAME") or "there"
end

function M.run(ctx)
  ctx.interface:reset()
  local name = username()
  local choice = ctx.interface:select("INTRODUCTION", "hello, " .. name, {
    "hi",
    "call me something else",
  })

  if choice == "call me something else" then
    name = ctx.interface:input("DISPLAY_NAME", "what should Neon call you?", {
      placeholder = "Mel",
      default = name,
    })
  end

  ctx.answers.name = name
  ctx.user_data.name = name
end

return M
