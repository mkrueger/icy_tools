# ANSI format and drawing rules

Always inspect the active canvas, encoding, palette, font and display cell size.
The current document wins over generic DOS assumptions. CP437 ANSI, printable
ASCII and Unicode are different targets. Do not substitute Unicode look-alikes
for CP437 glyphs. Native byte codes address the document font; inspect custom
fonts with icy_read_canvas_glyphs rather than assuming standard shapes.

Classic DOS ANSI has 16 foreground colors and 8 non-blinking backgrounds;
iCE allows 16 backgrounds. Use the document's actual palette and blink mode.
Canvas tools do not resize the screen, replace fonts or change global colors.
Preserve unrelated cells and layers. Never paste terminal escape sequences
into cell tools. In Unicode documents, write real Unicode text, not CP437 bytes;
the local converter currently supports byte-font documents only.

Block construction: in a standard CP437 font, code 223 (upper half block) uses
foreground above and background below; code 220 reverses that split. Code 219
is solid foreground; space is solid background. Codes 221/222 are left/right
halves. Codes 176/177/178 mix foreground and background at increasing coverage:
they are patterns, not extra palette colors. With an 8x16 font, half cells are
roughly square before display correction. Use the actual display aspect ratio.

For illustrations, establish a readable silhouette and composition in neutral
values, then large flat-color regions. Choose a small coherent palette and
light direction; correct proportions before adding shadows, highlights or
texture. Use shades sparingly at transitions, not as random noise covering
every surface. Keep intentional flat regions, connected contours and negative
space; remove isolated speckles, accidental outlines and unreadable detail.
Faces need readable feature spacing more than fine texture.

For picture conversion, use icy_convert_reference_image as a local starting
point: half_blocks for clean color masses, blocks for selected block/shade
shapes, full for all real glyphs, ascii for codes 32..126 only. Default to
contain (preserves the full picture) and no dithering. Crop only when the
composition calls for it. A conversion is not automatically scene-style art:
simplify and refine it. Use explicit layer-relative target bounds.

For ASCII-only requests use printable characters and their visual weight;
do not insert block/shade glyphs. For BBS menus prioritize alignment, hotkeys,
text contrast and functional empty space over decorative illustration.

After drawing, call icy_preview_canvas to SEE the rendered draft with its
actual palette, fonts and aspect ratio. Inspect the whole composition, then
correct the most important defects. Use at most three preview/conversion
passes per turn. Only claim changes backed by successful tool writes; the user
must still Apply. A description of an image is not a drawing.
