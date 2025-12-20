# ANSI Editor MCP Documentation

## Overview

The ANSI editor is the primary editor for creating and editing ANSI/ASCII art files. It provides a canvas-based editing environment with support for layers, colors, fonts, and various drawing tools.

## Document Model

### Buffer Structure

The document is based on a `TextBuffer` containing:

| Component   | Description                                      |
| ----------- | ------------------------------------------------ |
| **Size**    | Width × Height in character cells                |
| **Layers**  | Multiple overlapping layers (like Photoshop)     |
| **Caret**   | Current editing position with text attributes    |
| **Palette** | Color palette (16-256 colors depending on format) |
| **Fonts**   | One or more bitmap fonts                         |

### Format Modes

| Mode             | Colors         | Fonts     | Palette Editing | Description                  |
| ---------------- | -------------- | --------- | --------------- | ---------------------------- |
| **LegacyDos**    | 16 fixed       | 1         | ❌               | Classic DOS ANSI             |
| **XBin**         | 16 selectable  | 1         | ✅               | XBin format                  |
| **XBinExtended** | 8 foreground   | 2         | ✅               | Extended XBin with dual fonts |
| **Unrestricted** | Full RGB       | Unlimited | ✅               | Modern unrestricted format   |

### Layers

Each layer has:

| Property             | Type     | Description                  |
| -------------------- | -------- | ---------------------------- |
| `title`              | String   | Layer name                   |
| `is_visible`         | bool     | Whether layer is rendered    |
| `is_locked`          | bool     | Prevents editing             |
| `is_position_locked` | bool     | Prevents moving              |
| `offset`             | Position | X/Y offset from origin       |
| `size`               | Size     | Width × Height               |
| `transparency`       | u8       | Layer transparency (0-255)   |
| `mode`               | Mode     | Normal, Chars, or Attributes |

Layer modes:

- **Normal**: Edit both characters and attributes
- **Chars**: Edit characters only, preserve attributes
- **Attributes**: Edit attributes only, preserve characters

### Caret (Cursor)

The caret represents the current editing position:

| Property      | Type          | Description                   |
| ------------- | ------------- | ----------------------------- |
| `x`, `y`      | i32           | Position in layer coordinates |
| `attribute`   | TextAttribute | Current text styling          |
| `insert_mode` | bool          | Insert vs overwrite mode      |
| `font_page`   | u8            | Current font index            |

### Text Attributes

Each character cell has attributes:

| Attribute          | Description                                |
| ------------------ | ------------------------------------------ |
| `foreground_color` | Foreground color (palette/RGB/transparent) |
| `background_color` | Background color (palette/RGB/transparent) |
| `font_page`        | Font index for this character              |
| `bold`             | Bold flag                                  |
| `blink`            | Blinking flag                              |

### Color Types

Colors can be represented as:

| Type                 | Format     | Description                      |
| -------------------- | ---------- | -------------------------------- |
| `Palette(n)`         | 0-15       | Standard 16-color palette index  |
| `ExtendedPalette(n)` | 0-255      | Extended xterm 256-color palette |
| `Rgb(r,g,b)`         | 0-255 each | Direct RGB color                 |
| `Transparent`        | -          | Fully transparent (see-through)  |

### Selection

- **Rectangle**: Block selection (Alt+drag)

## File Formats

Supported formats for ANSI art:

| Format  | Extension | Description                              |
| ------- | --------- | ---------------------------------------- |
| ANSI    | .ans      | Standard ANSI with escape codes          |
| XBin    | .xb       | Extended binary with custom palette/font |
| PCBoard | .pcb      | PCBoard BBS format                       |
| Avatar  | .avt      | Avatar format                            |
| Artworx | .adf      | Artworx Data Format                      |
| IcyDraw | .icy      | Native format with full features         |
| PNG     | .png      | Image export                             |
| GIF     | .gif      | Animated GIF export                      |

## Coordinate Systems

Two coordinate systems are used:

1. **Document coordinates**: Absolute position on canvas
2. **Layer coordinates**: Relative to layer offset

