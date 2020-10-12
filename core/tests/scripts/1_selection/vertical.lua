editor:scratch(env.client, "testing", "123\n456")

local function assert_sel(c)
    local got = editor:get_context(env.client).selections.testing[1].text
    assert(got == c, string.format("%q != %q", got, c))
end

assert_sel("1")
editor:move_down(env.client, true)
assert_sel("123\n4")
editor:move_down(env.client)
assert_sel("4")
editor:move_up(env.client, true)
assert_sel("123\n4")
editor:move_up(env.client)
assert_sel("1")
