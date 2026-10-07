# Icy Board display macros

Original factual summary checked against Icy Board revision
483f4aaec5c5b93231da05457dd5241d77f023fc. Not a universal PCBoard version reference.
Source: https://github.com/mkrueger/icy_board/blob/483f4aaec5c5b93231da05457dd5241d77f023fc/docs/macros.md
Additional codes: https://github.com/mkrueger/icy_board/blob/483f4aaec5c5b93231da05457dd5241d77f023fc/docs/new_macros.md

The target is Icy Board, also spelled IcyBoard or icy_board. Its PCBoard-style
display syntax is shared vocabulary, not a guarantee of identical expansion
behavior or macro availability. Icy Board's documented macros and extensions
are first-class features here; historical PCBoard compatibility is separate.

## Values

- @BOARDNAME@: configured board name.
- @SYSOPNAME@: configured sysop name, with an optional real-name setting.
- @FIRST@: normalized caller first name.
- @USER@: uppercase caller display name; may be an alias.
- @CITY@: caller location.
- @CONFNAME@ / @CONFNUM@: conference name / number; main board is number 0.
- @AREANAME@ / @AREANUM@: message area name / number; areas start at 1.
- @DIRNAME@ / @DIRNUM@: file directory name / number; directories start at 1.
- @TIMELEFT@: remaining session minutes, or UNLIMITED for an unlimited session.
- @NUMBLT@ / @NUMDIR@ / @NUMAREA@: configured conference counts, not necessarily
  counts accessible to this caller.
- @VERSION@: Icy Board version.
- @GFXMODE@: localized graphics-mode label.
- @NODE@: currently zero-based; do not assume the same base as area numbers.

Caller values can be empty before login. Budget space for expanded values, not
the length of their macro spelling.

## Formatting

Names are case-insensitive. @CITY:20@ creates a left-aligned 20-character field;
@CITY:20R@ aligns right and @CITY:20C@ centers. Values longer than the field
are truncated at the end, even when right-aligned. Without a width, there is no
length limit. The optional T flag is accepted but does not change trimming.
@POS:n@ pads to a 1-based absolute column on the current line; it does not move
backwards. Editor tool coordinates remain 0-based.

## Control and colors

@CLS@ clears the screen. @PAUSE@ requests an Enter prompt; do not assume timed
automatic continuation.

@Xbf uses two hexadecimal attribute digits: background first, foreground second.
There is NO closing @. DOS colors are 0 black, 1 blue, 2 green, 3 cyan, 4 red,
5 magenta, 6 brown, 7 light gray, 8 dark gray, 9 light blue, A light green,
B light cyan, C light red, D light magenta, E yellow, F white.
Bright backgrounds can blink on traditional terminals.
@X00 and @XFF have special current implementation behavior; use an explicit
color such as @X07 for a predictable reset. Do not rely on @XON@ / @XOFF@ in
ordinary display text: the scanner handles @X as a color prefix first.

Original small source example (not copied from a bundled screen):
@CLS@@X0B@BOARDNAME:36C@
@X07Welcome, @FIRST:20@  Time: @TIMELEFT:9R@

These are BBS source tokens, not printable decorations. For a visual menu,
use actual cell attributes and illustrative expanded values. Only write literal
macro source when requested, and explain the expansion/export distinction.
