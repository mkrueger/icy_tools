# IcyView Changelog

## 0.9.10 - 2026-10-29

Changes since 0.9.9.

### Changed

- `.ans` and `.ansi` files show BEL, BS and TAB as their CP437 glyphs, as
  PabloDraw and Moebius do, in the preview and in thumbnails. Art drawn with
  them, such as TDC9.ans (#202), relied on it. Other text files keep the
  controls.
- ANSI music follows the CTerm manual: notes play at the right pitch (`O4 C`
  used to sound two octaves low), tempo, octave, length and articulation carry
  over between music strings, dotted notes and pauses last 1.5 times as long,
  lower case commands work, and `CSI = Ps M` switches the music introducers.
- Control Representation Mode (`CSI 3 h`) and DoorWay mode (`CSI = 255 h`)
  are supported, so files using them to show control characters display
  correctly.

### Fixed

- Avatar, PCBoard, Ctrl-A and Renegade files use their own parsers in the
  preview, while seeking and in thumbnails, and binary formats keep their
  SAUCE information.
- SkyPix pictures keep their Amiga font glyphs and spacing, and refresh
  correctly after graphics changes.
- Scrolling text (`CSI S`/`T`, inserting and deleting lines, line feeds on the
  bottom row) redraws correctly.
- Damaged or hostile files are handled safely: oversized OSC, DCS and APC
  strings, inline images and font loads are checked, and macro expansion is
  bounded.

## 0.9.9 - 2026-10-10

Changes since 0.9.1.

### Added

- Native egui frontend using the shared wgpu CRT renderer. `icy_view` uses it
  by default; `icy_view_egui` is a compatibility alias and the previous frontend
  remains available with the `legacy-ui` feature.
- Responsive list, masonry thumbnail grid and full-preview browsing, with
  navigation history, filtering, sorting and background loading.
- Ratings, file information, places, quick open and a hideable minimap.
- Download original files from Sixteen Colors and retry failed fetches.
- Image previews for PCX, Amiga IFF/ILBM/PBM and BASIC BSAVE screen dumps,
  alongside common image and Sixel formats.
- REXPaint document viewing with all layers.
- TheDraw and FIGlet font previews with editable sample text, spacing, colors,
  outline styles and glyph tables.
- Font specimens for Windows raster fonts, raw DOS fonts, Amiga fonts including
  ColorFonts, and Borland BGI stroke fonts.
- Tracker playback for MOD, S3M, XM and IT, plus Reality AdLib Tracker RAD music,
  with module information and instrument/sample names.
- MP3, Ogg, FLAC, WAV and AAC/M4A playback with metadata, seeking and persistent
  volume control.
- Slideshow overlays with title, author, group and scrolling SAUCE comments.

### Changed

- Reworked toolbars, status bars and dialogs, including color-coded SAUCE
  columns and an auto-hiding thumbnail toolbar.
- Shared system clipboard integration, export dialogs, monitor controls,
  mouse-wheel zoom and updated application icons with the other desktop tools.
- Tall artwork fills the window width and scrolls vertically instead of being
  reduced to fit the window height.
- Baud-rate playback sizes the canvas from the complete file and follows the
  typing cursor; manual scrolling takes priority over automatic following.
- Thumbnails use GPU-sized image tiles rather than the previous 512-pixel
  reduction, while cache eviction retains layout metadata.

### Fixed

- Fixed stale font-preview tiles and thumbnail layout updates after background
  directory loading.
- Improved text rendering, fractional scaling and ANSI document autowrap.
- Made slow baud-rate playback tests tolerant of scheduling delays.

### Compatibility

- Existing command-line options, file providers and configuration are retained.
  Settings preserve unknown fields and detect external configuration changes.
- Animated image previews currently show the first frame.
