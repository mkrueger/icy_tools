# Animation Editor - Lua Scripting API

The Animation Editor uses Lua scripts (`.icyanim` files) to create ANSI animations. Scripts generate frames by manipulating buffers and calling `next_frame()` to capture each frame.

## Quick Start

```lua
-- Simple scrolling text animation
local buf = new_buffer(80, 25)

for i = 1, 80 do
    buf:clear()
    buf.fg = 14  -- Yellow
    buf:set_char(i, 12, "★")
    buf:print("Hello World!")
    next_frame(buf)
end
```

## Global Variables

### Animation State

| Variable | Type | Description |
|----------|------|-------------|
| `cur_frame` | integer | Current frame number (1-based, read-only during playback) |

### Monitor Settings (CRT Effects)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `monitor_type` | integer | 0 | 0=Color, 1=Grayscale, 2=Amber, 3=Green, 4=Apple2, 5=Futuristic |
| `monitor_gamma` | float | 1.0 | Gamma correction |
| `monitor_contrast` | float | 100.0 | Contrast percentage |
| `monitor_saturation` | float | 100.0 | Saturation percentage |
| `monitor_brightness` | float | 100.0 | Brightness percentage |
| `monitor_blur` | float | 0.0 | Blur effect strength |
| `monitor_curvature` | float | 0.0 | CRT curvature effect |
| `monitor_scanlines` | float | 0.0 | Scanlines effect strength |

## Global Functions

| Function | Returns | Description |
|----------|---------|-------------|
| `new_buffer(width, height)` | Buffer | Create new empty buffer |
| `load_buffer(filename)` | Buffer | Load file relative to .icyanim location |
| `next_frame(buf)` | - | Capture buffer as new frame |
| `set_delay(ms)` | - | Set frame delay in milliseconds (default: 100) |
| `get_delay()` | integer | Get current frame delay |
| `log(text)` | - | Write to log panel (max 1000 entries) |

## Buffer Object

A Buffer represents a screen with layers, caret position, and color palette.

### Fields (Read/Write)

| Field | Type | Description |
|-------|------|-------------|
| `width` | integer | Buffer width in characters |
| `height` | integer | Buffer height in characters |
| `layer_count` | integer | Number of layers (read-only) |
| `layer` | integer | Current layer index |
| `fg` | integer | Foreground color (palette index 0-255) |
| `bg` | integer | Background color (palette index 0-255) |
| `x` | integer | Caret X position |
| `y` | integer | Caret Y position |
| `font_page` | integer | Current font page |

### Character Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `set_char(x, y, char)` | - | Set character at position (uses caret colors) |
| `get_char(x, y)` | string | Get character at position |
| `clear_char(x, y)` | - | Clear character (make invisible) |
| `grab_char(x, y)` | string | Get char and copy its attributes to caret |
| `clear()` | - | Clear entire buffer |

### Color Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `fg_rgb(r, g, b)` | integer | Set foreground RGB, returns palette index |
| `fg_rgb("#RRGGBB")` | integer | Set foreground from hex, returns palette index |
| `bg_rgb(r, g, b)` | integer | Set background RGB, returns palette index |
| `bg_rgb("#RRGGBB")` | integer | Set background from hex, returns palette index |
| `get_fg(x, y)` | integer | Get foreground color at position |
| `set_fg(x, y, color)` | - | Set foreground color at position |
| `get_bg(x, y)` | integer | Get background color at position |
| `set_bg(x, y, color)` | - | Set background color at position |
| `set_palette_color(idx, r, g, b)` | - | Define palette color |
| `get_palette_color(idx)` | r,g,b | Get palette color components |

### Text Output

| Method | Returns | Description |
|--------|---------|-------------|
| `print(text)` | - | Print at caret position, advance caret |
| `println(text)` | - | Print with newline |
| `set_caret(x, y)` | - | Set caret position |

Supports PCBoard @X color codes in print:
- `@X0F` = Black background, white foreground
- `@X1C` = Blue background, red foreground

### Layer Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `set_layer_position(layer, x, y)` | - | Set layer offset |
| `set_layer_x(layer, x)` | - | Set layer X offset only |
| `set_layer_y(layer, y)` | - | Set layer Y offset only |
| `get_layer_position(layer)` | x, y | Get layer offset |
| `set_layer_visible(layer, visible)` | - | Show/hide layer |
| `get_layer_visible(layer)` | boolean | Check layer visibility |

## Standard Colors (DOS Palette)

| Index | Color | Index | Color |
|-------|-------|-------|-------|
| 0 | Black | 8 | Dark Gray |
| 1 | Blue | 9 | Light Blue |
| 2 | Green | 10 | Light Green |
| 3 | Cyan | 11 | Light Cyan |
| 4 | Red | 12 | Light Red |
| 5 | Magenta | 13 | Light Magenta |
| 6 | Brown | 14 | Yellow |
| 7 | Light Gray | 15 | White |

## Examples

### Layer Animation (Parallax Scrolling)
```lua
local bg = load_buffer("background.ans")
local fg = load_buffer("foreground.ans")

-- Copy foreground to new layer
buf = bg
-- ... add layer logic

for i = 1, 100 do
    buf:set_layer_position(0, -i, 0)      -- Background scrolls slow
    buf:set_layer_position(1, -i * 2, 0)  -- Foreground scrolls fast
    next_frame(buf)
end
```

### Color Cycling
```lua
local buf = load_buffer("art.ans")

for frame = 1, 60 do
    -- Rotate palette colors 1-15
    local saved = buf:get_palette_color(1)
    for i = 1, 14 do
        local r, g, b = buf:get_palette_color(i + 1)
        buf:set_palette_color(i, r, g, b)
    end
    buf:set_palette_color(15, saved)
    
    next_frame(buf)
    set_delay(50)  -- Fast cycling
end
```

### Monitor Effects
```lua
local buf = load_buffer("retro.ans")

-- Apply CRT monitor look
monitor_type = 3        -- Green phosphor
monitor_scanlines = 0.3
monitor_curvature = 0.1
monitor_blur = 0.05

next_frame(buf)
```

## CP437 Character Reference

Lua uses Unicode strings. Characters are converted to/from CP437 automatically.

Common box drawing characters:
- `│` `─` `┌` `┐` `└` `┘` - Single line
- `║` `═` `╔` `╗` `╚` `╝` - Double line
- `░` `▒` `▓` `█` - Shading blocks

## Limits

- Maximum frames: 4096
- Maximum log entries: 1000
- Frame delay: 1-65535 ms

## MCP Tools for Animation Editor

| Tool | Description |
|------|-------------|
| `animation.get_text(offset?, length?)` | Get script text |
| `animation.replace_text(offset, length, text)` | Replace script text |
| `animation.get_screen(frame)` | Get rendered frame as ANSI |

Use `get_status()` to check:
- `text_length` - Script size
- `frame_count` - Number of frames after running
- `errors` - Lua errors if any
