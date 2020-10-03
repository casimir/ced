local keys = require "keys"

local M = {}

local ClientContext = {}
ClientContext.__index = ClientContext

---@return ClientContext
function ClientContext.new()
    local self = setmetatable({}, ClientContext)
    self.view = {}
    self.selections = {}
    return self
end

local Editor = {}
Editor.__index = Editor

---@class ClientContext
M.ClientContext = ClientContext

---@return Editor
function Editor.new()
    local self = setmetatable({}, Editor)
    self.core = _CORE
    self.clients = {}
    return self
end

---@param content string
function Editor:debug(content) self.core:debug(content) end

---@param client_id integer
---@param content string
function Editor:message(client_id, content) self.core:message(client_id, content) end

---@param client_id integer
---@param content string
function Editor:error(client_id, content) self.core:error(client_id, content) end

function Editor:get_status_line(client_id)
    local status_line = {}
    for k, v in pairs(self.clients[client_id].status_line) do
        status_line[k] = {index = v.index, text = utils.asstring(v.text)}
    end
    return status_line
end

---@param client_id integer
---@return ClientContext
function Editor:get_context(client_id)
    local data = self.core:get_context(client_id)
    return setmetatable(data, ClientContext)
end

---@param client_id integer
---@param name string
---@param content boolean
function Editor:scratch(client_id, name, content)
    if content == nil then content = "" end
    self.core:scratch(client_id, name, content)
end

---@param client_id integer
---@param name string
---@param scratch boolean
function Editor:edit(client_id, name, scratch)
    if scratch == nil then scratch = false end
    self.core:edit(client_id, name, scratch)
end

function Editor:append_to(buffer, text) self.core:append_to(buffer, text) end

---@param client_id integer
function Editor:add_client(client_id)
    self.clients[client_id] = {
        status_line = {
            client = {
                index = 100,
                text = function()
                    return "[" .. env.client .. "@" .. env.session .. "]"
                end
            }
        },
        key_handler = keys.ModalHandler(client_id)
    }
end

---@param client_id integer
function Editor:remove_client(client_id) self.clients[client_id] = nil end

---@param client_id integer
---@param extend boolean
function Editor:move_left(client_id, extend)
    return self.core:move_left(client_id, extend)
end

---@param client_id integer
---@param extend boolean
function Editor:move_right(client_id, extend)
    return self.core:move_right(client_id, extend)
end

---@param client_id integer
---@param extend boolean
function Editor:move_up(client_id, extend)
    return self.core:move_up(client_id, extend)
end

---@param client_id integer
---@param extend boolean
function Editor:move_down(client_id, extend)
    return self.core:move_down(client_id, extend)
end

---@param client_id integer
---@param extend boolean
function Editor:move_to_line_begin(client_id, extend)
    return self.core:move_to_line_begin(client_id, extend)
end

---@param client_id integer
---@param extend boolean
function Editor:move_to_line_end(client_id, extend)
    return self.core:move_to_line_end(client_id, extend)
end

---@param client_id integer
---@param extend boolean
function Editor:move_to_begin(client_id, extend)
    return self.core:move_to_begin(client_id, extend)
end

---@param client_id integer
---@param extend boolean
function Editor:move_to_end(client_id, extend)
    return self.core:move_to_end(client_id, extend)
end

---@class Editor
M.Editor = Editor

return M
