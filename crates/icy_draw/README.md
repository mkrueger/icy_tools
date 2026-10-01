<p align="center">
  <img src="build/icon.svg" alt="IcyDraw Logo" width="128" height="128">
</p>

<h1 align="center">IcyDraw</h1>

<p align="center">
  <strong>A modern, cross-platform ANSI & ASCII art editor</strong>
</p>

<p align="center">
  <a href="https://github.com/mkrueger/icy_tools/releases"><img src="https://img.shields.io/github/v/release/mkrueger/icy_tools?label=Release" alt="Release"></a>
  <a href="https://github.com/mkrueger/icy_tools/blob/master/LICENSE-MIT"><img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-blue" alt="License"></a>
  <a href="https://github.com/mkrueger/icy_tools/actions"><img src="https://img.shields.io/github/actions/workflow/status/mkrueger/icy_tools/build.yml?branch=master" alt="Build Status"></a>
</p>

---

## Frontend Migration

The normal `icy_draw` binary now uses egui/eframe with the shared wgpu terminal
renderer used by IcyTerm, IcyView, and IcyMail. The `icy_draw_egui` alias is also
available. Original drawing icons, bitmap glyphs, painting helpers, document
formats, undo operations, font loading, Lua plugins, and the MCP HTTP server are
reused.

```sh
cargo run -p icy_draw -- art.icy
cargo run -p icy_draw -- --mcp-port 8080
cargo run -p icy_draw -- host --bind 127.0.0.1 --port 8000 art.icy
```

### Windows graphics troubleshooting

If startup reports `vkCreateInstance: Found no drivers!`, the Vulkan loader
cannot find a usable driver. Try DirectX 12 instead from PowerShell:

```powershell
$env:WGPU_BACKEND = "dx12"
cargo run --locked -p icy_draw
```

This selects DirectX 12 for the current shell; it requires a compatible adapter
and driver. Use `Remove-Item Env:WGPU_BACKEND` to restore automatic selection.
On a physical PC, install the graphics vendor's driver for Vulkan support. In a
virtual machine, check the hypervisor's graphics support, enable 3D acceleration
and update the guest graphics tools; Vulkan or DirectX 12 may not be available
on the virtual adapter.

`VK_LAYER_KHRONOS_validation` is an optional development layer, not a graphics
driver. Its absence in a debug build does not by itself prevent rendering.
An OpenGL `glTexSubImage2D` bounds error is a separate rendering failure, not
just a missing Vulkan extension; selecting DirectX 12 avoids that backend if
supported, but does not repair the OpenGL error.

The egui frontend currently includes:

- The original window layout: colour switcher and 8×2 palette above the tool
  column on the left, tool options on top, minimap and layer list on the right,
  and the moebius status bar with iCE/letter-spacing/aspect toggles, tool hint,
  caret or selection, and font selection.
- ANSI/ASCII editing, brushes, shapes, half-block drawing, flood fill, rectangular
  selection, layers, palette editing, tags, SAUCE, and undo/redo.
  While the canvas has keyboard focus, arrow keys move the text caret rather
  than switching focus to other controls; Tab and Escape also stay with the
  canvas. Clicking another control or opening a dialog still transfers input.
- Box-drawing lines: the line tool's **Outline** mode (after the brush modes)
  draws with CP437 line characters in one of four styles, chosen beside it like
  the shading options (`─│` single, `═║` double, `═│` and `─║` mixed). Lines
  join the lines they meet with the matching junction (`┼ ╬ ├ ╤ …`), and a loose
  line end they leave from becomes a corner; the drag previews the characters
  and joins. Dragging diagonally draws an elbow along the longer direction
  first; the right button swaps the colors, the Apply switches keep a cell's own
  colors, and Shift erases along the line, like the other shapes.
- Shading with user-defined ramps. The Shade brush mode offers a character ramp
  (`░▒▓█` by default, any CP437 characters from light to dark, or keep the
  characters) and an optional foreground color ramp. Each stroke moves a cell one
  step along each ramp independently; right-click steps back. The color ramp is
  used while the Foreground filter is on. **Edit Ramps…** manages the lists, which
  are stored in `settings.toml`.
