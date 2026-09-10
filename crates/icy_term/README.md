# IcyTERM

A modern BBS terminal for connecting to nostalgic and contemporary bulletin board systems.

Visit [Telnet BBS Guide](https://www.telnetbbsguide.com/) to explore active BBSes worldwide.

## Features

### 🌐 Connectivity

- **Protocols**: Telnet, SSH, RLogin, Raw TCP, WebSocket (including secure)
- **Proxy (Tor / I2P)**: Per-connection SOCKS5 proxy with remote DNS, so `.onion` / `.i2p` BBSes work
- **Modems** still supported :).
- **Baud emulation**: Authentic modem speeds for nostalgia

#### Proxy / Tor / I2P

Each entry in the dialing directory can route its connection through a SOCKS5
proxy (Telnet, Raw, RLogin and SSH). The target hostname is resolved by the
proxy (remote DNS), which is what makes `.onion` and `.i2p` addresses reachable.

- Pick a proxy in the entry's options: **Tor** (`127.0.0.1:9050`),
  **I2P** (`127.0.0.1:4447`), or **Custom SOCKS5** (host/port + optional auth).
- WebSocket is not proxied yet.

### 🖥️ Terminal Emulations

- **ANSI/PC**: Full ANSI-BBS with iCE colors and extended attributes + Avatar
- **Commodore**: PETSCII (C64/C128)
- **Teletext**: Viewdata/Mode7
- **Atari ST**: Atari ST VT52 + IGS 2.19
- **Atari 8Bit**: ATASCII + XEP80
- **Graphics**: RIPscrip, SkyPix
- **UTF8**: Experimental
- **Modern**: Sixel graphics (DEC 80 / DEC 1070 palettes), OSC8 hyperlinks, loadable fonts and palettes

### 🕹️ SyncTERM Door Compatibility

IcyTERM follows current SyncTERM/CTerm behavior for common door games (SyncDoom, SyncMOO, SyncSCUMM):

- **Cached JPEG XL**: SyncTERM APC cache store/draw (`SyncTERM:C;S` / `DrawJXL`)
- **Audio APC**: PCM plus Ogg Opus music (requires `libopus`; enabled by default)
- **Kitty keyboard**: Progressive enhancement (`CSI > u` / `CSI ? u`)
- **Pixel mouse**: SGR and DECSET 1016 pixel coordinates
- **Sixel**: Shared vs private palettes (`DECSET 1070`) and scrolling (`DECSET 80`)
- **LF handling**: Received LF expands to CR+LF by default (SyncTERM 0.8.3)
- **Diagnostics**: Terminal Info dialog and clipboard dump for handshake/debug

RIP file APCs (`QueryFile`, `ReadFile`, …) are parsed but not answered yet.

### 📁 File Transfers

- **Protocols**: ZModem (including 8k), XModem (Classic/1k/1k-G), YModem/YModem-G
- **Features**: Auto-download detection, batch transfers, resume support
- **UI**: Real-time statistics, transfer logs, protocol details

### 🎨 Rendering Engine

- **3D accelerated** WGPU pipeline
- **Font support**: Loadable fonts, multiple fonts per session
- **Color depth**: 24-bit RGB, extended palettes, iCE colors
- **Special effects**: CRT filter simulation, customizable scaling

### 🤖 Automation & Control

- **IEMSI**: Automatic login support
- **MCP Server**: JSON-RPC automation API
- **Lua Scripting**: Full automation scripts (see [SCRIPTING.md](SCRIPTING.md))

### 🎵 Multimedia

- **ANSI Music**: PlayMod, MIDI support
- **SyncTERM Audio APC**: Multi-channel playback with live stereo gain
- **Sound effects**: Beep patterns, system sounds

### 🌍 International

- **Multi-language**: Fluent-based localization system
- **Supported languages**: English, German, Italian, Spanish, Portuguese, and more

## Installation

### Download Binaries

