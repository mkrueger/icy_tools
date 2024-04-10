-- Title: Elite Writing
table = {
    ["e"] = "ε",
    ["E"] = "Σ",
    ["I"] = "¡",
    ["r"] = "τ",
    ["R"] = "₧",
    ["F"] = "ƒ",
    ["f"] = "ƒ",
    ["a"] = "α",
    ["A"] = "^",
    ["b"] = "ß",
    ["B"] = "ß",
    ["n"] = "π",
    ["N"] = "π",
    ["u"] = "û",
    ["U"] = "∩",
    ["o"] = "φ",
    ["O"] = "σ",
    ["L"] = "£",
    ["l"] = "£",
    ["X"] = "æ",
    ["x"] = "æ",
    ["S"] = "$",
    ["s"] = "$",
    ["C"] = "¢",
    ["c"] = "¢",
    ["D"] = "δ",
    ["d"] = "δ",
    ["y"] = "µ",
    ["t"] = "Γ"
}

for y = start_y, end_y, 1 do
    for x = start_x, end_x, 1 do
        local ch = buf:pickup_char(x, y)
        if table[ch] then
            buf:set_char(x, y, table[ch])
        end
    end
end