- **Insert Image from File** accepts all formats supported by the text-document
  loader (including ANSI/ASCII, ICY, XBin, BIN, IDF, Tundra, Artworx, REXPaint,
  Avatar, PCBoard, Ctrl-A, Renegade, PETSCII and ATASCII) and the image decoders
  (including Sixel, PCX, IFF/ILBM and BSAVE). The dialog uses the central format
  registries rather than a separate extension whitelist. Visible
  layers are combined into one new editable layer at the caret, retaining colors
  and fonts without replacing the current document. Hidden layers are excluded;
  the insertion can be undone in one step. Raster images still use Sixel layers.
- Bitmap font editing and PSF saving; TDF collection/glyph editing; TDF/FIGlet
  text drawing with the existing watched font library.
- RIPscrip drawing: create a RIP drawing from New, or open a command-only `.rip`
  file. Draw pixels, lines, rectangles, filled rectangles, circles, outlined and
  filled ellipses, polygons, filled polygons, polylines, circular and oval arcs,
  circular and oval pie slices, text, Bézier curves and buttons in the 640×350 preview using the
  left-hand icon tools. Click a draw, border or fill swatch to pick its color from
  the popup palette. The fill tool floods an enclosed region up to the chosen
  border color, using the current fill color and pattern. Custom 8x8 fill patterns
  and 16-bit line patterns can be edited in the toolbar and command properties.
  The toolbar above the drawing holds the options of the tool (line
  style and thickness, fill pattern, font, size and direction, Bézier segments,
  button type and label) and shows the parameters of the shape being drawn, which
  the preview renders with the RIP engine while dragging. Only state commands whose
  value changes (color, line, fill, font, button style) are written before a shape.
  With the text tool, click the drawing and type: the text is previewed in the
  chosen font, size and direction with a frame and caret, Backspace deletes, and
  Enter or a click elsewhere adds it (Escape discards it). Clicking existing text
  with the text tool, or double-clicking it with the select tool, types into it
  again and loads its font, size, direction and color into the tool settings, where
  changing them restyles it (previewed live); clearing it removes it. Selected text
  also shows font, size, direction and color in the property panel. The editor
  changes the font or color command directly in front of the text, or inserts one,
  and restores the previous value after it, so later commands look unchanged.
  Only printable ASCII characters are accepted, so
  the text looks the same in every RIP terminal.
  Mouse regions (`|1M`) are only shown and selectable while the mouse region tool
  is active, similar to tag mode in ANSI documents: each region is outlined with its
  host command, a drag on free space adds one with the host command and the
  "invert when clicked" and "clear text window" options from the toolbar, and a
  selected region is moved, resized, nudged or deleted like a rectangle while the
  toolbar edits its host command and options. Selecting a region in the command
  list switches to the tool.
  Bézier curves are dragged from start to end; then their two end points and two
  control points can be moved until a right-click or Enter adds the curve (Escape
  discards it). For polygons and polylines,
  click each vertex, then right-click or press Enter to finish
  (Escape discards the unfinished path); filled polygons need at least three
  vertices, polylines at least two. Arcs and pie slices are dragged from the
  center to their radius, with start and end angles in the toolbar. The select
  tool picks the topmost shape under the pointer (or the one
  selected in the command list) and shows handles: a frame with eight handles for
  rectangles, bars and buttons, the vertices of polygons and polylines, the
  end and control points of lines and Bézier curves, a radius handle for circles
  and arcs, and four handles for ellipses and oval arcs. Dragging a handle
  resizes the shape and dragging the shape moves it, previewed with the RIP engine and
  applied as one undo step; arrow keys (with Shift: 10 pixels) nudge it and Delete
  removes it. Text selections use the effective RIP font, size and direction for
  their frame and hit area; text moves as a whole without resize handles.
  The RIP canvas follows the View menu's zoom settings: Fit Window, Fit Width and
  manual zoom levels, Ctrl/Cmd+Plus/Minus, Ctrl/Cmd+0 for actual size and
  Ctrl/Cmd+9 to fit. Ctrl/Cmd+mouse wheel zooms the drawing without also scrolling
  it; enlarged drawings can be scrolled in both directions.
  The Edit menu and Ctrl/Cmd+C/X/V copy, cut and paste a single selected object
  through the system clipboard as a self-contained RIP block, including its
  drawing state. Ctrl/Cmd+D duplicates it with an eight-pixel offset. Each cut,
  paste or duplicate is one undo step; focused text fields keep their normal
  clipboard behavior. Pasting restores the destination's drawing state.
  An empty drawing can adopt the copied object's palette; an incompatible
  palette in a nonempty drawing is rejected rather than recoloring existing art.
  Operations that depend on surrounding content or external resources, such as
  XOR drawing, flood fills and icon/clipboard buttons, cannot be copied safely
  and are explicitly rejected. Preserved mixed ANSI/RIP streams remain protected.
  Buttons are objects of a button style and the button itself:
  **Button…** asks for the plain, icon or clipboard type, label, host command,
  hot key, group, colors, font, label position, bevel, effects and behavior with a
  preview, then a click places the button at the style's size or a drag sets its
  bounds, and the new button is selected for moving and resizing. Double-click a
  button, or use **Edit Button…**, to change it; selecting a button or its style in
  the command list edits both together. **Edit RIP palette…** maps the 16 color slots
  to any of the 64 EGA colors with a live preview of the recolored drawing; applying it
  adds one `|a` (one slot) or `|Q` (several slots) command, and a selected `|Q` can be
  edited in place. The command list on the right shows one line per command —
  number, icon, name and a short summary, truncated instead of wrapped; a shape
  selected on the canvas scrolls into view. Select a command to see all of its
  parameters in the panel below the list, reorder or remove it with the buttons
  above the list, or use the eye button to preview the scene through that command
  (inclusive). Edit the parameters of drawn shapes, colors and fill
  styles in the property panel. Parameter edits apply without a confirmation: a
  dragged number when it is released, a typed value or text on Enter or when the
  field is left, choices immediately; the drawing previews the change meanwhile, and
  each change is one undo step.
  Save preserves the ordered RIP
  commands, with undo/redo and crash recovery. In RIP files containing terminal
  text or control sequences, the original stream is preserved verbatim and its
  commands are read-only; new drawing commands can be appended. External icon references
  are retained in the command list but cannot be previewed safely.
  The RIP editor has the same animation bar as the IGS editor, sharing its code:
  play, pause, step, stop, a position slider and a BPS rate (1200 by default) that
  paces the commands by the time a modem needs to transmit them. The command list
  follows the animation, marks the current command with a bar, dims commands not
  drawn yet and runs the animation up to a double-clicked command; the eye button's
  preview through the selection is the same position. Editing waits until the
  animation stops.
  The Bézier, button and mouse region icons are adapted from Google's
  [Material Design Icons](https://github.com/google/material-design-icons)
  (`polyline`, `smart_button` and `ads_click`), licensed under
  [Apache-2.0](../../LICENSE-APACHE).
- IGS drawing (Atari ST Instant Graphics and Sound) in its own editor: create an
  IGS drawing from New in low (320×200, 16 pens), medium (640×200, 4 pens) or high
  (640×400, 2 pens) resolution, or open any `.ig` file. The canvas renders with the
  IGS engine; medium resolution pixels are shown twice as tall as wide, like on an
  Atari ST monitor. The left-hand tools draw markers, lines, polylines, boxes,
  rounded boxes, filled rectangles, circles, ellipses, arcs, elliptical arcs, pie
  slices, elliptical pie slices, filled polygons, flood fills, text and spray paint
  (`X 0`: random markers in a dragged area of up to 255 × 255 pixels, with the marker
  type, size and a density from the toolbar). Drawing attributes are explicit
  commands, as in IG's own drawing program: the sidebar (pens for lines, fills,
  text and markers, fill pattern, drawing mode) and the toolbar (line style,
  thickness and start/end styles, fill pattern and border, marker type and size,
  text size, effects and rotation) show the attributes in effect where new commands
  go, and changing one adds its `C`, `A`, `T`, `M` or `E` command there at once. A
  command for the same attribute right before that point is changed instead, so
  trying out colors leaves one command; shapes are inserted on their own. The fill
  pattern control opens all patterns like IG's pattern screen: hollow, solid, the 24
  patterns, the 12 hatches and the eight user patterns as swatches, the border, and
  **Draw user pattern…**. Text is typed on the canvas like in the RIP editor;
  `@` ends IGS text and cannot be typed. **Edit IGS palette…** sets the colors of
  the pens in the eight Atari ST levels per channel with a live preview and adds one
  `S` command per changed pen. The select tool moves, resizes, nudges and deletes
  shapes with fixed coordinates; circles and pie slices keep the aspect correction
  IGS draws them with. The command list shows every command, VT52 text between
  commands and unparsed bytes; its buttons reorder, delete and preview through the
  selected entry. The property panel edits the parameters of drawing and attribute
  commands, applied without a confirmation. Every entry can also be edited as IGS
  source, where bytes outside printable ASCII are written as `\xNN` (`\r`, `\n`,
  `\e` and `\\` are accepted too); loops and other commands are edited
  this way. Sound effect commands offer a named selection of all 20 Atari ST
  effects (0–19) in the property panel; chip music, effect repeats, cursor, VT52
  inverse text and text colors, user input (`<`), spray color rotation, color
  registers (`X 1`, set as ST color words) and color rotation (`X 8`) have property
  fields too. Random (`r`, `R`) and loop (`x`, `y`)
  parameters are shown but only changed in the source. Opening and saving keeps
  the file byte for byte; only edited or new commands are rewritten, one `G#`
  command per line. Previews use a
  fixed seed for random parameters so they do not flicker, and run loops. Undo/redo,
  autosave and crash recovery work like in the other editors.
  While the eye button previews through the selected entry, new shapes, attribute
  commands, templates and pasted commands are inserted after it, so an attribute
  changed there applies to the commands after it too. Ctrl+C, Ctrl+X and Ctrl+V copy,
  cut and paste entries as IGS source, and Ctrl+D (or the duplicate button)
  duplicates one. The **+** button adds commands from templates in groups: drawing
  attributes, drawing (draw-to, random range, spray color rotation), screen (clear,
  initialize, resolution, wipe BitBlit memory), color registers (set, rotate,
  restore), loops and pauses, VT52 text and cursor, sound, and input and mouse
  zones, to be adjusted in the property panel or source.
  The resolution box sets the first `R` command (or adds one); an `R` after drawing
  commands is flagged. **Draw user pattern…** draws one of the eight 16 × 16 user
  patterns (`X 7`) with a tiled preview; applying adds the pattern and a fill
  command selecting it, and a selected pattern command is edited in place. The copy area tool
  drags out an area and places screen-to-screen copies (`G`) with a copy mode, which
  move like shapes. Mouse zones (`X 4`) are drawn, moved and resized with their own
  tool like RIP mouse regions, with the host string from the toolbar and the next
  free zone number. The transport bar plays or pauses the drawing, steps backward
  and forward through its IGS items, and stops to restore the full canvas. Its
  position slider jumps to any item, while paused or during playback, without
  replaying earlier sounds. The command list follows the animation: the current
  item is selected, scrolled into view and marked with a bar, and items not drawn
  yet are dimmed; double-clicking an entry runs the animation up to it. The eye
  button's preview through the selection is the same position: Play continues
  after the previewed entry, stepping and the slider move the selection, and the
  eye turns a paused animation into an editable preview through its frame. Stop
  ends both and shows the whole drawing. Playback
  waits for IGS pauses, chip music timing and loop delays, draws loops with a delay
  iteration by iteration as a terminal shows them (e.g. blit animations), shifts
  color rotations one step per delay, and plays sound effects.
  A selectable BPS rate (1200 by default, or Max for no transmission delay)
  approximates the time needed to transmit each item's source bytes, so drawings
  without explicit pauses animate too. After pausing, Play resumes at the next
  item; after reaching the end, Play starts again at the beginning. Sound commands
  can also be played from the property panel with the sound table in effect there.
