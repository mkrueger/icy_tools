# icy_view

ANSI, ASCII and text-art viewer with a native egui frontend and the existing wgpu CRT renderer.

## Running From Source

The regular binary now uses egui:

```sh
cargo run -p icy_view -- [FILE_OR_DIRECTORY]
cargo run -p icy_view -- --config-dir /tmp/icy-view-test [FILE_OR_DIRECTORY]
```

`--auto`, `--bps RATE`, `--portable` and `--config-dir DIR` remain available.
The `icy_view_egui` alias runs the same frontend. The previous icy_ui frontend is retained explicitly:

```sh
cargo run -p icy_view --no-default-features --features legacy-ui --bin icy_view_legacy
```

An isolated default build does not depend on icy_ui. Building the entire workspace may still enable it through other applications.

## egui Frontend

- List, thumbnail grid and full preview, with responsive navigation for small windows.
- Custom masonry tiles reuse the original layout algorithm: variable aspect-ratio heights, one to three columns per thumbnail, shortest-column placement, spatial arrow navigation and scrolling to the selected tile.
- The tile view has the original overlay toolbar with up, sort, list and slideshow buttons; it hides itself after five seconds (1.5 seconds later on) and returns when the top left corner is touched.
- Compact 24-pixel file rows with filter highlighting, clickable name/size headers, direct sort controls and SAUCE mode with separate name, title, author and group columns. SAUCE mode expands the list; narrow windows can scroll the columns horizontally.
- The original colour coding: file names by type, SAUCE title in yellow, author in green and group in blue, with a placeholder for empty fields.
- A colour-coded status bar summary of title, author, group, date, content size, buffer size and capabilities; clicking it opens the SAUCE dialog.
- Local directories, nested archives and the existing Sixteen Colors provider.
- Background loading, cancellation, filtering, sorting and navigation history.
- Text-art, image and Sixel previews; tiled uploads support images taller than a GPU texture.
- Art fills the window width and scrolls vertically instead of being shrunk to fit the height, as in the original viewer.
- Zoom, mouse panning and automatic scrolling. An auto-scroll icon sits above every preview; manual wheel, scrollbar, pan, minimap or page scrolling switches it off and takes priority over cursor following. Parser-based previews also have play/pause, replay, baud-rate selection and byte seeking in that bar. At a simulated baud rate, the complete file determines the canvas height before playback; the viewer follows the typing cursor rather than jumping to the bottom of the still-unrevealed document.
- TheDraw (`.tdf`) and FIGlet (`.flf`) font viewer: the bar above the preview picks one font of a bundle (or all of them, mouse wheel steps through), renders a typed multi-line sample text, and adjusts letter spacing, line gap, the outline style, the colours of outline/block/FIGlet fonts and a 40/80/132-column width guide. A single font also shows a table of all its glyphs.
- Tracker music (`.mod`, `.s3m`, `.xm`, `.it`) plays in the background of an info sheet with the title, format, channels, tempo, length, song message and the numbered instrument/sample names in their original CP437, so the greetings and ASCII art that sceners put there stay intact. The bar above offers play/pause, replay and a time slider; slideshows start modules paused. Other `.mod` files (kernel or Fortran modules) are shown as text.
- Slideshow mode with the original timing (minimum display time and a pause after scrolling), a title/author/group overlay, SAUCE comments that scroll up and fade, Space/Enter for the next file, Escape or a click to leave, and background preloading of the next file.
- Text selection, rectangular selection with Alt, word/line selection, copy and hyperlinks.
- SAUCE inspection, export with format-specific options, and overwrite confirmation.
- Monitor settings, external commands F5-F8, export directory, help and ANSI About dialog.
- Existing command definitions and translations; new labels have English/German translations with fallback for other locales.

