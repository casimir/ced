local M = {}

---@param value string|function|table|nil
---@return string
local function asstring(value)
    if not value then
        return ""
    end

    local ok, text = pcall(value)
    if ok then
        return text
    else
        return value
    end
end
M.asstring = asstring

local function stringify(val)
    if type(val) == "table" then
        local s = "{"
        for k, v in pairs(val) do
            if string.len(s) > 1 then
                s = s .. ", "
            end
            s = s .. stringify(k) .. " = " .. stringify(v)
        end
        return s .. "}"
    elseif type(val) == "string" then
        return string.format("%q", val)
    else
        return tostring(asstring(val))
    end
end
M.stringify = stringify

local function max_length(values)
    local max = 0
    for _, it in ipairs(values) do
        if #it > max then
            max = #it
        end
    end
    return max
end
M.max_length = max_length

return M