- Lua animation editing, frame preview/playback, GIF and Asciicast export.
- Existing Lua plugins, direct Lua scripts, MCP automation, and the headless
  collaboration server.
- Shared monitor controls and GPU rendering, manual/fit zoom, scrolling,
  character grid, native file dialogs, and independent editor-window processes.
- Atomic native document/font/script saves with external-change checks and
  explicit overwrite confirmation. Save uses `.icy`, `.tdf`, or `.psf` as
  appropriate; ANSI and image formats use Export.
- Autosave and crash recovery for drawings, RIP and IGS graphics, TheDraw fonts, bitmap
  fonts and animations. While a document has unsaved changes, a snapshot is written in the
  background at most every 2 seconds (less often for very large documents) to the
  `recovery` folder of the local data directory, and removed once the document is
  saved, all changes are undone, or you discard them when closing. Snapshots
  replace the previous one atomically and each window holds an OS file lock on
  its own, so after a crash, kill or power loss, the next start offers exactly the
  documents of windows that are no longer running: restore them (in this window,
  or in new windows when this one has unsaved changes), discard them after a
  confirmation, or decide later. A restored document keeps its file name and
  counts as unsaved; if the file changed on disk in the meantime, saving asks
  before replacing it. Undo history is not part of the snapshot. If autosave
  cannot write, an error is shown.

