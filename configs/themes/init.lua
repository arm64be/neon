local M = {}

M.catalog = {
  ["catppuccin-mocha"] = require("themes.catppuccin_mocha"),
  ["tokyo-night"] = require("themes.tokyo_night"),
}

function M.names()
  local names = {}
  for name, _ in pairs(M.catalog) do
    names[#names + 1] = name
  end
  table.sort(names)
  return names
end

function M.get(name)
  return M.catalog[name]
end

function M.require(name)
  local theme = M.get(name)
  if not theme then
    error("unknown theme: " .. tostring(name))
  end
  return theme
end

function M.is_valid(name)
  return M.catalog[name] ~= nil
end

return M
