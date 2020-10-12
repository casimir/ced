editor:scratch(env.client, "testing", "123\n456")

local function assert_sel(c)
    local got = editor:get_context(env.client).selections.testing[1].text
    assert(got == c, string.format("%q != %q", got, c))
end

assert_sel("1")
editor:move_to_end(env.client, true)
assert_sel("123\n456")
editor:move_to_end(env.client)
assert_sel("6")
editor:move_to_begin(env.client, true)
assert_sel("123\n456")
editor:move_to_begin(env.client)
assert_sel("1")
