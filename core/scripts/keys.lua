local M = {}

local ModalHandler = {}
ModalHandler.__index = ModalHandler

ModalHandler.modes = {
    normal = "N",
    insertion = "I"
}

setmetatable(
    ModalHandler,
    {
        __call = function(cls, ...)
            return cls.new(...)
        end
    }
)

function ModalHandler.new(client_id)
    local self = setmetatable({}, ModalHandler)
    self.client_id = client_id
    self.mode = ModalHandler.modes.normal
    self.redraw_status = false
    return self
end

function ModalHandler:set_status(key)
    local status_line = editor.clients[self.client_id].status_line
    if not status_line.keys then
        status_line.keys = {index = 80}
    end
    if not status_line.mode then
        status_line.mode = {index = 90}
    end
    status_line.keys.text = key and key.display or ""
    status_line.mode.text = self.mode
    editor.clients[self.client_id].status_line = status_line
    self.redraw_status = true
end

function ModalHandler:handle_normal(key)
    if key.value == "i" then
        self.mode = ModalHandler.modes.insertion
    elseif key.value == "m" then
        editor:message(self.client_id, "hello!")
    elseif key.value == "e" then
        editor:error(self.client_id, "oops!")
    end
end

function ModalHandler:handle_insertion(key)
    if key.value == "esc" then
        self.mode = ModalHandler.modes.normal
    end
end

function ModalHandler:handle(key)
    if self.mode == ModalHandler.modes.normal then
        self:handle_normal(key)
    elseif self.mode == ModalHandler.modes.insertion then
        self:handle_insertion(key)
    end
    self:set_status(key)
    local redraw_status = self.redraw_status
    self.redraw_status = false
    return {redraw_status = redraw_status}
end

M.ModalHandler = ModalHandler

return M
