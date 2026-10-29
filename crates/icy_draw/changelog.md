# IcyDraw Changelog

## Unreleased

### Added

- ANSI music editor for `.ams` and `.mus` files and in **New**: a piano roll and
  the music string side by side, keyboard note entry, playback with a moving
  playhead, tempo and articulation changes, and a choice of `ESC [ |`, `ESC [ N`
  or `ESC [ M` when saving. Unchanged files are saved byte for byte.
- The ANSI music editor selects ranges of notes to transpose, change, copy, cut
  and paste; inserts notes anywhere with a click; changes lengths by dragging a
  note's end; plays the selection, also in a loop; and takes part in crash recovery.
- The ANSI music editor has a player under the roll, a ruler with bars of a chosen time
  signature on which a click sets where playback starts and a drag the range it plays,
  and lanes for setting changes and rests. Notes and selected ranges move sideways
  by dragging.
- The AI assistant works in the ANSI music editor: it reads and writes the music
  string, and its proposals can be listened to before they are applied.
- Opening an ANSI file that contains BEL, BS or TAB outside escape sequences
  asks whether to show them as CP437 glyphs, as PabloDraw and Moebius do, or to
  execute them as control codes (#202).
- ANSI export can write control-character glyphs with Control Representation
  Mode (`CSI 3 h`, with DoorWay for characters CRM can't show) or DoorWay mode
  (`CSI = 255 h`). The control character option only appears when the picture
  contains such characters.

### Fixed

- An idle window no longer redraws continuously. The window title was sent
  every frame, and each title update requested another frame, keeping a CPU
  core busy (#201).

## 0.6.1 - 2026-10-28

Changes since 0.6.0.

### Added

- Dedicated Move Layer tool for text-mode documents, with drag previews,
  cell-based keyboard nudging, axis-constrained dragging and cancellation.
  Drags start on the selected layer and can continue past the canvas edges;
  consecutive arrow-key nudges undo in one step. The cursor shows a grab hand
  over the selected layer and a grabbing hand while dragging.
- UTF-8 Art document mode on the start screen, in the New dialog and through
  MCP (`"type": "utf8"`), with terminal-compatible one- and two-cell graphemes,
  wide characters and combining marks.
- Grapheme-aware typing, painting, selection erase/fill, clipboard operations,
  undo/redo, resizing, layer merging and flips.
- Unicode character picker with code-point navigation, drawing-symbol shortcuts
  and background font fallback loading; the click tool can insert the chosen
  character at the caret.
- Scalable, zoom- and DPI-aware Unicode canvas rendering, with cell-filling
  block elements, shades and connected box drawing.
- UTF-8 text and ANSI export with a BOM, explicit `.utf8` file loading, and
  Unicode PNG/GIF export.

### Fixed

- Fixed IGS loop parameters split across lines with `_` being truncated during
  playback, causing invalid blit parameters and missing drawing operations.
- Restored alphabetized plugin category submenus from script metadata, with
  title sorting and description/author tooltips.
- Corrected SkyPix Amiga font bitmaps, proportional spacing and bearings,
  scanline geometry and text-mode transitions.
- Refreshed cached SkyPix and RIP graphics after pixel changes.
- Prevented queued audio patches from exhausting the memory budget and blocking
  later loads, without interrupting sounds already playing.

### Compatibility

- UTF-8 `.icy` documents use native format version 6; older readers reject
  them. Existing CP437 documents retain their encoding and bitmap rendering.
- UTF-8 text/ANSI exports include a BOM so they reopen in the correct mode.
  BOM-less files require the `.utf8` extension to opt into UTF-8; other
  BOM-less text/ANSI files retain CP437 interpretation.
- Unicode mode does not support RTL, contextual script shaping or color emoji
  layout. Glyph availability depends on font coverage.
- Legacy byte-oriented exports such as XBin and the floating-paste transparency
  toggle are unavailable for Unicode documents. Unsupported operations such as
  rotation, justification and row/column shifting report an error rather than
  damage Unicode text.

## 0.6.0 - 2026-10-10

Changes since 0.5.1.

### Added

- Native egui desktop frontend using the shared wgpu terminal renderer, with
  drawing tools, font editing, Lua plugins and Moebius-compatible collaboration.
  `icy_draw` uses it by default and `icy_draw_egui` is a compatibility alias.
- Dedicated RIPscrip, IGS, SkyPix, ATASCII, Atari ST VT52 and PETSCII editors.
- PETSCII screen modes for C64, C128, VIC-20, PET, PET 80, C16 and C128 VDC,
  with platform fonts, chunky pixels, reverse drawing and screen-mode controls.
- RIP selection, shape, text, mouse-region and palette editing; IGS command and
  property editing, fill and line patterns, BitBlit logic operations and a chip
  tune editor.
- Shared RIP/IGS animation playback with controls below the canvas.
- Nested layer groups with drag-and-drop ordering, collapse/expand, subtree
  visibility, group movement and undoable grouping and ungrouping.
- Non-destructive per-layer palette remapping and selection masks, with preview,
  disable, remove and bake controls.
- Editable TheDraw text layers with inline typing, embedded fonts, outline
  styles and adjustable letter and line spacing.
- AI drawing assistant with editor-aware tools for ANSI, fonts, RIP, IGS and
  SkyPix, image attachments and AI-assisted TheDraw font editing.
- Image import with half-block and ASCII conversion, AI previews before
  acceptance, and RIP, IGS, PETSCII and VT52 targets.
- Autosave and crash recovery for unsaved documents.
- Custom character and color ramps for the shading brush, a canvas context menu,
  attribute picker, box-frame drawing and joined outline lines.
- Matching-cell selection with Look mode, connected-only and sample-merged
  options; keyboard shortcuts and drag-and-drop placement for tags.
- Bitmap font import and export dialogs, document font slots and a paste
  transparency toggle.

### Changed

- Redesigned the editor layout, new-document choices, image import and font
  dialogs, including grouped font color variants and favorites.
- Improved canvas keyboard focus and Moebius-style editing shortcuts.
- Shared clipboard integration, export and shortcut dialogs, mouse-wheel zoom
  and updated application icons with the other desktop tools.
- Shape previews now show the intended drawing result, with translucent previews.

### Fixed

- Preserved extended attributes when loading and saving Ctrl-A, Avatar, PCBoard
  and Renegade files; improved ANSI autowrap and ANSI.SYS export behavior.
- Corrected ATASCII and VT52 load/save behavior and persisted home-computer
  screen metadata in native documents.
- Fixed paste positioning for reversed selections, opaque blanks on non-alpha
  layers and selection cleanup after cropping.
- Improved bitmap font editing focus, colors and font-size undo.
- Improved collaboration palette compatibility with Moebius clients.
- Fixed macOS Option-key input and Finder document opening, including multiple
  files.

### Compatibility

- Save as `.icy` to preserve editable layer effects, text layers and groups.
  These use native format versions 3, 4 and 5 respectively; older readers reject
  unsupported versions. Documents without these features retain their existing
  format version.
- Flat exports contain the visible result, not editable layer effects or group
  structure. Groups are pass-through containers rather than isolated blends.
