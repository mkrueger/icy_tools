# IcyDraw

IcyDraw is the successor of MysticDraw an Ansi drawing tool I developed 1996 and last updated 2003. Due to lack on feedback I lost interest there.
Now 20 years later I've decided to give it an update and here it is.

IcyDraw is a full features ansi drawing tool doing things a bit different than other tools.

## Features
 
 - Full CP437 support
 - Runs on Linux, Mac & Windows (Linux recommended - but looks best on Mac)
 - The model is more like modern graphic tools rather than AcidDraw (draw lines/fill/brushes etc.)
 - File formats import/export: Ansi, Ascii, Artworx ADF, Avatar, BIN, XBIN, PCBoard, iCE, Tundra Draw, CtrlA and Renegade
 - Export files to .png
 - Own custom .png based file format: .ice
 - Layer model & transparent chars
 - Free selections & select by attribute/chars etc.
 - Can open multiple files
 - Like Mystic draw full TDF font support, including creating/altering fonts
 - Font editor for tdf fonts
 - Built in font editor for bit fonts
      - Supports the usage of multiple bit fonts in the same file.
      - Preview font changes in all open files
 - Suports full RGB colors
 - Sixel support - just paste an image
 - Shares the engine with IcyTerm and IcyView so it's 100% compatible
 - Palette loading (.pal JASC, Paint.NET .txt, GIMP .gpl and .hex files supported)
 - Complex animation engine (export to ansimation or animated gif)
     - Note: Icy Term can display animations without flickering (as well as any other client with a propert DCS macro implementation)
 - LUA based plugins
 - Many display options, guides, grids, line numbers
 - Full SAUCE support including 9px & aspect ratio display
 - 3d accelerated output & output filters
 - BBS Tag support.

# Get binaries

Get the latest release here:
https://github.com/mkrueger/icy_tools/releases

## Requires

IcyDraw needs a graphics card that can can do opengl 3.3+.
(It's the 2010 version but some people have problems starting)

If it doesn't run check if graphics card drivers are up to date.

On Windows:
opengl32.dll
And VCRUNTIME140.dll is required. Usually these two are installed and it should run out of the box. If you can run any game with 3D graphics it should just work.

## File structure

IcyDraw stores data only in one directory (~/.config/icy_draw on linux) - it doesn't scatter data around.

There are several folders:

| path           |  Description
|----------------|:----------------:|
| data/fonts/    | Stores bit fonts
| data/tdf/      | Stores tdf fonts
| data/palettes/ | Stores palettes
| data/plugins/  | Stores plugins
| autosave/      | Autosave data
| settings.json  | IcyDraw settings
| character_sets.json  | Character set mappings
| key_bindings.json  | Keybind settings
| recent_files.json  | MRU file list
| icy_draw.log   | IcyDraw log file

Note that fonts/palettes etc. do not need to be unzipped. Just throw a .zip file in there containing fonts and IcyDraw will pick them up.

## What it can't yet do, but potentially will be implemented

 - Full Unicode support
 - By layer transparency (esp. useful for animations - as well as by layer filters)
 - PETSCII, ATSCII and Viewdata - the engine can do it (see icy_term)
 - Server capabilities

## Translations

| Language| user | e-mail |
| :... | :... | :... |
| German  | mkrueger | mkrueger@posteo.de |
| Spanish | lu9dce   | hellocodelinux@gmail.com |

## Help

Contributions are welcome. But also testing & bug reporting or feature requests.

If you can't/want to contriubte code you can donate via paypal to <mkrueger@posteo.de>
