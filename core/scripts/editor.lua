local keys = require "keys"
local utils = require "utils"

local M = {}

local Editor = {}
Editor.__index = Editor

---@return Editor
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
            client = {
                index = 100,
                text = function ()
                    return "[" .. env.client .. "@" .. env.session .."]"
                end
            }
        },
        key_handler = keys.ModalHandler(client_id)
    }
end

---@param client_id integer
function Editor:remove_client(client_id)
    self.clients[client_id] = nil
end

function Editor:get_status_line(client_id)
    local status_line = {}
    for k, v in pairs(self.clients[client_id].status_line) do
        status_line[k] = {
            index = v.index,
            text = utils.string(v.text),
        }
    end
    return status_line
end

---@class Editor
M.Editor = Editor

return M