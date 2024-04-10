# LUA API

Lua is used as scripting language for the animation engine and plugin language

## Global variables
### Animations only

| Variable     | Description
|--------------|--------------------------------------
| `cur_frame`  | Number of current frame (1 based)

Monitor settings (just for video output, play around with:)
`monitor_type`
`monitor_gamma`
`monitor_contrast`
`monitor_saturation`
`monitor_brightness`
`monitor_blur`
`monitor_curvature`
`monitor_scanlines`


### Plugins only
| Variable     | Description
|--------------|--------------------------------------
| `buf`        | Current buffer
| `start_x`    | Current area start x
| `end_x`      | Current area end x
| `start_y`    | Current area start y
| `end_y`      | Current area end y

The current area is the whole layer or the selected portion of it. The coordinates are current layer coordinates.
## Global function

### Animations only

| Function                               | Returns    | Description
|----------------------------------------|------------|--------------------------
| `new_buffer(width: i32, height: i32)`  |  Buffer    | Create new, empty buffer with given get_size
| `load_buffer(file_name: String)`       |  Buffer    | Loads a buffer relatively to the animation file.
| `next_frame(buf: Buffer)`              |  -         | Snapshots the "buf" table as new frame and moves to the next frame.
| `set_delay(delay: u32)`                |  -         | Sets current frame delay in ms - note each frame has it's own delay so animations can change speed (default: 100)
| `get_delay()`                          |  u32       | Gets current frame delay

## Buffers

### Fields

| Field      | Description
|---------------|--------------------------
| `width`       | Gets or sets the width of the buffer
| `height`      | Gets or sets the height of the buffer
| `layer_count` | Gets th  number of layers in the buffer
|  |Caret related fields
| `fg` | Gets or sets current foreground color of the caret (palette index)
| `bg` | Gets or sets current background color of the caret (palette index)
| `layer` | Gets or sets the current layer
| `font_page` | Gets or sets the current font page of the caret
| `x`           | Gets or sets the caret x position
| `y`           | Gets or sets the caret y position

### Methods


| Method                                 | Returns | Description
|----------------------------------------|---------|--------------------------
| `clear()`                              | -       | clears the buffer & resets caret

Layer related methods

| Method                                 | Returns | Description
|----------------------------------------|---------|--------------------------
| `set_layer_position(layer, x, y)`      | -       | Sets the offset of a specific layer to move it
| `get_layer_position(layer)`            | x, y    | Gets the offset of a specific layer to move it
| `set_layer_visible(layer, is_visible)` |  -      | Sets if layer is visible
| `get_layer_visible(layer)`             | bool    | Gets if layer is visible

Input/Output

| Method                                 | Returns | Description
|----------------------------------------|---------|--------------------------
| `fg_rgb(r, g, b)`                      | u32     | Sets the caret fg rgb color, returns color palette number
| `fg_rgb(#rrggbb)`                      | u32     | Sets the caret fg rgb color with html style notation , returns color palette number
| `bg_rgb(r, g, b)`                      | u32     | Sets the caret bg rgb color, returns color palette number
| `bg_rgb(#rrggbb)`                      | u32     | Sets the caret bg rgb color with html style notation , returns color palette number
| `set_char(x, y, string)`               | -       | Sets a specific char at a given position (uses caret color)
| `get_char(x, y)`                       | string  | Gets a specific char at a given position
| `clear_char(x, y)`                     | -       | Clears a char (sets to invisible)
| `pickup_char(x, y)`                    | string  | Like get char but sets all attributes to the char attributes
| `get_fg(x, y)`                         | u32     | Gets the foreground at a given positon
| `set_fg(x, y, fg)`                     | -       | Sets a specific foreground at a given layer position
| `get_bg(x, y)`                         | u32     | Gets the background at a given positon
| `set_bg(x, y, bg)`                     | -       | Sets a specific foreground at a given layer position
| `print(string)`                        | -       | Prints a string at caret position, advances caret.

Note for representing chars strings with length 1 is used. Additional chars are ignored. Empty strings lead to error.
LUA uses unicode as char representation which is converted to the according buffer type.

## CP437 - Unicode table