When the caret is at layer position (5, 3) and the layer has offset (10, 5), the document position is (15, 8).

## MCP Tools

All MCP editing operations that modify the buffer are **atomic** - they are wrapped in a single undo group and can be reversed with a single undo command.

### Undo Behavior

| Operation           | Atomic | Description                                         |
| ------------------- | ------ | --------------------------------------------------- |
| `ansi_run_script`   | ✅      | All script changes = one undo                       |
| `ansi_set_char`     | ✅      | Single character change = one undo                  |
| `ansi_set_region`   | ✅      | All region changes = one undo                       |
| `ansi_set_layer_props` | ✅   | Property changes use layer undo operations          |
| `ansi_set_color`    | ❌      | Palette changes are currently not undo-tracked      |

### ansi_run_script

Execute a Lua script on the current buffer. This provides maximum flexibility for AI-driven manipulation.

**Parameters:**

| Name               | Type   | Required | Description                                                  |
| ------------------ | ------ | -------- | ------------------------------------------------------------ |
| `script`           | string | ✅        | The Lua script code to execute                               |
| `undo_description` | string | ❌        | Optional description for undo stack (default: "MCP Script") |

**Returns:**

- On success: Script output (collected `log()` messages)
- On error: Error message describing the failure

**Example:**

```lua
-- Fill selection with random colors
for y = start_y, end_y do
    for x = start_x, end_x do
        buf.fg = math.random(0, 15)
        buf.bg = math.random(0, 7)
        buf:set_char(x, y, "█")
    end
end
log("Filled " .. (end_x - start_x + 1) * (end_y - start_y + 1) .. " cells")
```

**Notes:**

- All changes are wrapped in an atomic undo operation
- The `log()` function output is collected and returned
- The script has access to the same API as plugins (see Lua API below)
- Selection bounds (`start_x`, `end_x`, `start_y`, `end_y`) are automatically provided

## Lua API

Lua is used as scripting language for the animation engine and plugin language.

### Global Variables

#### Animations only

| Variable    | Description                       |
| ----------- | --------------------------------- |
| `cur_frame` | Number of current frame (1 based) |

Monitor settings (just for video output):
`monitor_type`, `monitor_gamma`, `monitor_contrast`, `monitor_saturation`, `monitor_brightness`, `monitor_blur`, `monitor_curvature`, `monitor_scanlines`

#### Plugins only

| Variable  | Description          |
| --------- | -------------------- |
| `buf`     | Current buffer       |
| `start_x` | Current area start x |
| `end_x`   | Current area end x   |
| `start_y` | Current area start y |
| `end_y`   | Current area end y   |

The current area is the whole layer or the selected portion of it. The coordinates are current layer coordinates.

### Global Functions (Animations only)

| Function                              | Returns | Description                                              |
| ------------------------------------- | ------- | -------------------------------------------------------- |
| `new_buffer(width: i32, height: i32)` | Buffer  | Create new, empty buffer with given size                 |
| `load_buffer(file_name: String)`      | Buffer  | Loads a buffer relatively to the animation file          |
| `next_frame(buf: Buffer)`             | -       | Snapshots the "buf" table as new frame and moves to next |
| `set_delay(delay: u32)`               | -       | Sets current frame delay in ms (default: 100)            |
| `get_delay()`                         | u32     | Gets current frame delay                                 |

### Buffer Fields

| Field         | Description                                           |
| ------------- | ----------------------------------------------------- |
| `width`       | Gets or sets the width of the buffer                  |
| `height`      | Gets or sets the height of the buffer                 |
| `layer_count` | Gets the number of layers in the buffer               |
| `fg`          | Gets or sets current foreground color (palette index) |
| `bg`          | Gets or sets current background color (palette index) |
| `layer`       | Gets or sets the current layer                        |
| `font_page`   | Gets or sets the current font page of the caret       |
| `x`           | Gets or sets the caret x position                     |
| `y`           | Gets or sets the caret y position                     |

### Buffer Methods

