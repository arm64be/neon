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

local interface = require("interface")

local ctx = {
  interface = interface.new({ theme = "catppuccin-mocha" }),
  answers = {},
  user_data = {},
  secrets = {},
}

local repo_root = neon.env_or("NEON_REPOSITORY_ROOT", ".")
local preset_source_override = neon.env("NEON_ONBOARDING_PRESET_SOURCE")

local function shell_quote(value)
  return "'" .. tostring(value):gsub("'", "'\\''") .. "'"
end

local function run_checked(command)
  local output = neon.tools.bash(command)
  if not output:match("^exit=0\n") then
    error(output)
  end
  return output
end

local function lua_quote(value)
  return string.format("%q", tostring(value or ""))
end

local function write_user_data(root)
  local lines = {
    "return {",
    "  name = " .. lua_quote(ctx.user_data.name) .. ",",
    "  theme = " .. lua_quote(ctx.user_data.theme or "catppuccin-mocha") .. ",",
    "  instructions = " .. lua_quote(ctx.user_data.instructions or "") .. ",",
    "}",
    "",
  }
  neon.tools.write_file(root .. "/user_data.lua", table.concat(lines, "\n"))
end

local function write_env(root)
  local lines = {}
  for _, provider in ipairs(ctx.user_data.providers or {}) do
    local key = ctx.secrets.providers and ctx.secrets.providers[provider.id]
    if key and key ~= "" then
      lines[#lines + 1] = "NEON_PROVIDER_" .. provider.id:gsub("%-", "_"):upper() .. "_API_KEY=" .. key
    end
  end
  neon.tools.write_file(root .. "/.env", table.concat(lines, "\n") .. "\n")
end

local function write_selected_providers(root)
  local lines = { "return {" }
  for _, provider in ipairs(ctx.user_data.providers or {}) do
    lines[#lines + 1] = "  {"
    lines[#lines + 1] = "    id = " .. lua_quote(provider.id) .. ","
    lines[#lines + 1] = "    name = " .. lua_quote(provider.name) .. ","
    lines[#lines + 1] = "    base_url = " .. lua_quote(provider.base_url) .. ","
    lines[#lines + 1] = "    type = " .. lua_quote(provider.type) .. ","
    lines[#lines + 1] = "    authentication = " .. lua_quote(provider.authentication) .. ","
    lines[#lines + 1] = "  },"
  end
  lines[#lines + 1] = "}"
  lines[#lines + 1] = ""
  neon.tools.write_file(root .. "/providers/selected.lua", table.concat(lines, "\n"))
end

local function install_preset()
  local target = neon.config_root
  local backup = target .. ".onboarding-backup"
  local preset_checkout = target .. ".preset-download"

  local fetch_command
  local preset_source
  local themes_source
  if preset_source_override and preset_source_override ~= "" then
    preset_source = preset_source_override
    themes_source = preset_source .. "/../themes"
    fetch_command = "test -d " .. shell_quote(preset_source)
  else
    preset_source = preset_checkout .. "/configs/preset"
    themes_source = preset_checkout .. "/configs/themes"
    fetch_command = table.concat({
      "rm -rf " .. shell_quote(preset_checkout),
      "mkdir -p " .. shell_quote(preset_checkout),
      "git -C " .. shell_quote(repo_root) .. " archive HEAD configs/preset configs/themes | tar -x -C " .. shell_quote(preset_checkout),
      "test -d " .. shell_quote(preset_source),
      "test -d " .. shell_quote(themes_source),
    }, " && ")
  end

  local command = table.concat({
    "set -e",
    fetch_command,
    "rm -rf " .. shell_quote(backup),
    "cp -R " .. shell_quote(target) .. " " .. shell_quote(backup),
    "find " .. shell_quote(target) .. " -mindepth 1 -maxdepth 1 -exec rm -rf {} +",
    "cp -R " .. shell_quote(preset_source) .. "/. " .. shell_quote(target) .. "/",
    "cp -R " .. shell_quote(themes_source) .. " " .. shell_quote(target) .. "/themes",
    "rm -rf " .. shell_quote(preset_checkout),
  }, " && ")

  run_checked(command)
  write_env(target)
  write_user_data(target)
  write_selected_providers(target)
  return true
end

local function main()
  require("questions.introduction").run(ctx)
  require("questions.providers").run(ctx)
  require("questions.personalization").run(ctx)
  require("questions.aesthetics").run(ctx)

  install_preset()
  print("Preset installed. Run Neon again with this config root to start.")
end

local ok, err = xpcall(main, function(error_value)
  ctx.interface:close()
  return tostring(error_value):match("runtime error: ([^\n]+)") or tostring(error_value)
end)
ctx.interface:close()

if not ok then
  io.stderr:write(err, "\n")
  os.exit(1)
end
