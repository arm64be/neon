local neon = require("neon")

local M = {}

M.spec = {
  name = "bash",
  description = "Run a shell command. Destructive sudo rm patterns are refused with feedback.",
  parameters = {
    type = "object",
    properties = {
      command = { type = "string", description = "Command to run with bash -lc." },
    },
    required = { "command" },
    additionalProperties = false,
  },
}

function M.run(args)
  local command = args.command or ""
  if command:match("sudo%s+rm") then
    return "blocked: sudo rm is not allowed from the bash tool. Explain why the operation is needed and ask the user for a safer command."
  end
  return neon.tools.bash(command)
end

return M
