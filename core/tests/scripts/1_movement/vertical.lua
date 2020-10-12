editor:scratch(env.client, "testing", [[abcdefghijklmnopqrstuvwxyz
1234567890

ABCDEFGHIJKLMNOPQRSTUVWXYZ]])

local function assert_sel(c)
    local got = editor:get_context(env.client).selections.testing[1].text
    assert(got == c, string.format("%q != %q", got, c))
end

assert_sel("a")
editor:move_down(env.client)
assert_sel("1")
editor:move_down(env.client)
assert_sel("\n")
editor:move_down(env.client)
assert_sel("A")
editor:move_down(env.client)
assert_sel("A")

editor:move_up(env.client)
assert_sel("\n")
editor:move_up(env.client)
assert_sel("1")
editor:move_up(env.client)
assert_sel("a")
editor:move_up(env.client)
assert_sel("a")
