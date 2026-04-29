local M = {}

function M.run(ctx)
  ctx.interface:reset()
  local choice = ctx.interface:select("CUSTOM_INSTRUCTIONS", "any custom instructions for the system prompt?", {
    "no",
    "yes",
  })

  if choice == "yes" then
    ctx.user_data.instructions = ctx.interface:input("INSTRUCTIONS", "what should Neon keep in mind?", {
      placeholder = "Be concise.",
    })
  else
    ctx.user_data.instructions = ""
  end
end

return M
