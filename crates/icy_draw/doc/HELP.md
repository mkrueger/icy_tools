# icy_draw MCP API

icy_draw is a modern ANSI/ASCII art editor with support for multiple editor modes:

- **ANSI Editor** - Create and edit ANSI/ASCII art with layers, colors, and effects
- **Animation Editor** - Script Lua-based ANSI animations
- **BitFont Editor** - Edit bitmap fonts (PSF, YAFF, FXX formats)
- **CharFont Editor** - Edit TDF character fonts

## MCP Tools Overview

### General Tools

| Tool                 | Description                                                                          |
| -------------------- | ------------------------------------------------------------------------------------ |
| `get_help(editor?)`  | Get documentation. Without parameter: this overview. With `animation` or `bitfont`: editor-specific docs |
| `get_status()`       | Get current editor state, open file, dimensions, errors                              |
| `new_document(type)` | Create new document. Types: `ansi`, `animation`, `bitfont`, `charfont`               |
| `load_document(path)` | Open a file                                                                         |
| `save()`             | Save current document                                                                |
| `undo()`             | Undo last action                                                                     |
| `redo()`             | Redo undone action                                                                   |

### Animation Editor Tools

| Tool                                       | Description                                          |
| ------------------------------------------ | ---------------------------------------------------- |
| `animation.get_text(offset?, length?)`     | Get Lua script text. Without params: entire script   |
| `animation.replace_text(offset, length, text)` | Replace text in script at byte offset            |
| `animation.get_screen(frame)`              | Get rendered frame as ANSI text                      |

### BitFont Editor Tools

| Tool                       | Description                   |
| -------------------------- | ----------------------------- |
| `bitfont.list_chars()`     | List all glyph codes in the font |
| `bitfont.get_char(code)`   | Get glyph bitmap as base64    |
| `bitfont.set_char(code, data)` | Set glyph bitmap from base64 |

## Status Response Format

```json
{
  "editor": "ansi" | "animation" | "bitfont" | "charfont",
  "file": "/path/to/file.ext" | null,
  "dirty": true | false,
  ...editor-specific fields
}
```

### Animation-specific status fields

- `text_length`: Length of Lua script in bytes
- `frame_count`: Number of rendered frames
- `errors`: Array of script errors (empty if none)
- `is_playing`: Whether animation is playing
- `current_frame`: Current frame number

### BitFont-specific status fields

- `glyph_width`: Width of glyphs in pixels
- `glyph_height`: Height of glyphs in pixels
- `glyph_count`: Number of glyphs in font
- `first_char`: First character code
- `last_char`: Last character code

## Workflow Examples

### Creating an Animation

```text
1. new_document("animation")
2. animation.replace_text(0, 0, "local buf = new_buffer(80, 25)\n...")
3. get_status() -> check for errors
4. animation.get_screen(1) -> preview first frame
5. save()
```

### Editing a BitFont

```text
1. load_document("/path/to/font.psf")
2. get_status() -> get dimensions
3. bitfont.get_char(65) -> get 'A' glyph
4. bitfont.set_char(65, "base64...") -> update 'A'
5. save()
```

## Error Handling

All tools return errors in a consistent format:

```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

Common error codes:

- `WRONG_EDITOR`: Tool called in wrong editor mode
- `INVALID_PARAM`: Invalid parameter value
- `FILE_ERROR`: File operation failed
- `SCRIPT_ERROR`: Lua script error (animations)
