-- IcyTerm Screen API
-- Global wrapper functions and variables for the screen object
--
-- This file is loaded automatically when the scripting engine starts.
-- It provides convenient global access to screen manipulation functions.

-- Global variables (read/write for position/colors, read-only for dimensions)
local _G_mt = getmetatable(_G) or {}
local _G_index_orig = _G_mt.__index
local _G_newindex_orig = _G_mt.__newindex

_G_mt.__index = function(t, k)
    if k == "where_x" then return screen.x end
    if k == "where_y" then return screen.y end
    if k == "screen_width" then return screen.width end
    if k == "screen_height" then return screen.height end
    if k == "caret_fg" then return screen.fg end
    if k == "caret_bg" then return screen.bg end
    if k == "caret_visible" then return screen.caret_visible end
    if k == "caret_blinking" then return screen.caret_blinking end
    if type(_G_index_orig) == "function" then return _G_index_orig(t, k) end
    if _G_index_orig then return _G_index_orig[k] end
    return nil
end

_G_mt.__newindex = function(t, k, v)
    if k == "where_x" then screen.x = v; return end
    if k == "where_y" then screen.y = v; return end
    if k == "caret_fg" then screen.fg = v; return end
    if k == "caret_bg" then screen.bg = v; return end
    if k == "caret_visible" then screen.caret_visible = v; return end
    if k == "caret_blinking" then screen.caret_blinking = v; return end
    if type(_G_newindex_orig) == "function" then _G_newindex_orig(t, k, v); return end
    rawset(t, k, v)
end

setmetatable(_G, _G_mt)

-- Cursor positioning
function gotoxy(x, y) screen:gotoxy(x, y) end

-- Text output
function print(s) screen:print(s) end
function println(s) screen:println(s) end

-- Character manipulation
function set_char(x, y, ch) screen:set_char(x, y, ch) end
function clear_char(x, y) screen:clear_char(x, y) end
function get_char(x, y) return screen:get_char(x, y) end
function grab_char(x, y) return screen:grab_char(x, y) end

-- Color manipulation at position
function set_fg(x, y, col) screen:set_fg(x, y, col) end
function get_fg(x, y) return screen:get_fg(x, y) end
function set_bg(x, y, col) screen:set_bg(x, y, col) end
function get_bg(x, y) return screen:get_bg(x, y) end

-- Current color setting (RGB)
function fg_rgb(r, g, b) return screen:fg_rgb(r, g, b) end
function bg_rgb(r, g, b) return screen:bg_rgb(r, g, b) end

-- Palette
function set_palette_color(col, r, g, b) screen:set_palette_color(col, r, g, b) end
function get_palette_color(col) return screen:get_palette_color(col) end

-- Screen clearing
function cls() screen:clear() end

-- Layer management
function layer_count() return screen.layer_count end
function set_layer(l) screen.layer = l end
function get_layer() return screen.layer end
function set_layer_position(l, x, y) screen:set_layer_position(l, x, y) end
function get_layer_position(l) return screen:get_layer_position(l) end
function set_layer_visible(l, v) screen:set_layer_visible(l, v) end
function get_layer_visible(l) return screen:get_layer_visible(l) end

-- Caret movement (with default n=1)
function caret_left(n) screen:caret_left(n or 1) end
function caret_right(n) screen:caret_right(n or 1) end
function caret_up(n) screen:caret_up(n or 1) end
function caret_down(n) screen:caret_down(n or 1) end

-- Caret line control
function caret_home() screen:caret_home() end
function caret_eol() screen:caret_eol() end
function caret_cr() screen:caret_cr() end
function caret_lf() screen:caret_lf() end
function caret_next_line() screen:caret_next_line() end

-- Caret editing
function caret_bs() screen:caret_bs() end
function caret_del() screen:caret_del() end
function caret_ins() screen:caret_ins() end
function caret_tab() screen:caret_tab() end

-- Caret state
function save_caret() screen:save_caret() end
function restore_caret() screen:restore_caret() end
