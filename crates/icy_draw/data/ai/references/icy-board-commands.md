# Icy Board command reference

Original factual summary of the built-in command implementation modules at
Icy Board revision 483f4aaec5c5b93231da05457dd5241d77f023fc.
Source: https://github.com/mkrueger/icy_board/tree/483f4aaec5c5b93231da05457dd5241d77f023fc/crates/icy_board_engine/src/icy_board/state/user_commands/pcb
Configuration: https://github.com/mkrueger/icy_board/blob/483f4aaec5c5b93231da05457dd5241d77f023fc/crates/icy_board_engine/src/icy_board/commands.rs

This reference targets Icy Board (also spelled IcyBoard or icy_board).
Its PCBoard-style command conventions do not make it identical to historical
PCBoard. Icy Board extensions are part of the Icy Board target, not exceptions
to be omitted from an Icy Board menu.

This is a reference vocabulary, NOT proof that every entry is enabled on a
particular board or supported by every historical PCBoard version. Custom
commands, conferences, security levels and menus can change availability.
Use the user's actual configuration as the authority. Never invent commands
to make a layout look full; do not put privileged numeric sysop commands on
an ordinary caller menu.

## Single-letter caller commands

A abandon conference; B bulletins; C comment to sysop; D download;
E enter message; F file directories; G goodbye/logoff; H help;
I initial welcome; J join conference; K delete message; L locate files;
M graphics mode; N new files; O page sysop; P page length;
Q quick message scan; R read messages; S surveys; T transfer protocol;
U upload; V view settings; W write/change settings; X expert mode;
Y personal mail scan; Z directory filename search (Zippy scan).

## Additional implemented command families

ALIAS alias mode; BD batch download; BR broadcast; BU batch upload;
CHAT chat; FLAG flag files; LANG language; NEWS news;
OPEN doors; PPE program execution; QWK offline mail;
REPLY message reply; RM memorized-message reading; SELECT conference selection;
TEST file test; TS text search; USER user list; WHO node display.
Icy Board additionally has message-area selection and email commands;
do not label its extensions as historical PCBoard features.

If asked for "all main menu commands", clarify the target version/configuration
when needed. If no configuration is supplied, explicitly call an Icy Board
result an Icy Board reference menu, not the board's verified enabled command
list. Label a historical PCBoard result with its requested target instead.
Fit the requested list with short labels and
columns, or explain when the canvas is too small; never silently omit entries.
