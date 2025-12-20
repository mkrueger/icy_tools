# BitFont Editor - MCP API

The BitFont Editor allows editing bitmap fonts used for ANSI art rendering.
These are monospace pixel fonts where each glyph is a fixed-size bitmap.

## Supported Formats

| Format | Extension         | Description                            |
| ------ | ----------------- | -------------------------------------- |
| PSF    | `.psf`, `.psfu`   | PC Screen Font (Linux console)         |
| YAFF   | `.yaff`           | Yet Another Font Format (human-readable) |
| Raw    | `.fXX`            | Raw bitmap fonts (XX = height, e.g. `.f16`) |

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
