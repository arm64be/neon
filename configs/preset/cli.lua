local neon = require("neon")

local M = {}

function M.parse()
  return {
    help = neon.util.arg_flag("help") or neon.util.arg_flag("h"),
    oneshot = neon.util.arg_flag("oneshot"),
    prompt = neon.util.arg_value("prompt"),
    session_name = neon.util.arg_value("resume") or neon.util.arg_value("session") or "default",
    debug = neon.env_or("NEON_DEBUG", "0") == "1" or neon.util.arg_flag("debug"),
  }
end

function M.oneshot_prompt(args)
  if args.prompt and args.prompt ~= "" then
    return args.prompt
  end

  local tail = neon.util.arg_glob()
  if #tail > 0 then
    return table.concat(tail, " ")
  end

  error("oneshot mode requires --prompt=<text> or positional args")
end

function M.print_help()
  print("neon preset")
  print("  --help                 Show this help")
  print("  --oneshot --prompt=... Run a single prompt")
  print("  --resume=<name>        Resume a named session")
  print("  --debug                Print tool call previews")
end

return M
