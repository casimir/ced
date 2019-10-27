local keys = {}

local PrintHandler = {}
PrintHandler.__index = PrintHandler

setmetatable(
    PrintHandler,
    {
        __call = function(cls, ...)
            return cls.new(...)
        end
    }
)

function PrintHandler.new(client_id)
    local self = setmetatable({}, PrintHandler)
    self.client_id = client_id
    return self
end

function PrintHandler:handle(key)
    if key == "m" then
        editor:message(self.client_id, "hello!")
    elseif key == "e" then
        editor:error(self.client_id, "oops!")
    end
    editor:debug("key (" .. self.client_id .. '): "' .. key .. '"')
    clients[self.client_id].status_line.keys = {index = 90, text = key}
    return {handled = true, redraw_status = true}
end

keys.PrintHandler = PrintHandler

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
    return self
end

function ModalHandler:handle(key)
    if key == "i" then
        self.mode = ModalHandler.modes.insertion
    elseif key == "n" then
        self.mode = ModalHandler.modes.normal
    elseif key == "m" then
        editor:message(self.client_id, "hello!")
    elseif key == "e" then
        editor:error(self.client_id, "oops!")
    else
        return {handled = false, redraw_status = false}
    end
    return {handled = true, redraw_status = false}
end

keys.ModalHandler = ModalHandler

return keys
