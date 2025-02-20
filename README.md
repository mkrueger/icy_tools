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

* Install rust toolchain: https://www.rust-lang.org/tools/install
* On linux you need "sudo apt-get install build-essential libgtk-3-dev libasound2-dev libxcb-shape0-dev libxcb-xfixes0-dev"
- if you don't have apt-get installing these libraries is similiar the next step will tell you what's missing exactly.
* Then you're ready to go "cargo build --release"
* Exectuables for all tools are in target/release
* Note: Initalizing submodules "git update --init" may be required on first checkout with git.