Get the latest release: [GitHub Releases](https://github.com/mkrueger/icy_tools/releases)

### Build from Source

```bash
git clone https://github.com/mkrueger/icy_tools.git
cd icy_tools
cargo build -p icy_term --release
./target/release/icy_term
```

### egui Desktop Client

`icy_term` now uses egui by default and retains the existing wgpu terminal renderer.
The `icy_term_egui` binary is a compatibility alias for the same application.

```bash
cargo run -p icy_term
cargo run -p icy_term -- color_test.ans
cargo run -p icy_term -- --connect telnet://bbs.example.com:23
cargo run -p icy_term -- --connect raw://localhost:2323 --utf8
# Optional legacy fallback:
cargo run -p icy_term --no-default-features --features legacy-ui --bin icy_term_legacy
```

The client displays the existing welcome artwork or a supported terminal-art
file. It supports file selection, drag-and-drop, fit-to-window, manual zoom,
horizontal and vertical scrolling, and live monitor controls. Small windows use
proportional downscaling even when integer scaling is enabled.

**Dialing Directory** opens the existing local phonebook in the same format and
location as the legacy client. It provides name/address/notes search, favorites,
name/call-count/last-call sorting, quick connect, and entry details. Double-click
dials an entry; arrow keys and Enter work from the search field. Narrow windows
switch between directory and entry views instead of squeezing both columns.
The editor has Connection, Terminal, Login, Colors, and Notes pages, with a fixed
Save/Discard row. It includes modem selection, SOCKS5/Tor/I2P settings, SSH keys
and authentication modes, terminal-specific resolutions, baud emulation, music
mode, SAUCE fonts, LF/mouse options, iCE, a 16-color palette picker, and auto-login
presets. Password generation uses the operating system's secure random source.
Quick-connect addresses can be turned into local profiles without losing their
WebSocket path. The summary includes call statistics and transfer totals already
stored by the legacy client.

Entries can be created, edited, duplicated, and deleted with confirmation.
Edits use a separate draft with Save/Discard; closing an editor asks before
discarding it. Passwords are masked, and URL credentials are omitted from list
and connection-status labels. Existing advanced fields are preserved. The common writer uses an owner-only
temporary file on Unix, a `.bak` backup, and atomic replacement. Saving is blocked
if the file changed since loading, an existing `.new` file is present, or its
format version is newer. Reload reads external changes. Successful profile
connections update the call count and last-call time; disconnecting or closing
the application also records the connection duration. Do not edit the same
phonebook concurrently in both clients; conflict detection is not a cross-process
file lock. Global options use a separate draft and are written only by Settings / Save,
with a backup and conflict check. Terminal, audio, paths, IEMSI, serial, modem,
transfer-protocol, and web-source options can be edited there.
Enabled web directories load in the background, with the
existing HTTPS/cache behavior. A source filter separates local and remote entries.
Remote entries are read-only; Duplicate creates a local editable copy. Remote
credentials, login expressions, SSH key paths, and proxy commands are stripped.

The connection bar and command-line URLs use the same transport configuration
as saved profiles, with an 80x25 ANSI/CP437 or UTF-8 default terminal. They support
connect, cancel, disconnect, remote disconnect/error
reporting, text input, special keys, control keys, and paste (including remote
bracketed-paste mode). Tab, Escape, and arrow keys stay in the focused terminal;
opening monitor controls or the dialing directory prevents input from reaching
the connection. Unmappable CP437 characters are replaced with `?`. Saved profiles use their
credentials, auto-login expression/IEMSI, terminal size, baud emulation, proxy,
palette, supported SAUCE font name, iCE mode, and LF behavior. Telnet and Raw TCP
are unencrypted protocols; stored passwords retain the legacy plaintext format.

Saved profiles can dial Telnet, Raw TCP, SSH, WebSocket/WSS, both Rlogin variants,
and configured modems through the shared worker. All existing terminal emulations
use their corresponding screen/parser and keyboard map, including paste line
endings. SSH uses strict `~/.ssh/known_hosts` checking; unknown or changed keys
are rejected. Verify and enroll a host with a trusted SSH client before dialing.
The egui client never silently accepts a new host key. SSH key passphrases are
kept in memory only. WebSocket URLs retain their path and query; SOCKS5 settings
on WebSocket profiles must be explicitly removed because that transport does
not implement proxy support.

Profile-enabled mouse reports use the shared engine for button, motion, and
wheel encoding, including SGR/pixel modes. Shift bypasses reporting for local
scrolling. Reporting is suppressed while the terminal view is scrolled away
from its origin or a dialog owns input.

Both applications now use the same `TerminalThread`, parsers, connection code,
data models, and keymaps through the `icy_term` library. Worker events wake egui
without a GUI polling timer. Cancelling or closing a session interrupts even a
pending connection attempt and closes the worker's connection. File opening is
disabled while a session is active. A second connection cannot replace an active
one through the dialing directory.

The egui frontend includes:

- Text selection, rectangular selection with Alt, word/line selection, copying,
    search, and scrollback snapshots while the live connection continues updating.
    Scrollback retains Unicode cells alongside the original graphics and can be exported.
- Transfer selection, progress, logs, cancellation, external protocols, capture,
    replay, and streamed text upload. Automatically detected transfers require local
    approval. Received filenames cannot escape the chosen download directory or
    overwrite an existing file.
- ANSI music, chip/GIST sounds, modem sounds, beep, and SyncTERM audio through the
    existing audio worker, including output-device and volume settings.
- Lua files, a Lua console, script cancellation, and the opt-in localhost MCP server.
- Direct serial connections and baud detection, live terminal settings, IEMSI
    host information, terminal diagnostics, fullscreen, and independent native windows.
- Fluent-localized controls with English fallback; new labels include German
    translations. Other locales reuse existing translations and fall back for new keys.

The compact toolbar provides the dialing directory, upload, download, and hangup.
The menu at the right contains File, View, Quick Connect, Edit, and Session;
View contains zoom, monitor, and history controls. Right-clicking the terminal
opens the same context menu as the legacy client.

Keyboard shortcuts come from the same `commands_icy_term.toml` table the legacy
frontend uses, so both clients stay identical; press F1 for the generated
overview. As before, Alt combinations and F1 stay application level while
connected, and other control combinations are left to the terminal, so Ctrl+C
and Ctrl+W still reach the host.

Overlay controls follow the legacy layout: a scrollback position indicator above
the terminal, plus clickable BPS emulation, IEMSI, terminal info, stop-capture,
stop-sound, and update-notification controls.

The frontend uses locally bundled Fira Sans, neutral light/dark surfaces, and
consistent dialog actions. Settings use tabs on wide windows and a category
selector on narrow windows. The font is distributed under the
[SIL Open Font License](data/fonts/OFL.txt).

Shift+PageUp enters history, Shift+PageDown advances or returns
to live output, and Escape leaves history. Shift bypasses host mouse reporting.
Ctrl/Cmd+Shift+F opens search; F11 toggles fullscreen. Copy/Cut with a selection
copies locally, while Ctrl+C without a selection retains terminal semantics.
Window resizing changes display scale, not negotiated terminal dimensions.

Known platform limits: egui 0.33 does not expose distinct keypad locations or
left/right modifier-key events, so those Kitty details cannot match the legacy
frontend. Host-key enrollment remains an explicit trusted-SSH-client step.
RIP file APC replies remain unimplemented in the shared backend. Hardware serial,
modem and audible output still require testing on the target machine.

Both frontends share the existing WGSL shader, texture uploads, tile cache, and
frame preparation. egui draws through a direct wgpu paint callback, without a
per-frame GPU readback. `eframe`/`egui-wgpu` 0.33.3 use wgpu 27, matching the current
UI. `icy_engine_gui` can now build without either UI: `terminal/frame.rs` owns
frame preparation, and `terminal/shader.rs` depends directly on wgpu. Terminal
state, tile caches, monitor settings, and mouse coordinate calculations are
available without `icy_ui`. The egui path passes pixel density explicitly per
frame instead of relying on the legacy global scale factor.

The default `egui` feature excludes `icy_ui` from an isolated icy_term build.
`legacy-ui` retains the fallback frontend and existing adapter APIs. Other tools
in this workspace still use that feature, so a whole-workspace build can unify
both sets of dependencies. No replacement general-purpose UI library is added.
Both `icy_engine_gui` and the `icy_term` library also build with no UI features.

Validation:

```bash
cargo test -p icy_engine_gui --no-default-features --lib
cargo test -p icy_term --no-default-features --lib
cargo test -p icy_term --bin icy_term
# Requires a working GPU adapter; exercises egui's actual wgpu render pass:
cargo test -p icy_term --bin icy_term gpu_tests -- --include-ignored
# Optional PNG captures (use an absolute output directory):
ICY_EGUI_SCREENSHOTS="$PWD/target/egui-smoke" cargo test -p icy_term --bin icy_term gpu_tests -- --include-ignored
```

The GPU test checks desktop/narrow/HiDPI rendering, aspect ratio, CRT effects,
toolbar clipping, a monitor window over the terminal, and two-axis scrolling.
Local loopback tests cover Raw TCP receive/send, Telnet window-size negotiation,
WebSocket receive/send, both Rlogin credential orders, invalid WSS/TLS rejection,
remote close, refused connections, cancellation during a stalled handshake, and
egui focus isolation through to bytes received by a TCP server. They do not
contact public hosts.
Phonebook tests use temporary files to cover backup/permissions, explicit edits,
preservation of credentials and advanced settings, duplicate/delete/favorites,
conflicting writes, and future-format read-only behavior. Dialog tests exercise
actual egui clicks, keyboard dialing, password masking, and modal input isolation.
The dialing-directory GPU test covers desktop, narrow, HiDPI, and all editor
pages, checks visible Save/Discard controls, and verifies 360x240 overflow handling.
SSH configuration is tested for strict host-key policy; successful SSH and WSS
handshakes and physical modem dialing have not been end-to-end tested here.
Additional loopback tests cover capture/text upload, automatic-download approval,
stalled Xmodem cancellation, Lua success/error responses, and an HTTP MCP handshake.
New dialog GPU tests cover all Settings pages and transfer actions at desktop,
narrow, HiDPI, and 360x240 sizes, as well as actual drag selection.

### System Requirements

- **GPU**: OpenGL 3.3+ support (2010 or newer)
- **OS**: Windows 10+, macOS 10.14+, Linux (X11/Wayland)
- **Windows**: Requires VCRUNTIME140.dll (usually pre-installed)

## Quick Start

### Connect via Command Line

```bash
# Simple connection
icy_term bbs.example.com

# With port
icy_term bbs.example.com:2323

# SSH with credentials
icy_term ssh://username:password@bbs.example.com

# RLogin

# Replay a captured session into the terminal
icy_term --play capture.txt
icy_term rlogin://retrobbs.org
```

### Using the Dialing Directory

1. Press `Alt+D` to open the dialing directory
2. Click "Add" to create a new entry
3. Configure connection settings, terminal type, and auto-login
4. Double-click to connect

## MCP Automation API

IcyTERM includes an optional Model Context Protocol server for automation:

```bash
# Start with MCP enabled on port 3000
icy_term --mcp-port 3000

```

Available tools:

- `connect` / `disconnect` - Session management
- `send_text` / `send_key` - Input control
- `clear_screen` / `run_script` / `get_scripting_api` - Screen and Lua automation
- `capture_screen` - Screen capture (text/ANSI)
- `list_addresses` - Address book access
- `get_state` - Terminal state query

## Configuration

Optional CLI overrides: `--config FILE`, `--phonebook FILE`.
Default platform-specific locations:

- **Linux**: `~/.config/icy_term/`
- **macOS**: `~/Library/Application Support/com.GitHub.icy_term'`
- **Windows**: `%APPDATA%\icy_term\`

## Contributing

Contributions are welcome! Areas where help is appreciated:

- Testing on various BBSes and reporting compatibility issues
- Translations to new languages
- Protocol implementation improvements
- Documentation and tutorials

### Development

```bash
# Run in development
cargo run -p icy_term

# Run tests
cargo test -p icy_term

# Check specific translation usage
grep -r "fl!(.*\"key-name\"" crates/icy_term/src/
```

## Support

- **Bug Reports**: [GitHub Issues](https://github.com/mkrueger/icy_tools/issues)
- **Discussions**: [GitHub Discussions](https://github.com/mkrueger/icy_tools/discussions)
- **Donations**: PayPal to `mkrueger@posteo.de`

## Screenshots

| CP437 (DOS) | PETSCII (C64) |
|-------------|---------------|
| ![CP437](assets/cp437.png?raw=true) | ![PETSCII](assets/c64.png?raw=true) |

| ATASCII | Viewdata |
|---------|----------------|
| ![ATASCII](assets/atascii.png?raw=true) | ![Viewdata](assets/viewdata.png?raw=true) |

| RIPscrip | SkyPix |
|----------|--------|
| ![RIPscrip](assets/ripscript.png?raw=true) | ![SkyPix](assets/skypix.png?raw=true) |

| VT52 (Atari ST) | IGS Graphics |
|-----------------|--------------|
| ![VT52](assets/vt52.png?raw=true) | ![IGS](assets/igs.png?raw=true) |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## About

IcyTERM started as a test project for an ANSI rendering engine but evolved into a full-featured BBS terminal. It's part of the larger Icy Tools suite, which includes:

- **IcyDraw** - ANSI/ASCII art editor
- **IcyView** - File viewer for ANSI/ASCII art
- **IcyPlay** - ANSI animation player

The goal is to provide modern, cross-platform tools for the BBS community while preserving the authentic retro computing experience.

---

*Relive the golden age of BBSing with modern comfort!*
