# PETSCII character art in Icy Draw

Original implementation notes based on `icy_engine`'s SEQ loader/saver,
`petscii_screen_code`, `petscii_charset`, and Icy Draw's screen profiles.
This describes the editor, not universal behavior of every Commodore terminal.

Start with `icy_canvas_info`, then read the region being changed. PETSCII art
uses the active machine's screen codes and font, not CP437 block characters.
The tool's `char_code` is a native **screen glyph code**, not a PETSCII wire
byte or terminal control sequence. Codes 128..255 are inverse glyphs.
Read `glyph_codes` and `font_pages`; the Unicode text is only an approximation.

The uppercase/graphics and lowercase/uppercase character sets differ.
Icy Draw maps ordinary text with the current set: in the graphics set, A/a
use screen code 1; in the lowercase set, a uses 1 and A uses 65.
The C128 VDC can choose the set per cell via its existing font page.
Assistant writes preserve those font pages; they do not switch machine/set.
For graphical glyphs without a reliable Unicode mapping, copy native codes
from existing art or the editor's character picker.

Do not assume every screen is C64 40x25: VIC-20 is 22x23; PET80 and C128 VDC
are 80x25; other supported machines typically use 40x25. Read actual dimensions.
Use the actual machine palette: C64/VDC color numbers are not ANSI indices.
VIC-20 text uses its first eight colors; the C16 has its own larger palette.
The background is shared by the screen; do not create per-cell backgrounds.
Use the editor's screen/palette controls for global background changes.

Batch cells/text/rectangles, preserve existing art and review Apply/Discard.
Use inverse native codes rather than ANSI color swaps for reverse characters.
`.pet` and `.seq` references are parsed into read-only screen snapshots with
raw glyph codes, font pages, encoding, machine mode, palette and attributes.
Importing a reference neither sends terminal controls nor changes the document.
