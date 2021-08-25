editor:scratch(
    env.client,
    "testing",
    [[abcdefghijklmnopqrstuvwxyz
1234567890

ABCDEFGHIJKLMNOPQRSTUVWXYZ]]
)

local function assert_sel(c)
    local got = editor:get_context(env.client).selections.testing[1].text
    assert(got == c, string.format("%q != %q", got, c))
end

for _ = 1, 12, 1 do
    editor:move_right(env.client)
end
assert_sel("m")
editor:move_down(env.client)
assert_sel("\n")
editor:move_down(env.client)
assert_sel("\n")
editor:move_down(env.client)
assert_sel("M")

editor:move_left(env.client)
assert_sel("L")
editor:move_up(env.client)
assert_sel("\n")
editor:move_up(env.client)
assert_sel("\n")
editor:move_up(env.client)
assert_sel("l")
