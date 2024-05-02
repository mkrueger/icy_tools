# icy_term

IcyTERM is a BBS terminal program with allows you to connect to BBSes.
Visit https://www.telnetbbsguide.com/ for a start to enter the BBS world.

Features supported so far:
- Platforms: Linux, macOs, Windows
- Telnet, SSH, Websockets and Raw connections.
- Ansi BBS, Avatar, PETSCII, ATASCII, Viewdata and RIPscrip emulation
- File transfer X/Y/Z Modem and variants (1k/1k-G/8k)
- Rich set of ansi features
  - Modern engine with extended colors, 24bit fonts, ice support
  - Sixels, loadable fonts, ansi macros, osc8 www links 
  - ANSI music
- Misc features
  - 3D accelerated rendering engine
  - IEMSI autologin
  - Baud emulation
  - Exporting buffer to disk & capture session
  - Copy & Paste
- And many more. If something is missing open a feature request :)

# Get binaries

Get the latest release here:
https://github.com/mkrueger/icy_tools/releases


## Requires

IcyTerm needs a graphics card that can can do opengl 3.3+.
(It's the 2010 version but some people have problems starting)

If it doesn't run check if graphics card drivers are up to date.

On Windows:
opengl32.dll
And VCRUNTIME140.dll is required. Usually these two are installed and it should run out of the box. If you can run any game with 3D graphics it should just work.

# Help

Contributions are welcome. But also testing & bug reporting or feature requests.

If you can't/want to contriubte code you can donate via paypal to mkrueger@posteo.de
# Screenshots

Code page 437 (aka "DOS") support:

![DOS](assets/dos_bbs.png?raw=true "CP437 DOS")

Petscii screenshot:

![Petscii](assets/c64_bbs.png?raw=true "Petscii")

Atascii screenshot:

![Petscii](assets/atascii_bbs.png?raw=true "Atascii")

Viewdata screenshot:

![Viewdata](assets/viewdata_bbs.png?raw=true "Viewdata")

RIPscrip screenshot:

![Viewdata](assets/ripscrip_bbs.png?raw=true "RIPscrip")

# History

I had an own BBS back in the 90'. When I started rust I searched a project and some days earlier I spoke with my wife about the good old days, PCBoard and then I got the idea to improve the PPL decompiler we used these days.
That was my first project and it was successful (https://github.com/mkrueger/PPLEngine).
Around that time I learned that there are still BBSes in the internet and I started to update my old ansi drawing tool (MysticDraw) however I lost a bit track because of the gtk4 bindings. It's very difficult to write even a mid sized UI application with these.

First I tried to ressurect my old ansi drawing tool (Mystic Draw) using gtk4 bindings. But they didn't really suit my needs.
Tried Druid/Egui/Iced and decided to do a smaller project that relies on an ansi engine too.

So I decided to do a terminal program. After a first implementation with iced (cool library, can recommend) I switched to egui because I needed an opengl control and the support in iced for that was lacking at that point of time.

So this was more of a test project for the ansi engine & writing rust UI apps but it got a bit larger than I thought and now IcyTerm is a fully featured terminal app for BBSes.

# Build instructions

* Install rust toolchain: https://www.rust-lang.org/tools/install
* On linux you need "sudo apt-get install build-essential libgtk-3-dev libasound2-dev libxcb-shape0-dev libxcb-xfixes0-dev"
* Then you're ready to go "cargo run"