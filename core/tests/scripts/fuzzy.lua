math.randomseed(os.time())

local function move()
    local roll = math.random(4)
    if roll == 1 then
        -- print("> left")
        editor:move_left(env.client)
    elseif roll == 2 then
        -- print("> right")
        editor:move_right(env.client)
    elseif roll == 3 then
        -- print("> up")
        editor:move_up(env.client)
    else
        -- print("> down")
        editor:move_down(env.client)
    end
end

for i = 1, 1000, 1 do
    -- local ctx = editor:get_context(env.client).selections
    -- print("step", i, utils.stringify(ctx["*debug*"][1].cursor.pos))
    move()
end
