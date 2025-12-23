<p align="center">
  <img src="assets/logo.png" alt="IcyDraw Logo" width="128" height="128">
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

<p align="center">
  <img src="assets/main_window.jpg" alt="IcyDraw Main Window" width="800">
</p>

---

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

## 📁 Data Directory

IcyDraw stores all data in a single location:

| Platform | Path |
|----------|------|
| Linux | `~/.config/icy_draw/` |
| macOS | `~/Library/Application Support/icy_draw/` |
| Windows | `%APPDATA%\icy_draw\` |

### Directory Structure

```
icy_draw/
├── data/
│   ├── fonts/       # Bit fonts (.psf, .f16, etc.)
│   ├── tdf/         # TheDraw fonts
│   ├── palettes/    # Color palettes
│   └── plugins/     # Lua plugins
├── autosave/        # Autosave backups
├── settings.json    # Application settings
├── key_bindings.json
├── character_sets.json
├── recent_files.json
└── icy_draw.log
```

> **Tip**: Fonts and palettes can be loaded directly from `.zip` files — no need to extract!

## 🗺️ Roadmap

Planned features for future releases:

- [ ] Full Unicode support
- [ ] Per-layer transparency and filters
- [ ] PETSCII, ATASCII, and Viewdata modes
- [ ] Collaboration server

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

[![PayPal](https://img.shields.io/badge/Donate-PayPal-blue.svg)](https://paypal.me/mkrueger)

Donations can be sent via PayPal to: **mkrueger@posteo.de**

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
