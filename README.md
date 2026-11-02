# icy_tools

This repository contains tools releated to BBSing and Ansi in general. It contains:

[Icy Term](https://github.com/mkrueger/icy_tools/blob/master/crates/icy_term/README.md)
a terminal program for legacy BBS systems.

[Icy Draw](https://github.com/mkrueger/icy_tools/blob/master/crates/icy_draw/README.md)
a drawing tool supporting almost all ANSI formats.

[Icy View](https://github.com/mkrueger/icy_tools/blob/master/crates/icy_view/README.md)
a viewer to browse/view Ansi screens.

[Icy Play](https://github.com/mkrueger/icy_tools/blob/master/crates/icy_play/README.md)
a tool that shows icy draw animations on cmd line/bbs.

## Plain-text paste

Icy Term, Icy Mail and Icy Draw adapt pasted smart quotes and common
typographic symbols to the target encoding. A symbol the encoding has a
printable glyph for is kept (for example `½`/`¼` in CP437, `←`/`→`/`½` in
Viewdata, `©`/`®`/`™` on the Atari ST); otherwise it falls back to plain text:
quotes become straight quotes, arrows become `<-` and `->`, en/em dashes become
`-` and `--`, copyright/registered/trademark symbols become `(c)`, `(r)` and
`TM`, fractions become `1/4`, `1/2` and `3/4`, ellipses become `...`, and
modifier circumflexes become `^`. Glyphs that sit on control codes (CP437 and
ATASCII arrows) are never sent. PETSCII and Icy Term's SkyPix (Amiga) mode
always use the plain-text forms. UTF-8 text and rich Icy clipboard data are
unchanged.

The shared routines are `icy_engine::clipboard::normalize_paste_text` and, for
text sent to a terminal host, `normalize_terminal_paste_text`; callers apply
them before converting plain clipboard text to the destination encoding.

# Build instructions

Use the current stable Rust toolchain. The egui 0.37 desktop frontend requires
Rust 1.95 or newer.

Dependency versions and sources are maintained in `[workspace.dependencies]` in
the root `Cargo.toml`. Member crates inherit them with `workspace = true` and
keep crate-specific features and optional flags locally. Shared dependencies
with differing default-feature needs (such as `chrono` and `resvg`) disable
defaults at the workspace level; members opt in with `features = ["default"]`.
The legacy UI continues to use the egui/wgpu versions provided by `icy_ui`.

## Windows

Install the Rust MSVC toolchain and Visual Studio (or Build Tools) with the
**Desktop development with C++** workload, including a Windows SDK and
**C++ CMake tools for Windows**.

Open **x64 Native Tools Command Prompt for Visual Studio**, then run:

```bat
cd C:\work\icy_tools
git submodule update --init --recursive
cargo build --workspace --locked
```

Debug executables are in `target\debug`. Add `--release` for optimized
executables in `target\release`.

The native Opus dependency requires CMake and Ninja. If the build reports
`CMAKE_MAKE_PROGRAM is not set` for Ninja, use the developer command prompt
above so Visual Studio's bundled tools are on `PATH`.

## Linux

```
# Clone the repository  
git clone https://github.com/mkrueger/icy_tools.git  
cd icy_tools  

# Update the repository and submodules  
git pull  
git submodule update --init --recursive  

# Install Rust toolchain  
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh  
source $HOME/.cargo/env  

# Install dependencies (for Debian/Ubuntu)  
sudo apt-get install build-essential libssl-dev libasound2-dev

# Install dependencies (for Nobara/Fedora)  
sudo dnf install @development-tools pkgconf-pkg-config openssl-devel alsa-lib-devel

# If you don't have apt-get, installing these libraries is similar.  
# The next step will tell you what's missing exactly.  

# Update Rust dependencies  
cargo update  

# Build the project  
cargo build --release  

# Executables for all tools are in  
ls target/release  
```

# App icons

Each GUI tool keeps its icon master at `crates/<tool>/build/icon.svg`. After
editing one, regenerate the Linux, macOS, Windows and web icons with
`python tools/render_app_icons.py` (requires Inkscape and ImageMagick).
The design proposals are in `doc/icon-proposal*`.

# CI and releases

Pushes and pull requests run `cargo fmt`, the workspace tests, and one debug
build of IcyTERM. They do not package installers.

Release binaries are built when you push a version tag for one tool:

```
git tag IcyTerm0.8.3
git push origin IcyTerm0.8.3
```

Accepted prefixes: `IcyTerm`, `IcyDraw`, `IcyView`. That opens a **draft**
GitHub release and attaches Linux (`.deb`, AppImage), Windows (`.exe`), and
macOS (`.dmg`) packages for that tool only. Review the draft, then publish it.

You can rebuild an existing tag from Actions → Release → Run workflow.