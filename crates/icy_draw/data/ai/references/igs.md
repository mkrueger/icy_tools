# IGS graphics and static AI drafts

Original implementation-backed notes for Icy Draw's Atari ST Instant Graphics
and Sound editor, based on icy_parser_core IGS and icy_engine VDI rendering.
Start with icy_igs_info, icy_igs_api and icy_read_igs_items. This is an ordered
pixel-graphics/state stream with mixed VT52 text, not ANSI character cells.

Low resolution: 320x200, 16 pens; medium: 640x200, 4 pens; high: 640x400,
2 pens. Preserve the current mode unless changing it is explicitly requested.
Use only active logical pens; RGB register components are 0..7. Coordinates
are pixels unless scaling is enabled; G#g>1: uses virtual 0..10000 coordinates.
Aspect and resolution matter: do not assume square pixels in medium mode.

Use separate complete command lines beginning G#. Parameters are decimal,
comma-separated; most commands end with a colon. W text ends with @ instead.
State persists and command ordering matters. Read existing state and items
before insertion. Set line/fill/text pens deliberately; changing resolution,
initializing or clearing can destroy previous rendered art.

Common forms (ASCII examples):
- G#R>0,2: selects low resolution/IG default palette (1 medium, 2 high).
  The second field is palette mode: 0 unchanged, 1 desktop, 2 IG, 3 VDI.
- G#C>1,2: line pen 2. Pen types: 0 marker, 1 line, 2 fill, 3 text.
- G#S>2,7,0,0: set palette register 2 to red.
- G#A>1,1,1: solid fill with border; type 0 hollow, 1 solid, 2 pattern,
  3 hatch, 4 user pattern. Pattern indices 1..24, hatch 1..12, user 0..7.
- G#M>1: replace drawing mode (2 transparent, 3 XOR, 4 reverse transparent).
- G#L>10,10,100,80: line; G#D>120,90: draw from previous endpoint.
- G#B>20,20,100,70,0: square-cornered box.
- G#O>100,80,20: circle; G#Q>100,80,30,15: ellipse.
- G#z>3,10,10,50,30,80,10: polyline; G#f>3,10,10,50,30,80,10:
  filled polygon. At most 128 points.
- G#E>0,9,0: normal 9-point text, no rotation.
- G#W>20,40,Hello@: positioned VDI text, not terminal text.

Verify syntax against canonical_source returned for existing items. This guide
is not the full protocol. AI drafts accept a bounded static subset: shapes,
pens, patterns, text, drawing/scaling modes, clear/resolution and safe cursor/
VT52 text operations. Text size is 1..48; radii <=640; literal coordinates
0..10000; random/loop parameters are not accepted for new static commands.
Use ASCII source or source_hex for native byte strings/VT52 escapes, never
Unicode approximations. Read source_hex to preserve exact existing byte data.

Loops, delays, palette rotations, sound/MIDI, input, queries, macros, flow
control, memory/file operations and malformed/unsafe fragments are read-only:
keep them unchanged and in order. Static previews OMIT them entirely; they
do not run loops, wait, play sound, request input or touch external assets.
Omitted state/loop effects mean the static preview may differ from playback.
Read the omitted count and tell the user about that limitation.

For illustrations, compose large shapes first, use a restrained palette, set
state before dependent shapes and add readable text last. Picture attachments
can guide geometry, but the character-canvas raster converter does not work
in IGS. Use icy_preview_igs to inspect and refine, at most three previews per
turn. All edits remain Apply/Discard drafts; applying is one undo step and
rejects stale documents or unfinished editor operations.
