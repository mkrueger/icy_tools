# ATASCII character art in Icy Draw

Original implementation notes based on `icy_engine`'s ATASCII loader/saver,
character tables and Icy Draw's Atari screen profiles.

Use `icy_canvas_info` and `icy_read_region` before writing. ATASCII is an Atari
8-bit character encoding, not CP437. Use the active Atari font; do not assume
ANSI block/shade characters have equivalent glyphs. `char_code` writes a native
glyph code (0..255), not a wire control command. The upper half of the glyph
codes provides inverse video. Reads expose raw `glyph_codes` and `font_pages`
alongside an approximate Unicode view.

`icy_canvas_info` includes the real font name, glyph dimensions, display cell
dimensions and tool capabilities. `icy_read_canvas_glyphs` reads up to all 256
actual font bitmaps, including inverse glyphs. Hex rows run top to bottom;
bit 7 is the leftmost pixel. A set bit uses the shared foreground, a clear bit
the shared background. Do not guess glyph shapes from Unicode labels.

The standard ANTIC text screen has 40 columns; XEP80 text has 80. Read the
actual dimensions/machine mode rather than assuming an ANSI 80x25 screen.
ATASCII uses a shared foreground/background palette and one font for the
screen. Do not paint different per-cell colors; use native inverse glyphs for
contrast and the editor's palette controls for global colors.

For a picture conversion, simplify to a two-tone silhouette and major light/dark
regions on the existing grid. Prefer icy_convert_reference_image with mode=full
for a local luminance-to-glyph first pass, then inspect with icy_preview_canvas.
Preserve proportions using the display cell size,
then match edges and textures to actual glyph bitmaps. Write native `char_code`
batches, omitting `fg` and `bg`. Screen dimensions, resolution, font size and
font bitmaps cannot be changed by canvas tools. Read back the result and only
claim a draft was created when successful writes changed it. A description of
the intended picture alone is not a conversion; report rejected writes or an
unchanged draft explicitly.

Use ordinary text only when it maps to the active encoding. For line art,
copy native glyph codes from existing cells or the character picker. Write
coherent bounded batches, preserve untouched cells, then review Apply/Discard.

`.ata` and `.xep` files are terminal streams decoded into read-only screen
snapshots. `.xep` selects the 80-column loader, matching normal document import.
ATASCII line endings/control bytes are not UTF-8 and must not be imported as
plain text. Reference parsing does not send commands to a BBS.
