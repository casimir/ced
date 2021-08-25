local M = {}

local ModalHandler = {}
ModalHandler.__index = ModalHandler

setmetatable(ModalHandler, {
    __call = function(cls, ...)
        return cls.new(...)
    end,
})

---@param extend boolean
local function make_goto_mappings(extend)
    return {
        ["h"] = {
            desc = "line begin",
            fn = function(mh)
                mh:exit_mode()
                return editor:move_to_line_begin(mh.client_id, extend)
            end,
        },
        ["l"] = {
            desc = "line end",
            fn = function(mh)
                mh:exit_mode()
                return editor:move_to_line_end(mh.client_id, extend)
            end,
        },
        default = function(mh, key)
            mh:exit_mode()
        end,
    }
end

ModalHandler.modes = {
    normal = {
        name = "N",
        title = "",
        mappings = {
            ["i"] = {
                fn = function(mh)
                    mh:enter_mode("insertion")
                end,
            },
            ["g"] = {
                fn = function(mh)
                    mh:enter_mode("moveto")
                end,
            },
            ["s-g"] = {
                fn = function(mh)
                    mh:enter_mode("extendto")
                end,
            },
            ["h"] = {
                fn = function(mh)
                    return editor:move_left(mh.client_id, false)
                end,
            },
            ["j"] = {
                fn = function(mh)
                    return editor:move_down(mh.client_id, false)
                end,
            },
            ["k"] = {
                fn = function(mh)
                    return editor:move_up(mh.client_id, false)
                end,
            },
            ["l"] = {
                fn = function(mh)
                    return editor:move_right(mh.client_id, false)
                end,
            },
            ["s-h"] = {
                fn = function(mh)
                    return editor:move_left(mh.client_id, true)
                end,
            },
            ["s-j"] = {
                fn = function(mh)
                    return editor:move_down(mh.client_id, true)
                end,
            },
            ["s-k"] = {
                fn = function(mh)
                    return editor:move_up(mh.client_id, true)
                end,
            },
            ["s-l"] = {
                fn = function(mh)
                    return editor:move_right(mh.client_id, true)
                end,
            },
            ["m"] = {
                fn = function(mh)
                    editor:message(mh.client_id, "hello!")
                end,
            },
            ["e"] = {
                fn = function(mh)
                    editor:error(mh.client_id, "oops!")
                end,
            },
            ["d"] = {
                fn = function(mh)
                    editor:delete_selection(mh.client_id)
                end,
            },
        },
    },
    insertion = {
        name = "I",
        title = "",
        mappings = { ["esc"] = {
            fn = function(mh)
                mh:exit_mode()
            end,
        } },
    },
    moveto = {
        name = "g",
        title = "move to",
        hint = true,
        mappings = make_goto_mappings(false),
    },
    extendto = {
        name = "G",
        title = "extend to",
        hint = true,
        mappings = make_goto_mappings(true),
    },
}

function ModalHandler.new(client_id)
    local self = setmetatable({}, ModalHandler)
    self.client_id = client_id
    self.mode_stack = { "normal" }
    return self
end

function ModalHandler:enter_mode(mode)
    table.insert(self.mode_stack, mode)
    self:show_mode_hint(mode)
end

function ModalHandler:exit_mode()
    table.remove(self.mode_stack)
end

function ModalHandler:curmode()
    return self.mode_stack[#self.mode_stack]
end

function ModalHandler:show_mode_hint(mode)
    local modecfg = self.modes[mode]
    if not modecfg.hint then
        return
    end

    local lines = { modecfg.title }
    for k, v in pairs(modecfg.mappings) do
        if k ~= "default" then
            local line = string.format("%s: %s", k, v.desc or "")
            table.insert(lines, line)
        end
    end

    editor:show_hint(self.client_id, lines, true)
end

function ModalHandler:set_status(key)
    local status_line = editor.clients[self.client_id].status_line
    if not status_line.keys then
        status_line.keys = { index = 80 }
    end
    if not status_line.mode then
        status_line.mode = { index = 90 }
    end
    status_line.keys.text = key and key.display or ""
    status_line.mode.text = self.modes[self:curmode()].name
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

    if handled then
        self:set_status(key)
        editor:push_status_line(self.client_id)
    end
end

M.ModalHandler = ModalHandler

return M
