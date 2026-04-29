local M = {}

function M.new(opts)
  opts = opts or {}
  local state = {
    session = opts.session,
    name = opts.name or "default",
  }
  local api = {}

  function api:run()
    print(("Neon session %s ready. Type /quit to exit."):format(self.name))
    while true do
      io.write("you> ")
      io.flush()
      local line = io.read("*l")
      if not line or line == "/quit" then
        break
      end
      self.session:push("user", line)
      local ok, reply = pcall(self.session.run, self.session)
      if not ok then
        error(reply, 0)
      end
      print("assistant> " .. reply)
    end
  end

  return setmetatable(state, { __index = api })
end

return M
