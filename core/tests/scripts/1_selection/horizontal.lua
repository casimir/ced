editor:scratch(env.client, "testing", "123")

local function assert_sel(c)
    local got = editor:get_context(env.client).selections.testing[1].text
    assert(got == c, string.format("%q != %q", got, c))
end

assert_sel("1")
editor:move_right(env.client, true)
assert_sel("12")
editor:move_right(env.client)
assert_sel("3")
editor:move_left(env.client, true)
assert_sel("23")
editor:move_left(env.client)
assert_sel("1")
