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
it). ZIP archives must contain `CONTROL.DAT` and `MESSAGES.DAT`; standalone
REP archives are export files, not readable incoming packets.

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
  matches author, recipient and subject.
- The reader header links to the referenced message (“reply to #n”) and offers
  reply, forward, mark read/unread, copy and previous/next actions.
- ANSI/CP437 rendering, text/rectangular selection and copy, fit-width or 100%
  zoom, light/dark themes and additional native windows.
- Packets and message bodies load in the background; a failed open keeps the
  current packet and reports the error.

Drag over the reader to select text; Alt-drag selects a rectangle. Ctrl/Cmd+C
copies the selection, Ctrl/Cmd+A selects the body, and the copy button copies
the entire message.

## Writing replies

Use **New** (Ctrl/Cmd+N), **Reply** (Ctrl/Cmd+R) or **Forward** (Ctrl/Cmd+L)
from the toolbar, the message context menu or the reader header. The composer
replaces the reader pane: choose the conference and privacy, edit recipient,
subject and sender (with QWK's 25 character limits shown), and write the body.
Problems that would block export are listed live. Save with Ctrl/Cmd+S or
Ctrl/Cmd+Enter; Escape cancels and asks before discarding changes. Closing a
window with an unsaved composer asks as well.

Saved drafts appear in the **Outbox**. Select one to preview it, press Enter or
double-click to edit it, and Delete to remove it. Drafts are stored separately
for each source packet and survive restarts.

**Export Replies** (Ctrl/Cmd+Shift+E) saves all drafts as a QWK `.rep` ZIP
archive containing the BBS's `.MSG` reply file. Transfer that file to the BBS
using your usual offline mail workflow. Drafts with problems are listed instead
of being exported. Export does not delete drafts. Unsupported characters and
invalid QWK fields are reported rather than silently replaced.

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
| Delete | Delete the selected outbox draft |
| Ctrl/Cmd+Shift+N, Ctrl/Cmd+W | New window, close window |
| F1 | Show all shortcuts |

The shared `icy_engine_gui::egui` appearance, fonts, dialog shell, screen widget
and frame scheduling are also used by icy_term and icy_view. Native windows use
`AutoNoVsync`, matching those applications.

## Scope

QWK replies are exported as files, not sent over a network. Blue Wave, OMEN,
SOUP and OPX packets, address books, taglines, direct mail delivery and the
legacy frontend's composer are not implemented. Read marks, recent packets and
drafts are stored in the user data directory; theme, pane sizes and zoom are
session settings. Closing the main window also closes its additional windows.

## Validation

```sh
cargo test -p icy_mail --no-default-features --lib
cargo test -p icy_mail --bin icy_mail
cargo test -p icy_mail --no-default-features --features legacy-ui --bin icy_mail_legacy
```

GPU tests require a working wgpu adapter. They render actual terminal pixels,
check text selection/copy and scrolling, and cover desktop, 360x640, 360x240 and
HiDPI layouts in light and dark themes plus the composer, outbox and start
screen:

```sh
ICY_EGUI_SCREENSHOTS="$PWD/target/egui-mail" \
  cargo test -p icy_mail --bin icy_mail -- --include-ignored
```

No live network service is required. Tests use synthetic QWK packets and do not
modify user mail. The screenshot directory also receives a synthetic packet
for native startup checks. Platform file pickers and Windows/macOS window
behavior still require validation on those systems.