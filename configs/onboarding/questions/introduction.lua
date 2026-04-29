local M = {}

local function username()
	local neon = require("neon")
	return neon.env("USER") or neon.env("USERNAME") or ""
end

local function bash_stdout(command)
	local neon = require("neon")
	local output = neon.tools.bash(command)
	local exit_code = output:match("^exit=(%d+)\n")
	if exit_code ~= "0" then
		return nil
	end

	local stdout = output:match("\nstdout:\n(.-)\nstderr:\n")
	if not stdout then
		return nil
	end

	stdout = stdout:gsub("%s+$", "")
	if stdout == "" then
		return nil
	end
	return stdout
end

local function display_name()
	local neon = require("neon")
	local user = username()
	local name = bash_stdout(([[
user=%s
if command -v getent >/dev/null 2>&1; then
  getent passwd "$user" | cut -d: -f5 | cut -d, -f1
elif command -v id >/dev/null 2>&1; then
  id -F 2>/dev/null
fi
]]):format(string.format("%q", user)))

	if name and neon.util.trim_string(name) ~= "" then
		return neon.util.trim_string(name)
	end
	return user
end

function M.run(ctx)
	local neon = require("neon")
	ctx.interface:reset()
	local name = display_name()
	local choice = ctx.interface:select("INTRODUCTION", "hello, " .. name, {
		"hi",
		"call me something else",
	})

	if choice == "call me something else" then
		local custom_name = ctx.interface:input("DISPLAY_NAME", "what should Neon call you?", {
			placeholder = name,
		})
		if neon.util.trim_string(custom_name) ~= "" then
			name = neon.util.trim_string(custom_name)
		end
	end

	ctx.answers.name = name
	ctx.user_data.name = name
end

return M
