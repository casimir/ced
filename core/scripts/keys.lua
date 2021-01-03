local M = {}

local ModalHandler = {}
ModalHandler.__index = ModalHandler

setmetatable(ModalHandler, {__call = function(cls, ...) return cls.new(...) end})

local function make_goto_mappings(extend)
    return {
        ["h"] = {
            fn = function(mh)
                mh:exit_mode()
                return editor:move_to_line_begin(mh.client_id, extend)
            end
        },
        ["l"] = {
            fn = function(mh)
                mh:exit_mode()
                return editor:move_to_line_end(mh.client_id, extend)
            end
        },
        default = function(mh, key) mh:exit_mode() end
    }
end

ModalHandler.modes = {
    normal = {
        name = "N",
        title = "",
        mappings = {
            ["i"] = {fn = function(mh) mh:enter_mode("insertion") end},
            ["g"] = {fn = function(mh) mh:enter_mode("moveto") end},
            ["s-g"] = {fn = function(mh) mh:enter_mode("extendto") end},
            ["h"] = {
                fn = function(mh)
                    return editor:move_left(mh.client_id, false)
                end
            },
            ["j"] = {
                fn = function(mh)
                    return editor:move_down(mh.client_id, false)
                end
            },
            ["k"] = {
                fn = function(mh)
                    return editor:move_up(mh.client_id, false)
                end
            },
            ["l"] = {
                fn = function(mh)
                    return editor:move_right(mh.client_id, false)
                end
            },
            ["s-h"] = {
                fn = function(mh)
                    return editor:move_left(mh.client_id, true)
                end
            },
            ["s-j"] = {
                fn = function(mh)
                    return editor:move_down(mh.client_id, true)
                end
            },
            ["s-k"] = {
                fn = function(mh)
                    return editor:move_up(mh.client_id, true)
                end
            },
            ["s-l"] = {
                fn = function(mh)
                    return editor:move_right(mh.client_id, true)
                end
            },
            ["m"] = {
                fn = function(mh)
                    editor:message(mh.client_id, "hello!")
                end
            },
            ["e"] = {fn = function(mh)
                editor:error(mh.client_id, "oops!")
            end}
        }
    },
    insertion = {
        name = "I",
        title = "",
        mappings = {["esc"] = {fn = function(mh) mh:exit_mode() end}}
    },
    moveto = {name = "g", title = "", mappings = make_goto_mappings(false)},
    extendto = {name = "G", title = "", mappings = make_goto_mappings(true)}
}

function ModalHandler.new(client_id)
    local self = setmetatable({}, ModalHandler)
    self.client_id = client_id
    self.mode_stack = {"normal"}
    self.redraw_status = false
    return self
end

function ModalHandler:enter_mode(mode)
    table.insert(self.mode_stack, mode)
    self.redraw_status = true
end

function ModalHandler:exit_mode()
    table.remove(self.mode_stack)
    self.redraw_status = true
end

function ModalHandler:curmode() return self.mode_stack[#self.mode_stack] end

function ModalHandler:set_status(key)
    local status_line = editor.clients[self.client_id].status_line
    if not status_line.keys then status_line.keys = {index = 80} end
    if not status_line.mode then status_line.mode = {index = 90} end
    status_line.keys.text = key and key.display or ""
    status_line.mode.text = self.modes[self:curmode()].name
    self.redraw_status = true
end

function ModalHandler:handle(key)
    local mode = self.modes[self:curmode()]
    local mapping = mode.mappings[key.display]
    local handled = false
    if mapping then
        mapping.fn(self)
        handled = true
    elseif mode.mappings.default then
        mode.mappings.default(self, key)
        handled = true
    end

    if handled then self:set_status(key) end

    local ret = {redraw_status = self.redraw_status}
    self.redraw_status = false
    return ret
end

M.ModalHandler = ModalHandler

return M