| Method                                 | Returns | Description                                 |
| -------------------------------------- | ------- | ------------------------------------------- |
| `clear()`                              | -       | Clears the buffer and resets caret          |
| `set_layer_position(layer, x, y)`      | -       | Sets the offset of a specific layer         |
| `get_layer_position(layer)`            | x, y    | Gets the offset of a specific layer         |
| `set_layer_visible(layer, is_visible)` | -       | Sets if layer is visible                    |
| `get_layer_visible(layer)`             | bool    | Gets if layer is visible                    |
| `fg_rgb(r, g, b)`                      | u32     | Sets caret fg to RGB, returns palette number |
| `fg_rgb("#rrggbb")`                    | u32     | Sets caret fg from HTML color notation      |
| `bg_rgb(r, g, b)`                      | u32     | Sets caret bg to RGB, returns palette number |
| `bg_rgb("#rrggbb")`                    | u32     | Sets caret bg from HTML color notation      |
| `set_char(x, y, string)`               | -       | Sets a char at position (uses caret color)  |
| `get_char(x, y)`                       | string  | Gets a char at position                     |
| `clear_char(x, y)`                     | -       | Clears a char (sets to invisible)           |
| `grab_char(x, y)`                      | string  | Gets char and copies its attributes to caret |
| `get_fg(x, y)`                         | u32     | Gets the foreground at position             |
| `set_fg(x, y, fg)`                     | -       | Sets foreground at position                 |
| `get_bg(x, y)`                         | u32     | Gets the background at position             |
| `set_bg(x, y, bg)`                     | -       | Sets background at position                 |
| `print(string)`                        | -       | Prints string at caret, advances position   |

Note: For representing chars, strings with length 1 are used. Additional chars are ignored. Empty strings lead to error. Lua uses unicode which is converted to the buffer type.

## CP437 Unicode Table

The Lua API uses unicode. This makes scripts more flexible across different buffer types. For CP437 this conversion table is used:

| Offset | 0   | 1   | 2   | 3   | 4   | 5   | 6   | 7   | 8   | 9   | A   | B   | C   | D   | E   | F   |
| ------ | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0      | NUL | ☺   | ☻   | ♥   | ♦   | ♣   | ♠   | •   | ◘   | ○   | ◙   | ♂   | ♀   | ♪   | ♫   | ☼   |
| 16     | ►   | ◄   | ↕   | ‼   | ¶   | §   | ▬   | ↨   | ↑   | ↓   | →   | ←   | ∟   | ↔   | ▲   | ▼   |
| 32     | SP  | !   | "   | #   | $   | %   | &   | '   | (   | )   | *   | +   | ,   | -   | .   | /   |
| 48     | 0   | 1   | 2   | 3   | 4   | 5   | 6   | 7   | 8   | 9   | :   | ;   | <   | =   | >   | ?   |
| 64     | @   | A   | B   | C   | D   | E   | F   | G   | H   | I   | J   | K   | L   | M   | N   | O   |
| 80     | P   | Q   | R   | S   | T   | U   | V   | W   | X   | Y   | Z   | [   | \   | ]   | ^   | _   |
| 96     | `   | a   | b   | c   | d   | e   | f   | g   | h   | i   | j   | k   | l   | m   | n   | o   |
| 112    | p   | q   | r   | s   | t   | u   | v   | w   | x   | y   | z   | {   | \|  | }   | ~   | DEL |
| 176    | ░   | ▒   | ▓   | │   | ┤   | ╡   | ╢   | ╖   | ╕   | ╣   | ║   | ╗   | ╝   | ╜   | ╛   | ┐   |
| 192    | └   | ┴   | ┬   | ├   | ─   | ┼   | ╞   | ╟   | ╚   | ╔   | ╩   | ╦   | ╠   | ═   | ╬   | ╧   |
| 208    | ╨   | ╤   | ╥   | ╙   | ╘   | ╒   | ╓   | ╫   | ╪   | ┘   | ┌   | █   | ▄   | ▌   | ▐   | ▀   |

Source: <https://en.wikipedia.org/wiki/Code_page_437>
