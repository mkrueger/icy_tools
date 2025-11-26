cls()

if on_screen("Login failed") then
    println("Login failed, aborting...")
    return
end
caret_fg = 2

println("FG: " .. caret_fg)
caret_right()
caret_right()
caret_right()
caret_right()