This is **not yet full legacy UI parity**. Use the legacy frontend for graphical
collaboration sessions, AV1 export, advanced bitmap
font import/export and font-slot management, free-form selections, reference
images, and guides. The egui controls currently use English labels.
Copying puts the selection on the system clipboard as text, RTF, a PNG image and
Icy data, so it pastes with its colors into other Icy Draw instances and from
Icy Term, Icy View and Icy Mail. Paste prefers Icy data, then an image (as a
floating image layer), then text; **Edit ▸ Paste** also pastes a clipboard that
holds only an image, which Ctrl+V cannot because egui reports no paste for it.
Bitmap font pixels are copied in their own format with the pixels as `#`/`.`
text. New windows do not share live documents. Existing
legacy session and autosave files are not migrated or modified by the egui frontend. Lua
execution retains the existing backend's lack of a runtime cancellation limit.

```sh
cargo run -p icy_draw --no-default-features --features legacy-ui --bin icy_draw_legacy -- art.icy
```

Focused validation commands:

```sh
cargo test -p icy_draw --no-default-features --lib
cargo test -p icy_draw --bin icy_draw
# Requires a working native wgpu adapter; writes optional screenshots:
ICY_EGUI_SCREENSHOTS="$PWD/target/egui-draw" cargo test -p icy_draw --bin icy_draw gpu_ -- --ignored
cargo check -p icy_draw --features legacy-ui --bins
```

