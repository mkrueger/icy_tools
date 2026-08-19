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

# Build instructions

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