editor:scratch(env.client, "testing", "123\n456")

local function assert_sel(c)
    local got = editor:get_context(env.client).selections.testing[1].text
    assert(got == c, string.format("%q != %q", got, c))
end

editor:delete_selection(env.client)
assert_sel("2")
editor:move_right(env.client)
editor:delete_selection(env.client)
assert_sel("\n")
editor:delete_selection(env.client)
assert_sel("4")

editor:move_to_begin(env.client)
editor:move_to_end(env.client, true)
assert_sel("2456")
editor:delete_selection(env.client)
assert_sel("\n")
editor:delete_selection(env.client)