The feature overview below describes the complete application, including
features that still require the legacy frontend.

## ✨ Overview

IcyDraw is the spiritual successor to **MysticDraw** (1996–2003), completely reimagined for the modern era. Unlike traditional ANSI editors, IcyDraw brings a contemporary graphics editor workflow to the world of text-mode art.

## 🚀 Features

### Drawing & Editing
- **Modern toolset** — Lines, rectangles, ellipses, fill tools, brushes, and more
- **Layer system** — Full layer support with transparency
- **Flexible selections** — Free-form selections, select by attribute/character
- **Multi-document** — Work on multiple files simultaneously
- **Undo/Redo** — Full edit history

### File Format Support

| Format | Import | Export |
|--------|:------:|:------:|
| ANSI (.ans) | ✅ | ✅ |
| ASCII (.asc) | ✅ | ✅ |
| PCBoard (.pcb) | ✅ | ✅ |
| XBIN (.xb) | ✅ | ✅ |
| BIN (.bin) | ✅ | ✅ |
| Artworx ADF | ✅ | ✅ |
| iCE Draw | ✅ | ✅ |
| Tundra Draw | ✅ | ✅ |
| Avatar | ✅ | ✅ |
| CtrlA | ✅ | ✅ |
| Renegade | ✅ | ✅ |
| PNG | — | ✅ |
| IcyDraw (.iced) | ✅ | ✅ |

