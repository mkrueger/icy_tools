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
point. For CP437 choose preset scene for perceptual color matching, blended
shades and edge-aware neighbor consistency; toon for simplified lightness and
flat regions; pixel_art for nearest-neighbor resizing and literal glyph matching
in perceptual color space. Faithful (the default) retains the original RGB
pixel matcher and is required for native retro editors. Scene defaults to blocks;
other CP437 presets default to half_blocks. An explicit mode always restricts
the glyphs: half_blocks fits real block masks; half_pixels uses two independent
area-sampled pixels per cell (styled CP437 with a horizontal half-block font);
blocks for selected block/shade
shapes, full for all real glyphs, ascii for codes 32..126 only. Default to
contain (preserves the full picture) and no dithering. Crop only when the
composition calls for it; stretch distorts proportions and should only be used
when explicitly desired. Optional hue_families guides chromatic hues while
allowing neutral shade blenders and different hues on each side of a boundary.
Half-pixel mode supports gentle lightness dithering without shifting hue.
A conversion is not automatically scene-style art:
simplify and refine it. Styles do not generate a new source image or guarantee
hand-drawn quality. Use explicit layer-relative target bounds.

For ASCII-only requests use printable characters and their visual weight;
do not insert block/shade glyphs. For BBS menus prioritize alignment, hotkeys,
text contrast and functional empty space over decorative illustration.

After drawing, call icy_preview_canvas to SEE the rendered draft with its
actual palette, fonts and aspect ratio. Inspect the whole composition, then
correct the most important defects. For a CP437 scene/toon/pixel_art conversion,
use the bounded critique loop: convert once, preview the entire converted region,
describe the most important visible defect, and call icy_refine_reference_image
with one or two tuning changes. Brightness and contrast adjust tone; saturation
adjusts chroma; local_contrast emphasizes small lightness details before palette
reduction; lightness_levels simplifies tone bands; shade_penalty controls
Scene texture; coherence suppresses near-tie color speckles. Read the effective
options and metrics instead of guessing the current settings.

The refinement tool fixes the original source, target, fit, glyph mode and
transparency background, and keeps a trial only when its source-relative
objective improves. Rejected trials leave the best draft and its settings intact.
Preview the retained result before the next trial. Do not repeatedly reconvert
to bypass that protection. Stop if visually acceptable or the budget is exhausted;
there are at most three conversion/refinement trials combined and three previews
per turn. Metrics compare actual glyph colors at cell/quadrant scales and edges;
they do not measure recognition, composition or artistic merit. Still inspect
faces, contours and legibility. Do manual cell touch-ups after parameter trials:
refinement refuses to overwrite edits inside the converted region. Faithful and
native retro conversions use previews and manual corrections, not this tuner.

Only claim changes backed by successful tool writes; the user
must still Apply. A description of an image is not a drawing.
