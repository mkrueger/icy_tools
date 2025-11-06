# IcyTERM

A modern BBS terminal for connecting to nostalgic and contemporary bulletin board systems.

Visit [Telnet BBS Guide](https://www.telnetbbsguide.com/) to explore active BBSes worldwide.

## Features

### 🌐 Connectivity
- **Protocols**: Telnet, SSH, RLogin, Raw TCP, WebSocket (including secure)
- **Modems** still supported :).
- **Baud emulation**: Authentic modem speeds for nostalgia

### 🖥️ Terminal Emulations
- **ANSI/PC**: Full ANSI-BBS with iCE colors and extended attributes + Avatar
- **Commodore**: PETSCII (C64/C128)
- **Atari**: ATASCII (8-bit computers)
- **Teletext**: Viewdata/Mode7
- **Graphics**: RIPscrip, SkyPix
- **Modern**: UTF-8, Sixel graphics, OSC8 Hyperlinks, Loadable fonts
- **Experimental**: Atari ST IGS

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

### 🎵 Multimedia
- **ANSI Music**: PlayMod, MIDI support
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
- `capture_screen` - Screen capture (text/ANSI)
- `list_addresses` - Address book access
- `get_state` - Terminal state query

## Configuration

Settings are stored in platform-specific locations:
- **Linux**: `~/.config/icy_term/`
- **macOS**: `~/Library/Application Support/icy_term/`
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
| ![DOS](assets/dos_bbs.png?raw=true) | ![PETSCII](assets/c64_bbs.png?raw=true) |

| ATASCII | Viewdata |
|---------|----------|
| ![ATASCII](assets/atascii_bbs.png?raw=true) | ![Viewdata](assets/viewdata_bbs.png?raw=true) |

| RIPscrip | SkyPix |
|----------|--------|
| ![RIPscrip](assets/ripscrip_bbs.png?raw=true) | ![SkyPix](assets/skypix_bbs.png?raw=true) |

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