### Typography
- **Full CP437 support** — Complete DOS character set
- **TheDraw fonts (TDF)** — Create, edit, and use TDF fonts
- **Multiple bit fonts** — Use different fonts in the same document
- **Built-in font editor** — Edit fonts with live preview across all open files

### Advanced Features
- **Full RGB color support** — Beyond the 16-color palette
- **Sixel support** — Paste images directly
- **Animation engine** — Create complex animations, export to GIF or ANSImation
- **Plugin system** — Extend functionality with Lua scripts
- **SAUCE metadata** — Full support including 9px mode and aspect ratio
- **3D accelerated rendering** — GPU-powered display with filters
- **BBS tag support** — For bulletin board system integration

## 📦 Installation

### Download

Get the latest release for your platform:

**[⬇️ Download Latest Release](https://github.com/mkrueger/icy_tools/releases)**

Available for:
- 🐧 Linux (AppImage, .deb)
- 🍎 macOS (Universal binary)
- 🪟 Windows (.exe)

### System Requirements

- **Graphics**: OpenGL 3.3+ compatible GPU
- **Windows**: `opengl32.dll` and `VCRUNTIME140.dll` (usually pre-installed)

> **Note**: If IcyDraw doesn't start, ensure your graphics drivers are up to date.

### Backend selection (advanced)

IcyDraw renders through `wgpu`, which by default picks the best graphics
backend available (Vulkan / Metal / DX12 / GL). On systems where the primary
backend is unstable (older Intel iGPU, remote-desktop sessions, Wayland +
proprietary NVIDIA, …) you can force a different backend with the
`WGPU_BACKEND` environment variable:

```bash
# Force the OpenGL ES path (most compatible, slowest):
WGPU_BACKEND=gl   icy_draw

# Other valid values: vulkan, metal, dx12, primary, secondary, all
WGPU_BACKEND=vulkan icy_draw
```

If IcyDraw starts but a particular widget (minimap, layer preview, F-key
toolbar, …) renders blank or crashes the process, please file a bug with the
output of running with `RUST_LOG=icy_draw=warn,wgpu_core=warn` — the widget
GPU helpers log a single `texture clamped to device limits` line when they
fall back to a smaller texture, which makes diagnosing constrained-backend
behaviour straightforward.

### Build from Source

```bash
# Clone the repository
git clone https://github.com/mkrueger/icy_tools.git
cd icy_tools

# Build in release mode
cargo build --release -p icy_draw

# Run
./target/release/icy_draw
```

#### Build Dependencies (Linux)

```bash
# Debian/Ubuntu
sudo apt-get install build-essential libasound2-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora
sudo dnf install alsa-lib-devel libxcb-devel
```

## 🤝 Collaboration (Moebius-compatible)

IcyDraw supports **real-time collaboration** via a Moebius-compatible WebSocket protocol.

### Join a session

1. Start IcyDraw
2. Click **Connect to Server…** on the start page, or use **File → Connect to server…**
3. Enter the server address

Accepted formats (port defaults to **8000** if omitted):

- `localhost`
- `example.com:9000`
- `example.com:8000/some/path`
- `ws://example.com:8000`

Then choose a nickname (and optionally a group) and provide a password if the server requires one.

### Host a session (headless server)

IcyDraw can host a collaboration session as a headless server:

```bash
# Host an existing file (format is detected from the extension)
icy_draw host my_art.ans

# Host with password, custom bind/port, and autosave configuration
icy_draw host --bind 0.0.0.0 --port 8000 --password secret --backup-folder ./backups --interval 10 my_art.ans
```

Notes:

- In collaboration mode, **Save / Save As** are disabled (the server handles persistence). Use **File → Export** to write an export on the client.
- `--interval` is in minutes; use `0` for shutdown-only saves.

### Debugging collaboration traffic

Set `ICY_COLLAB_DEBUG=1` to print raw collaboration JSON messages (TX/RX) to the log/stdout.

```bash
ICY_COLLAB_DEBUG=1 cargo run -p icy_draw
```

## 📁 Data Directory

IcyDraw uses OS-specific directories (via `directories::ProjectDirs`) for configuration and local state.

Typical locations:

| Type | Linux | macOS | Windows |
|------|-------|-------|---------|
| Config | `~/.config/icy_draw/` | `~/Library/Application Support/icy_draw/`* | `%APPDATA%\icy_draw\`* |
| Local data (session/autosave) | `~/.local/share/icy_draw/` | `~/Library/Application Support/icy_draw/`* | `%LOCALAPPDATA%\icy_draw\`* |

\*Depending on platform conventions, an additional vendor folder (e.g. `GitHub`) may be used.

### Directory Structure
```text
icy_draw/
├── settings.toml        # Application settings
├── recent_files.json    # Most recently used files
├── fkeys.json           # F-key character sets
├── icy_draw.log         # Log file
└── data/
    ├── plugins/         # Lua plugins
    │   └── taglists/    # BBS tag replacement lists
    └── text_art_fonts/  # Text-art fonts (TDF/FIGlet); legacy fallback: data/fonts/
```

Local data (session restore + crash-recovery autosaves):

```text
session/
├── session.json          # Last window/session state
├── untitled_*.autosave   # Autosaves for new/unsaved documents
└── *.autosave            # Autosaves for existing files (hashed by path)
```

> **Tip**: Fonts and palettes can be loaded directly from `.zip` files — no need to extract!

### Tag replacement lists

The tag tool's properties dialog has a **…** button next to *Replacement* that opens the
replacement lists: PCBoard and IcyBoard are built in, and your own lists are `.toml` files in
`data/plugins/taglists/`. *Import…* copies a list there, *New List* creates one from a template
and opens it in your editor, and *Open Folder* shows the folder. A list looks like this:

```toml
name = "My Tags"
description = "Replacements of my BBS"
comments = """Optional notes shown below the list."""

[[entries]]
tag = "@USER@"          # what the BBS replaces
example = "Sysop"       # becomes the tag's preview
description = "Name of the current user."
```

## 🗺️ Roadmap

Planned features for future releases:

- [ ] Full Unicode support

## 🌍 Translations

IcyDraw is available in multiple languages:

| Language | Translator | Contact |
|----------|------------|---------|
| 🇩🇪 German | mkrueger | mkrueger@posteo.de |
| 🇬🇧 English | mkrueger | mkrueger@posteo.de |
| 🇪🇸 Spanish | lu9dce | hellocodelinux@gmail.com |
| 🇧🇷 Brazilian Portuguese | lu9dce | hellocodelinux@gmail.com |
| 🇨🇿 Czech | lu9dce | hellocodelinux@gmail.com |
| 🇫🇷 French | lu9dce | hellocodelinux@gmail.com |
| 🇭🇺 Hungarian | lu9dce | hellocodelinux@gmail.com |
| 🇮🇹 Italian | lu9dce | hellocodelinux@gmail.com |
| 🇵🇱 Polish | lu9dce | hellocodelinux@gmail.com |
| 🇷🇴 Romanian | lu9dce | hellocodelinux@gmail.com |
| 🏴 Catalan | lu9dce | hellocodelinux@gmail.com |

Want to add a translation? Contributions are welcome!

## 🤝 Contributing

Contributions are welcome in many forms:

- 🐛 **Bug reports** — Found an issue? [Open an issue](https://github.com/mkrueger/icy_tools/issues)
- 💡 **Feature requests** — Have an idea? Let us know!
- 🔧 **Code contributions** — PRs are appreciated
- 🧪 **Testing** — Help us find edge cases
- 🌍 **Translations** — Help make IcyDraw accessible worldwide

## 💖 Support

If you enjoy IcyDraw and want to support its development:

Give Feedback/report bugs.

I'm sure there are tons of small "niggles". I don't draw many ansis… never did.

## 📜 License

IcyDraw is dual-licensed under:

- [MIT License](../../LICENSE-MIT)
- [Apache License 2.0](../../LICENSE-APACHE)

## 🔗 Related Projects

IcyDraw is part of the **icy_tools** suite:

- **[IcyTerm](../icy_term/)** — Terminal emulator for BBSs
- **[IcyView](../icy_view/)** — ANSI art viewer
- **[IcyPlay](../icy_play/)** — ANSI animation player

---

<p align="center">
  Made with ❤️ for the ANSI art community
</p>
