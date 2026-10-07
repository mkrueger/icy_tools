# PETSCII / ATASCII / VT52 art workflow

Original Icy Draw workflow for the active retro character editor.

1. Start with icy_canvas_info: identify encoding, machine/resolution, dimensions,
   palette and font/character-set constraints. The user's selected editor wins;
   never change it into a generic ANSI/CP437 target.
2. Read the target region. Use glyph_codes/font_pages as authoritative native
   glyph data; Unicode text is an approximation, particularly for PETSCII.
   Inspect actual shapes with icy_read_canvas_glyphs. For pictures, prefer
   icy_convert_reference_image with mode=full; preserve shared colors and banks.
3. Plan readable titles, hotkeys, outlines and shading in the actual character
   grid. Copy native glyphs from existing art or the character picker.
4. PETSCII: respect the current machine and upper/graphics versus lower set.
   ATASCII: use inverse codes for contrast, not independent cell colors.
   VT52: use Atari ST glyphs and the resolution's actual palette.
5. Preserve machine-wide font/background/palette settings and existing font
   pages. Do not paste terminal escapes into cell-writing tools.
6. Use bounded multi-line text, rectangles and cell batches. Avoid one tool
   call per glyph/cell. Preserve untouched art unless replacement was requested.
7. Inspect the draft for alignment, native glyphs, color constraints and command
   coverage. Use icy_preview_canvas for rendered feedback and refine within the
   three-pass limit. Describe the result; Apply is required and is one undo step.
