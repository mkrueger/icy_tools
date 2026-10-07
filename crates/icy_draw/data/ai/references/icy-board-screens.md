# Icy Board screen roles and original layout examples

Icy Board bundles native .icy screens; its setup code converts several to
PCBoard display files. Verified at revision
483f4aaec5c5b93231da05457dd5241d77f023fc.
Templates: https://github.com/mkrueger/icy_board/tree/483f4aaec5c5b93231da05457dd5241d77f023fc/crates/icbsetup/data/new_bbs
Installation: https://github.com/mkrueger/icy_board/blob/483f4aaec5c5b93231da05457dd5241d77f023fc/crates/icbsetup/src/create/mod.rs

These are Icy Board resources (IcyBoard / icy_board), not generic historical
PCBoard screens. PCBoard display files are a format used by Icy Board, not
evidence that the platforms or all screen roles are identical.

Useful source files to import locally:
- welcome.icy and newuser.icy: initial welcome / new-caller displays.
- brdm.icy and brds.icy: board-menu resources.
- chtm.icy and group.icy: chat resources.
- blt.icy, news.icy, rules.icy, survey.icy: bulletin, news, rules and survey resources.
- help/hlp*.icy: command-specific help screens, e.g. hlpr.icy and hlpd.icy.
- noansi.asc: non-ANSI fallback.

The layouts below are newly authored design examples, not reproductions of
the supplied artwork. To study actual bundled colors, text or cell structure,
import the corresponding file. Do not claim these examples are shipped screens.

## Original 80x25 caller-menu plan

Use cells x=0..79, y=0..24. Reserve row 24 if the BBS supplies its own prompt.
- Rows 0..3: board title and a restrained geometric motif.
- Rows 4..5: caller/conference status, with bounded expansion widths.
- Rows 7..19: up to 39 command entries in three columns beginning at x=2,28,54.
  Give each column 24 cells, keep the hotkey aligned and the label concise.
- Rows 21..22: help/logoff reminder and a divider.
- Row 23: prompt only if the board does not generate one separately.
Suggested standard DOS palette: black background, blue structural accents,
cyan headings, light gray labels, light cyan or white hotkeys. Verify the
document palette rather than assuming these indices in a custom palette.

Original text sketch for a subset of commands:
  MESSAGES                  FILES                     BOARD
  [R] Read messages         [F] Directories           [B] Bulletins
  [E] Enter message         [D] Download              [J] Conference
  [Y] Your mail             [U] Upload                [H] Help
  [Q] Quick scan            [N] New files             [G] Goodbye

This is a subset illustration, not a complete command list.

## Original welcome/help plans

Welcome: title, one short greeting, board information, then an uncluttered
continuation prompt. Avoid exposing private caller information before login.
Help: command title, short explanation, syntax/example, then return guidance.
Use shorter text than a prose manual; preserve an ASCII-compatible fallback
when requested. Keep CP437 decoration separate from printable command labels.
