# SkyPix graphics and static AI drafts

Original implementation-backed notes for Icy Draw's Amiga SkyPix editor,
based on icy_parser_core SkyPix and icy_engine Amiga graphics rendering.
Read icy_skypix_info, icy_skypix_api and icy_read_skypix_items before editing.
This is an ordered graphics/state stream plus CP437 terminal text, not ANSI
cells, RIP base-36 or IGS commands.

The canvas is always 640x200 pixels, with 8 or 16 colors. The editor displays
vertical pixels at twice their raw height; use that aspect ratio for composition.
Coordinates are pixels: X 0..639, Y 0..199. Palette entries are 12-bit RGB,
R + (G << 4) + (B << 8), packed as decimal 0..4095 (red in the LOW nibble),
and all 16 entries must be supplied.
Display mode changes reset the palette; preserve the user's mode unless asked.

Wire commands use actual ESC, then [, command number and semicolon-separated
decimal parameters, ending !. In JSON source, encode ESC as \u001b. Do NOT
write the literal characters "<ESC>". source_hex is exact native bytes.
Comment/font strings use a second ! terminator. Read source_hex to preserve
existing spellings and CP437 bytes, not Unicode look-alikes.

Common forms (ESC below means the actual byte 1B):
- ESC[15;3! sets foreground/drawing pen A to 3; ESC[18;0! sets background B.
- ESC[8;10;10! moves the graphics pen, not the text cursor.
- ESC[2;100;60! draws a line from the previous pen position.
- ESC[1;40;30! sets a pixel.
- ESC[4;20;20;100;80! fills a rectangle.
- ESC[5;100;80;30;15! outlines an ellipse; 13 instead of 5 fills it.
- ESC[3;1;50;50! color flood fill; mode 0 stops at outline-colored pixels.
- ESC[17;1! selects 8 colors; mode 2 selects 16.
- ESC[19;20;40! positions the text cursor and drawing pen in PIXELS, then ordinary CP437 text.
- ESC[10;8!Topaz.font! selects a bundled font; ESC[10;0! restores default.
- ESC[12! resets the palette.
- ESC[6;10;10;30;20! captures a local brush (width/height, not endpoints).
- ESC[7;0;0;100;100;30;20;192;255! copies that brush with all planes.

Read bundled_fonts from info. Only available bundled font/name-size pairs are
accepted; no disk fonts are opened. Non-default text is transparent/JAM1;
carriage return advances to the next line, linefeeds are ignored. Default text
uses ANSI-style cursor/background behavior. Keep labels readable and avoid
font switches unless they help the requested design.

Allowed edits: bounded shapes, pens, palette/display mode, comments, bundled
fonts, local brush capture/copy, CP437 text and safe ANSI SGR/cursor/erase
sequences. Brush copies require a previously captured local source and supported
copy/all-plane minterm 192/mask 255. Bounds and color depth are validated.
Plain text cannot introduce bell, escape, query or other runtime controls.

Audio, delays, transfers, controller returns, gadgets, mode terminators,
unavailable fonts, unsupported brush modes and malformed/unsafe fragments are
read-only. Preserve them unchanged and ordered. Static previews OMIT them,
never play sound, wait, transfer files or send input. Omitted operations can
affect playback; explain that the preview is not playback-equivalent.

Compose broad geometry first, set state before dependent drawing commands,
then add readable text and restrained details. Picture references may guide
geometry, but the character-canvas raster converter does not work in SkyPix.
Use icy_preview_skypix to inspect and correct the static drawing, at most
three previews per turn. All edits remain Apply/Discard drafts; applying is
one undo step and rejects stale documents or unfinished user operations.
