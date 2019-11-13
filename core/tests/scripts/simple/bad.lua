assert(
    not pcall(
        function()
            assert(1 == 0)
        end
    )
)
