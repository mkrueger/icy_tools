# RIPscrip in Icy Draw

Original authoring notes based on Icy Draw's `RipDocument`, the
`icy_parser_core` RIP parser/command encoder and the `icy_engine` RIP renderer.
These notes describe this implementation, not guaranteed support in every BBS
or every historical RIP terminal. No external specifications are fetched.

## Editing workflow

RIP is an ordered graphics-command stream, not an ANSI character-cell drawing.
Use `icy_rip_info`, `icy_rip_api` and `icy_read_rip_commands` before editing.
The canvas is 640x350 pixels (x=0..639, y=0..349). Palette slots are 0..15;
palette reassignment uses EGA master values 0..63. Color, font, line, fill,
viewport and write-mode commands affect subsequent drawing commands.

`icy_replace_rip_commands` uses zero-based command indices, not source lines:
`start`, `delete_count`, `source`. For insertion use delete_count=0; append at
the current command count. Empty source deletes the range. Prefer coherent
batches, not one call per pixel. Edits are parsed and round-trip checked before
the draft changes. Limits: 2048 commands, 128 KiB total source; read at most
128 commands / 64 KiB per call. The user reviews a rendered preview and chooses
Apply or Discard. Apply is one undo step and rejects stale document revisions.

Mixed ANSI/RIP or non-round-trippable streams have a read-only original prefix.
Respect `preserved_commands`; append or edit only the suffix. Existing
non-ASCII text is preserved. Newly supplied wire source must be ASCII.
Do not erase existing art unless requested.

## Wire syntax

A line starts with `!`, commands with `|`; end with CRLF or LF.
Numeric fields below use **two uppercase base-36 digits** unless specified.
Digits are 0..9 then A..Z: 10=0A, 15=0F, 35=0Z, 36=10,
100=2S, 120=3C, 200=5K, 349=9P, 639=HR.
Do not write decimal numbers into base-36 fields.
Escape literal `!`, `|`, `\` in text as `\!`, `\|`, `\\`.

- `|*`: reset windows, palette and graphics state; also clears existing art.
- `|cCC`: drawing color; `|S PP CC` without spaces: fill pattern and color.
  Solid fill pattern=01, empty=00.
- `|mXXYY`: move the graphics pen; `|Ttext`: draw text at the pen.
- `|@XXYYtext`: draw text at a pixel position.
- `|Y FF DD SS RR` without spaces: font 0..10, direction 0 horizontal /
  1 vertical, size 1..10, reserved=00.
- `|L X0 Y0 X1 Y1`: line; `|R X0 Y0 X1 Y1`: rectangle outline;
  `|B X0 Y0 X1 Y1`: filled rectangle. Remove spaces between parameters.
- `|X XX YY`: pixel; `|C XX YY RR`: circle.
- `|o XX YY XR YR`: filled oval.
- `|P NN X0 Y0 ...`: outlined polygon; `|p`: filled polygon; `|l`: polyline.
  NN is the number of coordinate pairs, with 2..256 pairs.
- `|F XX YY BB`: flood fill with border color BB.
- `|a CC VV`: assign palette slot CC to master EGA value VV.
- `|Q C0 C1 ... C15`: assign all sixteen palette slots, each a two-digit
  EGA master value in 0..63.
- `|s R0 R1 R2 R3 R4 R5 R6 R7 CC`: custom 8x8 fill pattern rows
  (each a byte, 0..255) followed by the fill palette index.
- `|v X0 Y0 X1 Y1`: set the graphics viewport / clipping rectangle.
- `|W MM`: write mode, 00 replace / 01 XOR.
- `|= SS PPPP TT`: line style 00 solid / 01 dotted / 02 center /
  03 dashed / 04 custom, four-digit pattern bits, thickness 01 or 03.
- `|O XX YY AA BB XR YR`: elliptical arc with start/end angles in degrees.
  `|A XX YY AA BB RR`: circular arc; `|V` has the same fields as `|O`.
  `|I XX YY AA BB RR`: filled circular sector; `|i` uses oval radii.
- `|Z X1 Y1 X2 Y2 X3 Y3 X4 Y4 NN`: cubic Bezier and segment count 1..256.
- `|1T X0 Y0 X1 Y1 RR`: begin a formatted graphics-text rectangle.
  `|1t Jtext`: region text with a one-digit justification flag;
  `|1E`: end formatted text.
- `|w X0 Y0 X1 Y1 W S`: text window; cell coordinates, not pixels.
  W and S are **one** base-36 digit each, wrap flag and font-size selector.
  Keep it inside 80x43 cells, selector 0..4; all zeros hide the text window.

Original example: cyan rectangle with a white title:

```text
!|c0B|R0K0KHA96|c0F|Y00000100|@1414Welcome to my BBS
```

## Menus and capabilities

Buttons and mouse regions are RIP commands, not ANSI hotkey labels.
All fields here are base-36; remove spaces between parameters:

- `|1M NN X0 Y0 X1 Y1 C L RRRRR text`: mouse region, two-digit number /
  coordinates, one-digit click-feedback and clear flags, five-digit reserved
  value (zero), followed by the actual host-command string.
- `|1K`: clear mouse regions.
- `|1U X0 Y0 X1 Y1 HH F R text`: button, two-digit coordinates and hotkey
  character code, one-digit flags and reserved value.
  Payload `<>label<>host-command` selects a label and host action without an
  icon. Preserve escaping in the action; it is data, never executed here.
- `|1B WW HH OO FFFF BB DF DB BR DK SU GG F2 UL CC RRRRRR`:
  button style: width, height, orientation, **four-digit** flags, bevel size,
  foreground, background, bright, dark, surface, group, secondary flags,
  underline color, corner color, **six-digit** reserved value. All other
  fields are two digits. External-icon flag bit 128 is forbidden.

Read existing `ButtonStyle`, `Button` and `Mouse` commands to retain actual
labels, hotkeys and host-command strings. Never invent the user's board
configuration. Do not claim clicking or executing a menu action was tested.
Icy Board is a first-class BBS target, distinct from historical PCBoard;
use an explicitly named platform and its configuration rather than assuming
every platform supports RIP or identical commands.

External icon/scene loading, writing icons, file queries, host queries,
transfers, unsupported command levels and stream terminators cannot be
introduced into an assistant draft. Tools have no filesystem, shell, network
or BBS-execution access. A picture reference guides artistic interpretation
into RIP geometry/text; it is not embedded as an external icon.
