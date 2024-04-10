# Icy Play
Used for playing icy_anim files from Icy Draw on console/bbs. 
It's just a command line tool should be easy to build on any platform. Just set up rust on your system:
https://www.rust-lang.org/tools/install

And type "cargo build --release" - in target/release is icy_play.

That should work for all unix based OSes:
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/mkrueger/icy_play.git
cd icy_play
cargo build --release
sudo cp target/release/icy_play /usr/bin
```

# Usage
```
Usage: icy_play [OPTIONS] <PATH> [COMMAND]

Commands:
  play        Plays the animation (default)
  show-frame  Show a specific frame of the animation
  help        Print this message or the help of the given subcommand(s)

Arguments:
  <PATH>  File to play/show.

Options:
      --utf8         If true modern terminal output (UTF8) is used.
      --port <PORT>  Socket port address for i/o
  -h, --help         Print help
``````