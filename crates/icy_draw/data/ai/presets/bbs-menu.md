# Icy Board BBS menu (PCBoard-style conventions)

Original Icy Draw workflow. This preset bundles the Icy Board command reference
as factual data. Use the user's actual command configuration in preference to
that generic list.

Icy Board is the primary target of this workflow. Icy Board, IcyBoard and
icy_board refer to the same platform. If the request names no BBS platform,
use Icy Board and state that assumption. Its menus, display macros and command
conventions are similar to PCBoard, but the platforms are not identical.
For an Icy Board request, use its documented extensions and bundled screen
roles rather than limiting the result to historical PCBoard features.
An explicitly requested historical PCBoard version remains a separate target;
do not silently substitute Icy Board commands or macros.

1. Establish canvas size, encoding, palette, iCE/blink mode and target BBS.
   With character drawing tools, start with icy_canvas_info. In the RIP editor,
   use icy_rip_info and icy_rip_api, plan pixel geometry rather than ANSI cells,
   and follow the RIP workflow. Without tools, ask for a
   snapshot when document-specific advice requires it.
2. Extract the required command keys and labels before drawing. If version
   differences matter, ask one focused question rather than inventing a list.
3. Reserve title/status/prompt rows; plan columns for every required entry.
   Report a space constraint instead of silently dropping commands.
4. Use simple geometry, readable high-contrast hotkeys and restrained decoration.
   For "cool blue", choose blue/cyan/white from the actual palette.
5. Treat display macros as source tokens, not visual text. Distinguish a
   visual mockup from a macro-enabled BBS export.
6. Read existing art, preserve it unless replacement was requested, and work
   in a few bounded tool batches rather than one call per cell.
7. Check bounds, hotkey uniqueness, command coverage and contrast. Describe
   the draft and remind the user that it must be applied.