The LUA API useses unicode. This makes scripts more flexibile accross different buffer types. For CP437 this conversion table is used:

|Offset|0|1|2|3|4|5|6|7|8|9|A|B|C|D|E|F|
|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-
|  0|NUL|☺ (263A)|☻ (263B)|♥ (2665)|♦ (2666)|♣ (2663)|♠ (2660)|• (2022)|◘ (25D8)|○ (25CB)|◙ (25D9)|♂ (2642)|♀ (2640)|♪ (266A)|♫ (266B)|☼ (263C)
| 16|► (25BA)|◄ (25C4)|↕ (2195)|‼ (203C)|¶ (00B6)|§ (00A7)|▬ (25AC)|↨ (21A8)|↑ (2191)|↓ (2193)|→ (2192)|← (2190)|∟ (221F)|↔ (2194)|▲ (25B2)|▼ (25BC)
| 32|SP|!|"|#|$|%|&|'|(|)|*|+|,|-|.|/
| 48|0|1|2|3|4|5|6|7|8|9|:|;|<|=|>|?
| 64|@|A|B|C|D|E|F|G|H|I|J|K|L|M|N|O
| 80|P|Q|R|S|T|U|V|W|X|Y|Z|[|\|]|^|_
| 96|`|a|b|c|d|e|f|g|h|i|j|k|l|m|n|o
|112|p|q|r|s|t|u|v|w|x|y|z|{|||}|~|
|128|Ç (00C7)|ü (00FC)|é (00E9)|â (00E2)|ä (00E4)|à (00E0)|å (00E5)|ç (00E7)|ê (00EA)|ë (00EB)|è (00E8)|ï (00EF)|î (00EE)|ì (00EC)|Ä (00C4)|Å (00C5)
|144|É (00C9)|æ (00E6)|Æ (00C6)|ô (00F4)|ö (00F6)|ò (00F2)|û (00FB)|ù (00F9)|ÿ (00FF)|Ö (00D6)|Ü (00DC)|¢ (00A2)|£ (00A3)|¥ (00A5)|₧ (20A7)|ƒ (0192)
|160|á (00E1)|í (00ED)|ó (00F3)|ú (00FA)|ñ (00F1)|Ñ (00D1)|ª (00AA)|º (00BA)|¿ (00BF)|⌐ (2310)|¬ (00AC)|½ (00BD)|¼ (00BC)|¡ (00A1)|« (00AB)|» (00BB)
|176|░ (2591)|▒ (2592)|▓ (2593)|│ (2502)|┤ (2524)|╡ (2561)|╢ (2562)|╖ (2556)|╕ (2555)|╣ (2563)|║ (2551)|╗ (2557)|╝ (255D)|╜ (255C)|╛ (255B)|┐ (2510)
|192|└ (2514)|┴ (2534)|┬ (252C)|├ (251C)|─ (2500)|┼ (253C)|╞ (255E)|╟ (255F)|╚ (255A)|╔ (2554)|╩ (2569)|╦ (2566)|╠ (2560)|═ (2550)|╬ (256C)|╧ (2567)
|208|╨ (2568)|╤ (2564)|╥ (2565)|╙ (2559)|╘ (2558)|╒ (2552)|╓ (2553)|╫ (256B)|╪ (256A)|┘ (2518)|┌ (250C)|█ (2588)|▄ (2584)|▌ (258C)|▐ (2590)|▀ (2580)
|224|α (03B1)|ß (00DF)|Γ (0393)|π (03C0)|Σ (03A3)|σ (03C3)|µ (00B5)|τ (03C4)|Φ (03A6)|Θ (0398)|Ω (03A9)|δ (03B4)|∞ (221E)|φ (03C6)|ε (03B5)|∩ (2229)
|240|≡ (2261)|± (00B1)|≥ (2265)|≤ (2264)|⌠ (2320)|⌡ (2321)|÷ (00F7)|≈ (2248)|° (00B0)|∙ (2219)|· (00B7)|√ (221A)|ⁿ (207F)|² (00B2)|■ (25A0)|  (00A0)

Source: <https://en.wikipedia.org/wiki/Code_page_437>
