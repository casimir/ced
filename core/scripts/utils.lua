local M = {}

---@param value string|function|table|nil
---@return string
local function string(value)
    if not value then return "" end

    local ok, text = pcall(value)
    if ok then return text
    else return value
    end
end
M.string = string

return M