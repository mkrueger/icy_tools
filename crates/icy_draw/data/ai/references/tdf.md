# TheDraw text-art fonts (TDF)

TDF glyphs are grids of CP437 character cells, not bitmap pixels. A collection
contains several fonts; edit only the selected font. Each font has 94 glyph
slots: ASCII 33 (!) through 126 (~), inclusive. There is no glyph slot for
ASCII 32 (space); word spacing is a font setting, not a drawn space glyph.
The glyph named ~ is the last ordinary printable glyph, NOT a transparency
or hard-blank command.

Each glyph can be up to 30 cells wide and 12 rows high. Different widths are
allowed. Keep baseline, cap height, stroke weight, palette and spacing coherent
across uppercase, lowercase, digits and punctuation.

## Font types

- Color: artwork carries each cell's DOS foreground (0..15), background
  (0..7) and blink flag. Ordinary spaces are transparent when loaded from TDF;
  background-colored spaces cannot be used as opaque artwork.
- Block: CP437 artwork without stored colors. It uses the caller's colors.
- Outline: store semantic placeholder codes rather than final box-drawing
  characters. The same glyph can render in 19 line/block styles (indices 0..18).
  The style is chosen at rendering time, not baked into the glyph.

## Empty cells and the extra character

ASCII space (char_code 32) is a transparent/skip cell: advances the cursor
without drawing. Trailing empty cells do not define visible artwork.
The extra hard-blank character (char_code 255, Shift+Space in the editor) is
shown by the editor's special font as a low, tilde-like editing marker. This
marker is NOT the literal ~ character (126). In Color/Block
fonts it serializes as 0xFF and draws an actual blank in display mode; use it
to preserve occupied blank cells and right/bottom extents. Do not confuse its
editing appearance with literal char_code 126 (~). In Outline fonts prefer
O or @ for occupied blank cells; they have explicit outline semantics.

TDF reserves byte 0 as a terminator, byte 13 as newline and & as an internal
end marker. They are not ordinary artwork cells. An & end marker is visible
in edit mode but disappears without advancing in display mode; do not use it
to pad a glyph or to draw a literal ampersand. A glyph named & is still allowed;
construct its artwork using other cells.

## Outline vocabulary

Store these ASCII codes in the cell grid:

- A/B: top/bottom horizontal edges.
- C/D: left/right vertical edges.
- E/F: outer top-left/top-right corners.
- G/H: inner top-left/top-right corners.
- I/J: inner bottom-left/bottom-right corners.
- K/L: outer bottom-left/bottom-right corners.
- M/N: right-/left-facing junctions.
- O: occupied hole (draws a blank, not a skip).
- P/Q: reserved blank-producing placeholders in the current style table.
- @: fill marker, displayed as a blank.
- &: end marker, not a visible cell in display mode.

The editor keys F1..F10 insert A..J. Digits 1..5 insert K..O;
6 inserts @, 7 inserts &, 8 inserts the extra hard blank.
`icy_tdf_info` returns the actual style mappings. Inspect previews in more
than one outline style, including a line style and a block style, before
calling an outline font complete.

## Assistant workflow

Use icy_tdf_info, icy_read_tdf_glyphs and icy_write_tdf_glyphs, not bitmap
font tools or ANSI canvas tools. Read existing glyphs first. Writes replace
whole glyphs atomically within the draft; null rows explicitly remove a glyph.
For a full set, cover all 94 slots in a few coherent batches, not just A-Z.
Use the info tool's missing_codes to verify exact coverage and preview pages
with icy_preview_tdf. Fix defects in the draft; only User Apply changes the
live font. Preserve existing work unless replacement was requested.
