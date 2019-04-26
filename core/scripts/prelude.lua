editor:append_debug("init lua state...")

function key_handler (client, key)
    editor:append_debug('client "'..client..'" sent key "'..key..'"')
end
