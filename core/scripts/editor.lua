local keys = require "keys"

local M = {}

local Editor = {}
Editor.__index = Editor
setmetatable(
    Editor,
    {
        __call = function(cls, ...)
            return cls.new(...)
        end
    }
)

function Editor.new()
    local self = setmetatable({}, Editor)
    self.core = _CORE
    self.clients = {}
    return self
end

---@param content string
function Editor:debug(content)
    self.core:debug(content)
end

---@param client_id integer
---@param content string
function Editor:message(client_id, content)
    self.core:message(client_id, content)
end

---@param client_id integer
---@param content string
function Editor:error(client_id, content)
    self.core:error(client_id, content)
end

---@param client_id integer
function Editor:add_client(client_id)
    self.clients[client_id] = {
        status_line = {
            client = {index = 100}
        },
        key_handler = keys.ModalHandler(client_id)
    }
end

---@param client_id integer
function Editor:remove_client(client_id)
    self.clients[client_id] = nil
end

M.Editor = Editor

return M