`icy_engine_gui::egui` now owns the shared dialog layout, typography, fonts, monitor controls, shortcuts, screen widget and blink scheduling. icy_term uses the same appearance, monitor and frame-scheduling infrastructure. Both viewer frontends reuse the file providers, format/parser worker, audio backend, thumbnail loader, masonry layout and background SAUCE loader. The custom egui widgets paint thumbnails in GPU-sized slices without the former 512-pixel reduction; texture eviction preserves layout metadata.

Settings are saved explicitly from the settings dialog. Unknown TOML fields are retained; a changed configuration file blocks saving an older draft. This check is not a cross-process lock.

### Migration Notes

The core browsing and viewing workflows are ported, but the frontend is not pixel-identical to the legacy UI. Image previews show the first frame of animated image files. New windows run as independent processes.

Automated tests cover local/ZIP navigation, folder/archive double-clicks, cancellation, text/image/error loading, export, configuration conflicts, masonry navigation, thumbnail-cache eviction, the auto-hiding tile toolbar, slideshow timing and comment fading, and vertical scrolling of tall art. Real wgpu tests cover selection/copy, long image tiles, 80/160/240-column XBin thumbnails, SAUCE columns and sorting, the colour-coded status bar with its SAUCE dialog, the slideshow overlay, and dialogs in light/dark themes at desktop, narrow, short and HiDPI sizes:

```sh
cargo test -p icy_view --bin icy_view
cargo test -p icy_view --bin icy_view -- --include-ignored
```

The second command requires a working GPU adapter and network access for the optional live Sixteen Colors pack-list test. Run just that web test with `cargo test -p icy_view --bin icy_view live_web_pack_populates_tiles_after_loading -- --include-ignored`. A separate offline regression covers directory results arriving after an empty tile layout has already been rendered. Native file dialogs, audible output, full online browsing workflows and macOS/Windows window behavior still need testing on their target environments.

---

## 📋 Supported Formats

### ANSI/Text Art
- ANSI (.ans)
- ASCII (.asc, .txt)
- Artworx ADF (.adf)
- Avatar (.avt)
- BIN (.bin)
- XBIN (.xb)
- PCBoard (.pcb)
- iCE Draw (.idf)
- Tundra Draw (.tnd)
- Renegade (.an1, .an2, etc.)
- And many more...

### Images

- PNG, JPEG, GIF, BMP, WebP, TGA, TIFF, QOI, ICO
- Sixel graphics (.six, .sixel)

### Tracker Music

- ProTracker/SoundTracker MOD (.mod), Scream Tracker 3 (.s3m), FastTracker II (.xm), Impulse Tracker (.it), played with [xmrsplayer](https://codeberg.org/sbechet/xmrsplayer)

### Archives (browsable as virtual folders)

Powered by [unarc-rs](https://github.com/mkrueger/unarc-rs):
- **7z** (.7z) — Full support
- **ZIP** (.zip) — Full support including legacy methods
- **RAR** (.rar) — RAR4 & RAR5
- **LHA/LZH** (.lha, .lzh) — Full support
- **ARJ** (.arj) — Methods 0-4
- **ARC** (.arc) — Classic DOS archiver
- **ZOO** (.zoo) — Methods 0, 1, 2
- **UC2** (.uc2) — UltraCompressor II
- **SQZ** (.sqz), **SQ** (.sq, .sq2), **HYP** (.hyp)
- **Z** (.Z) — Unix compress

---

## 🚀 Installation

Get the latest release from:  
**https://github.com/mkrueger/icy_tools/releases**

Available for:
- 🐧 Linux (AppImage, native binary)
- 🍎 macOS
- 🪟 Windows

---

## 🤝 Contributing

Contributions are welcome! Whether it's:
- 🐛 Bug reports
- ✨ Feature requests
- 🔧 Code contributions
- 🌍 Translations
- 📝 Documentation

If you enjoy icy_view and want to support development, donations via PayPal to mkrueger@posteo.de are appreciated!

---

## 📜 License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.

---

*Made with ❤️ for the ANSI art community*
