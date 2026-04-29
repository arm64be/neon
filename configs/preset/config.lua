local neon = require("neon")

package.path = table.concat({
  neon.config_root .. "/?.lua",
  neon.config_root .. "/?/init.lua",
  neon.config_root .. "/themes/?.lua",
  neon.config_root .. "/themes/?/init.lua",
  neon.config_root .. "/../?.lua",
  neon.config_root .. "/../?/init.lua",
  package.path,
}, ";")

local themes = require("themes")
local cli = require("cli")
local sessions = require("sessions")
local providers = require("providers")
local tools = require("tools")
local agent = require("agent")
local interface = require("interface")

local ok_user_data, user_data = pcall(require, "user_data")
if not ok_user_data then
  user_data = {
    name = "there",
    theme = "catppuccin-mocha",
    instructions = "",
  }
end

local args = cli.parse()
if args.help then
  cli.print_help()
  return
end

local theme = themes.require(user_data.theme or "catppuccin-mocha")
local session = sessions.new(args.session_name)
local loaded_providers = providers.load()

session:context().site_url = neon.env_or("OPENROUTER_SITE_URL", "http://localhost")
session:context().site_name = "Neon"
session:context().model = neon.env_or("OPENROUTER_MODEL", "arcee-ai/trinity-mini")
session:context().debug = args.debug
session:context().system_prompt = table.concat({
  "You are Neon, a concise terminal-based agentic coding assistant.",
  "Prefer small, verifiable steps. Use tools when they reduce uncertainty.",
  user_data.instructions or "",
}, "\n")

tools.register(session)
agent.install(session, loaded_providers)

if args.oneshot then
  session:push("user", cli.oneshot_prompt(args))
  local ok, reply = pcall(session.run, session)
  if not ok then
    error(reply, 0)
  end
  print(neon.util.trim_string(reply))
else
  session:set_interface(function(_state)
    interface.new({
      session = session,
      name = args.session_name,
      theme = theme,
    }):run()
  end)
  session:run_interface()
end
