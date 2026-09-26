# Icy Mail

An offline QWK mail reader and reply-packet composer. The regular binary uses
egui with the existing wgpu ANSI renderer. QWK parsing, header indexing, lazy
body loading, caching and reply threading remain shared with the previous
frontend.

## Run

```sh
cargo run -p icy_mail -- [packet.qwk]
```

Open a packet with **Open**, Ctrl/Cmd+O, by dropping it on the window, or from
the start screen's recent packet list (hover an entry and press × to forget
it). Packets are unpacked with [unarc-rs](https://github.com/mkrueger/unarc-rs),
so ZIP, ARJ, LHA/LZH, RAR, 7z, ARC, ZOO and the other formats it detects work;
the archive must contain `CONTROL.DAT` and `MESSAGES.DAT`. Standalone REP
archives are export files, not readable incoming packets.

The old frontend remains available:

```sh
cargo run -p icy_mail --no-default-features --features legacy-ui --bin icy_mail_legacy
```

`icy_mail_egui` is an alias of the default frontend. The isolated default build
does not depend on `icy_ui`; workspace-wide builds may enable it for other apps.

## Reading

- Three-pane layout: mailboxes and conferences, message list, and reader.
  Windows narrower than 600 px switch to a single pane with
  Folders/Messages/Message tabs; short windows collapse the message header.
- **All Messages**, **Personal** (messages addressed to you) and each
  conference show unread counts. Showing a message marks it read; read marks
  are remembered per packet across restarts.
- Unread messages are bold with a dot; replied-to and private messages are
  flagged. Opening a folder selects its first unread message.
- Sortable columns, list or thread view, and an **Unread** filter. Search
  matches author, recipient and subject; matches are highlighted in the list
  and the message header.
- The reader header links to the referenced message (“reply to #n”) and offers
  reply, forward, mark read/unread, copy and previous/next actions.
- ANSI/CP437 rendering, text/rectangular selection and copy, zoom, light/dark
  themes and additional native windows.
- **Settings** (Ctrl/Cmd+,): theme, message zoom and the shared monitor
  emulation (monitor color, scaling, filtering, brightness, contrast, CRT
  effects…). Changes preview live; Cancel restores the previous values. They
  are saved to `settings.toml` in the icy_mail configuration directory.
- Packets and message bodies load in the background; a failed open keeps the
  current packet and reports the error.

Drag over the reader to select text; Alt-drag selects a rectangle. Ctrl/Cmd+C
copies the selection, Ctrl/Cmd+A selects the body, and the copy button copies
the entire message.

## Writing replies

Use **New** (Ctrl/Cmd+N), **Reply** (Ctrl/Cmd+R) or **Forward** (Ctrl/Cmd+L)
from the toolbar, the message context menu or the reader header. The composer
replaces the reader pane: choose the conference and privacy, edit recipient,
subject and sender (with QWK's 25 character limits shown), and write the
message in the BBS editor described below.
Problems that would block export are listed live. Save with Ctrl/Cmd+S or
Ctrl/Cmd+Enter; Escape cancels and asks before discarding changes. Closing a
window with an unsaved composer asks as well.

### Message editor

The message text is written on an 80 column CP437 terminal, modeled after
Synchronet's SlyEdit. Lines wrap at 79 columns and are saved as they appear on
screen. Typing inserts or, after Insert, overwrites; the terminal's status bar
shows the mode, line and column, and the current color. The tools around the
terminal are regular panels, reachable from the toolbar above it or by keyboard:

- **Colors** (Ctrl+K or the color button): a picker with the 16 foreground and
  8 background colors, blink and a preview. Click swatches or use the arrows
  (left/right foreground, up/down background), 0-F and Space, then Apply or
  Enter. With a selection the selected text is recolored, otherwise the color
  applies to what you type next. Colors are stored as ANSI sequences in the
  message, which most BBS readers display.
- **Characters** (Ctrl+G or the toolbar): the CP437 table in the terminal's
  font, beside the text (below it in narrow windows). Clicking a character
  inserts it and keeps the table open. Ctrl+G moves the keyboard into the table:
  arrows choose, Enter inserts, Space inserts and stays, Escape returns to the
  text. Control codes and QWK's line separator (227) are dimmed and cannot be
  used. Characters that CP437 lacks are refused when typed and become `?` when
  pasted.
- **Quoting** (Ctrl+Q): a reply starts empty with the quote panel open below the
  text. It lists an attribution line and the original quoted with the author's
  initials (` JD> `, requoting ` JD>` lines as ` JD>>`). The arrows select a
  line, Enter or **Quote Line** inserts it, Ctrl+A or **Quote Rest** inserts it
  and everything after it, and double-clicking inserts a line. Typing or Escape
  closes the panel. Forwards include the original quoted in full.
- **Find** (Ctrl+F): a search field above the text; Enter or F3 finds the next
  match, Escape returns to the text.
- **Editing**: Ctrl+D deletes the line, Ctrl+Z/Ctrl+Y undo and redo, Ctrl+A
  selects all, Shift with arrows or the mouse selects, and Ctrl+C/X/V copy, cut
  and paste.

Escape first closes the color picker, quote panel, character table, find field
or selection before it cancels the message.

Saved drafts appear in the **Outbox**. Select one to preview it in the message terminal
(with colors, like received mail), press Enter or
double-click to edit it, and Delete to remove it. Drafts are stored separately
for each source packet and survive restarts.

**Export Replies** (Ctrl/Cmd+Shift+E) saves all drafts as a QWK `.rep` ZIP
archive containing the BBS's `.MSG` reply file. Transfer that file to the BBS
using your usual offline mail workflow. Drafts with problems are listed instead
of being exported. Export does not delete drafts. Unsupported characters and
invalid QWK fields are reported rather than silently replaced.

## Taglines

Taglines are kept in `taglines.txt` in the user data directory, one per line
(the same format as MultiMail's `taglines` file, so an existing list can be
copied over). A tagline is sent below the message as `... text`, up to 76
characters.

- New messages get a random tagline; this can be switched off in Settings
  (General → Writing). The **Tagline** bar below the editor shows it and offers
  choosing, shuffling and removing it.
- Ctrl+T (or clicking the tagline) opens the picker: filter, arrows and Enter,
  **Random** or **No Tagline**.
- **T** or the tag button in the reader header saves the tagline of the shown
  message (its last `... ` line) to the list, like MultiMail's tagline stealer.
- Ctrl/Cmd+Shift+T or **Taglines…** in the menu manages the list: add, edit
  (Enter) and delete (Delete) entries.

## Address book

Contacts are stored in `addressbook.txt` in the user data directory in
MultiMail's address book format (name line, address line, blank line; Internet
addresses are recognized by their `@`).

- **A** or **Address Book…** in the menu opens it: filter, add, edit and delete
  contacts, or **Write Message** to start a new message to the selected one.
- **Shift+A** or the person button in the reader header adds the author of the
  shown message.
- While writing, Ctrl+B or the contacts button next to **To** picks the
  recipient.

## Keyboard

| Keys | Action |
| --- | --- |
| Ctrl/Cmd+O, F5 | Open packet, reload |
| Ctrl/Cmd+N, Ctrl/Cmd+R, Ctrl/Cmd+L | New message, reply, forward |
| Ctrl/Cmd+Shift+E | Export replies |
| Space | Page down, then next unread message |
| N | Next unread message (continues into the next conference) |
| M | Toggle read/unread |
| Shift+C | Mark folder read |
| Arrows, Home/End, Page Up/Down | Navigate the focused pane |
| Tab/Shift+Tab, Enter | Cycle panes, open the selection |
| Ctrl/Cmd+F, Escape | Search, clear search |
| Ctrl/Cmd+T | List/thread view |
| T, Ctrl/Cmd+Shift+T | Save the message's tagline, manage taglines |
| A, Shift+A | Address book, add the author to it |
| Delete | Delete the selected outbox draft |
| Ctrl/Cmd+Shift+N, Ctrl/Cmd+W | New window, close window |
| Ctrl/Cmd+, | Settings |
| F1 | Show all shortcuts (editor keys while writing) |
| Ctrl+K, Ctrl+G, Ctrl+Q | Editor colors, CP437 table, quote panel |
| Ctrl+T, Ctrl+B | While writing: choose a tagline, pick the recipient |

## Translations

The interface texts are Fluent messages in `i18n/<language>/icy_mail.ftl`, loaded
with i18n-embed like the other icy tools. English (`i18n/en`) is the fallback, so
another language only needs the messages it translates; the desktop language is
picked at startup.

The shared `icy_engine_gui::egui` appearance, fonts, dialog shell, screen widget
and frame scheduling are also used by icy_term and icy_view. Native windows use
`AutoNoVsync`, matching those applications.

## Scope

QWK replies are exported as files, not sent over a network. Blue Wave, OMEN,
SOUP and OPX packets, direct mail delivery and the legacy frontend's composer
are not implemented. Read marks, recent packets, drafts, taglines and the
address book are stored in the user data directory, theme, zoom and monitor settings
in the configuration directory; pane sizes are session settings. Closing the main window also closes its additional windows.

## Validation

```sh
cargo test -p icy_mail --no-default-features --lib
cargo test -p icy_mail --bin icy_mail
cargo test -p icy_mail --no-default-features --features legacy-ui --bin icy_mail_legacy
```

GPU tests require a working wgpu adapter. They render actual terminal pixels,
check text selection/copy and scrolling, and cover desktop, 360x640, 360x240 and
HiDPI layouts in light and dark themes plus the composer with its quote panel,
color picker, character table, find field, tagline and recipient pickers, the outbox and start screen:

```sh
ICY_EGUI_SCREENSHOTS="$PWD/target/egui-mail" \
  cargo test -p icy_mail --bin icy_mail -- --include-ignored
```

No live network service is required. Tests use synthetic QWK packets and do not
modify user mail. The screenshot directory also receives a synthetic packet
for native startup checks. Platform file pickers and Windows/macOS window
behavior still require validation on those systems.