-- http://lua-users.org/wiki/ObjectOrientationTutorial

local keys = require "keys"

clients = {
    new = function(id)
        return {
            status_line = {
                client = {index = 100}
            },
            key_handler = keys.ModalHandler.new(id)
        }
    end
}

editor:debug("lua state ready")
editor:debug("lua path: " .. package.path)