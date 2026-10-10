# IcyDraw Changelog

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
