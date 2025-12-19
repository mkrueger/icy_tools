# BitFont Editor - MCP API

The BitFont Editor allows editing bitmap fonts used for ANSI art rendering. These are monospace pixel fonts where each glyph is a fixed-size bitmap.

## Supported Formats

| Format | Extension | Description |
|--------|-----------|-------------|
| PSF | `.psf`, `.psfu` | PC Screen Font (Linux console) |
| YAFF | `.yaff` | Yet Another Font Format (human-readable) |
| Raw | `.fXX` | Raw bitmap fonts (XX = height, e.g. `.f16`) |

## Font Structure

A bitmap font consists of:
- **Glyph dimensions**: Width × Height in pixels (e.g., 8×16)
- **Glyph count**: Usually 256 (full CP437) or 512 (with bold variants)
- **Character range**: First and last character codes
- **Bitmap data**: 1 bit per pixel, stored row by row

## MCP Tools

### `bitfont.list_chars()`

Returns an array of character codes that have glyphs defined.

**Response:**
```json
{
  "chars": [0, 1, 2, ..., 255],
  "count": 256
}
```

### `bitfont.get_char(code)`

Get a glyph bitmap as base64-encoded data.

**Parameters:**
- `code` (integer): Character code (0-255 typically)

**Response:**
```json
{
  "code": 65,
  "char": "A",
  "width": 8,
  "height": 16,
  "bitmap": "base64..."
}
```

**Bitmap format:**
- 1 bit per pixel
- Rows stored top to bottom
- Bits stored left to right within each byte
- Row padding to byte boundary

Example for 8×16 font: 16 bytes per glyph (8 bits wide = 1 byte per row × 16 rows)

### `bitfont.set_char(code, data)`

Set a glyph bitmap from base64-encoded data.

**Parameters:**
- `code` (integer): Character code
- `data` (object):
  ```json
  {
    "width": 8,
    "height": 16,
    "bitmap": "base64..."
  }
  ```

**Response:**
```json
{
  "success": true
}
```

## Status Fields

When in BitFont editor mode, `get_status()` returns:

```json
{
  "editor": "bitfont",
  "file": "/path/to/font.psf",
  "dirty": false,
  "glyph_width": 8,
  "glyph_height": 16,
  "glyph_count": 256,
  "first_char": 0,
  "last_char": 255,
  "selected_char": 65
}
```

## Bitmap Encoding Details

### Encoding a glyph (8×16 example)

```
Pixel grid for 'A':
........  = 0x00
...##...  = 0x18
..#..#..  = 0x24
..#..#..  = 0x24
.#....#.  = 0x42
.#....#.  = 0x42
.######.  = 0x7E
.#....#.  = 0x42
.#....#.  = 0x42
.#....#.  = 0x42
........  = 0x00
...      (remaining rows)
```

Each row is one byte (for 8-pixel width). The bitmap is these bytes concatenated and base64 encoded.

### For wider fonts (e.g., 16×16)

Each row takes 2 bytes. Total: 32 bytes per glyph.

## Workflow Example

### Modifying a character

```
1. load_document("/fonts/myfont.psf")
2. get_status()
   -> {"glyph_width": 8, "glyph_height": 16, ...}

3. bitfont.get_char(65)  // Get 'A'
   -> {"bitmap": "ABgkJEJCfkJCQgA=", ...}

4. // Modify bitmap data...

5. bitfont.set_char(65, {
     "width": 8,
     "height": 16,
     "bitmap": "ABgkJEJCfkJCQgA="  // Modified
   })

6. save()
```

### Creating a new font

```
1. new_document("bitfont")
   // Creates 8×16 font with 256 empty glyphs

2. // Set each character...
   bitfont.set_char(65, {...})  // A
   bitfont.set_char(66, {...})  // B
   ...

3. save()  // Will prompt for filename
```

## Tips

- **Preview**: Use icy_draw's preview mode to see how the font renders text
- **CP437**: Standard ANSI art fonts use Code Page 437 character mapping
- **Aspect ratio**: Classic DOS fonts are 8×16 (1:2 ratio) or 8×8 (square)
- **Undo/Redo**: All glyph changes support undo/redo

## Common Character Codes

| Code | Char | Description |
|------|------|-------------|
| 0-31 | ☺☻♥... | Control characters (rendered as symbols in CP437) |
| 32 | (space) | Space |
| 48-57 | 0-9 | Digits |
| 65-90 | A-Z | Uppercase letters |
| 97-122 | a-z | Lowercase letters |
| 176-178 | ░▒▓ | Shading blocks |
| 179-218 | │┤... | Box drawing |
| 219-223 | █▄▌▐▀ | Block elements |
