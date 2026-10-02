# Icy Mail

An offline QWK mail reader and reply-packet composer. The regular binary uses
egui with the existing wgpu ANSI renderer. QWK parsing, header indexing, lazy
body loading, caching and reply threading remain shared with the previous
frontend.

## Run

```sh
cargo run -p icy_mail -- [packet.qwk]
```

Open a packet with **Open**, Ctrl/Cmd+O, by dropping it on the window (the
start screen's drop zone lights up while a file is dragged over it), or from
the start screen's recent packets. Each recent packet is a card with the BBS
name, unread count, file size, packing date, and the starred messages and
drafts still to export, as they were when the packet was last open; hover a
card and press × to forget it. Packets are unpacked with [unarc-rs](https://github.com/mkrueger/unarc-rs),
so ZIP, ARJ, LHA/LZH, RAR, 7z, ARC, ZOO and the other formats it detects work;
the archive must contain `CONTROL.DAT` and `MESSAGES.DAT`. Standalone REP
archives are export files, not readable incoming packets.

Archive entries may expand to at most 10 GiB each; bulletin and screen files
retain their separate 16 MiB limit. Cold extraction and disabled caching keep
message data in memory; warm cache openings can show the first message before
background loading finishes. Large packets still require enough available RAM
for the uncompressed data and index.

### Packet extraction cache

Both frontends reuse extracted packets on disk in the application's OS cache
directory under `packet-extractions`. **Settings ▸ Packet cache**
controls retention in days since last use (default **30**); the persisted
`settings.toml` field is `extraction_cache_days`, also used by the legacy frontend.
Expired entries are cleaned up when another packet is opened, not by a background
timer. Setting **0** disables reuse and removes cached entries on the next
opening. Changes apply to the next packet load.

Cache identity uses the source packet's canonical path, file size and modification
time, plus change time (`ctime`) on Unix: changing that metadata invalidates the
extraction. Source validation is metadata-based, not a content hash; replacing a
packet while preserving all checked metadata may reuse its old extraction.
Extracted files are checked with CRC32 to detect cache corruption. Cache
discovery or storage problems are logged and opening falls back to uncached
extraction. On Unix, cache directories are private (`0700`) and files use `0600`.
The cache also persists compact binary message metadata (including decoded
styles and dates), the message index and the default, unfiltered thread rows.
The binary index is decoded in parallel chunks on subsequent openings.
An older cache gains this metadata on its next opening; stale or corrupt metadata
is rebuilt without discarding valid extracted message data. Both frontends reuse
the saved thread rows only when every message is in scope. Conference, personal,
unread, starred and search filters build their own trees, and collapsed threads
and current read marks are applied as usual.
Warm cached packets open with file-backed message bodies: the selected message
loads verified ranges on demand, without waiting for every body. After the first
selected body is prepared, the egui frontend preloads the remaining raw, verified
body chunks in the background. Selecting another body cancels that preload so
the new selection takes priority; packet changes and closing the window also
cancel it. Search remains available during preloading and keeps its separate
decoded-text cache lazy. Preload errors are logged and shown for the current
packet; stale results cannot replace a newer packet's state. Corrupt body chunks
attempt recovery from the unchanged source packet, otherwise loading reports
the error.

Cold extraction and disabled caching still load all message bodies into memory.
Background preloading also eventually makes the warm packet's raw bodies
resident, so large packets still need enough RAM. The disk cache avoids repeated
decompression, metadata parsing and default thread construction.

The old frontend remains available:

```sh
cargo run -p icy_mail --no-default-features --features legacy-ui --bin icy_mail_legacy
```

`icy_mail_egui` is an alias of the default frontend. The isolated default build
does not depend on `icy_ui`; workspace-wide builds may enable it for other apps.

## Reading

In egui, received messages and bulletin pages are prepared in background workers
for both display modes. Modern mode prepares classic and wide screens, analyzes text/art
blocks and renders art pixels there; only texture upload and text layout run on
the UI thread. Navigation and mode changes discard stale preparations. Editable
draft and composer previews still render locally.

- Three-pane layout: mailboxes and conferences, message list, and reader.
  The reader sits beside the list on wide windows and below it otherwise;
  **View ▸ Reading Pane** in the main menu or the settings fixes either placement.
  The ☰ main menu groups everything under **File** (new window, open, recent,
  reload, export, settings), **Message**, **View**, **Tools** (address book,
  taglines) and **Help**; **File ▸ New Window** (Ctrl/Cmd+Shift+N) opens
  another window for a second packet.
  Windows narrower than 760 px switch to a single pane with
  Folders/Messages/Message tabs; short windows collapse the message header.
- **Bulletins** appears when the packet contains welcome/news screens, bulletins,
  goodbye screens or a new-files list. Files up to 16 MiB are supported; large
  lists are loaded in sections of 2,048 lines; scrolling or Page Up/Down at
  the end of a section continues into the next one without visible page controls.
  A packet without those files does not show the folder.
- Conferences of the same network (`fsx.Chat`, `fsx.BBS Ads`, …) are grouped
  under a collapsible heading with the network's unread count; networks with
  unread mail start open, and Next Unread opens a closed one when it continues
  there. The envelope button next to the sort button lists only conferences
  with unread messages.
- **All Messages**, **Personal** (messages addressed to you) and each
  conference show unread counts. Showing a message marks it read; read marks
  are remembered per packet across restarts.
- Unread messages are bold with a dot; replied-to and private messages are
  flagged. Opening a folder selects its first unread message.
- **S**, the star beside a message or the star in the reader header keeps a
  message for later; **Starred** lists them. Stars are saved with the read
  marks, and a message unstarred there stays listed until the folder is
  reopened, so a slip can be undone.
- Sortable columns, list or thread view, and an **Unread** filter. In the
  thread view, **Mark Thread as Read** (Shift+M, also in the message's context
  menu) and **Collapse/Expand All Threads** (Shift+←/→, View menu) act on whole
  threads. In threads,
  the part of a reply's subject that repeats its parent (`Re:`, the whole
  subject or shared leading words such as a newsletter title) is dimmed. Dates in the
  lists read “Today”, “Yesterday” or the weekday during the past week; the
  reader header shows the full date. Search
  matches author, recipient, subject and message text, ignoring case; the
  magnifier in the search field limits it to some of them (the placeholder then
  names them, and the choice is kept). Message
  bodies are searched in parallel using Rayon, in the background across the packet, including messages
  not opened yet; folder and unread filters still apply to the results.
  Decoded, case-normalized body text is cached for the current packet, so later
  queries reuse it instead of reparsing every message. This cache uses additional
  memory and is released when another packet is opened or the window is closed.
  A spinner indicates an ongoing text search. Changing or clearing the query
  cancels the previous search. ANSI formatting is ignored and CP437 characters
  are decoded for searching. Malformed ANSI formatting is logged without
  aborting the search; message-loading failures are logged and abort the search
  with an error, including unrecoverable file-backed cache reads. Failed searches
  never publish successful, incomplete body matches.
  Matches are highlighted in the list, message header and displayed message
  text. Body highlights update with the filter and disappear when it is cleared,
  without changing the original message colors, text selection or copied text.
- The reader header links to the referenced message (“reply to #n”) and offers
  reply, forward, mark read/unread and previous/next actions; copying the
  text, saving the tagline and adding the author are in its **⋯** menu.
- A visible **Next Unread** action in the toolbar shares the N/Space navigation
  through conferences; the status bar tracks read/total/unread messages across
  the packet. On narrow windows, advancing opens the message pane.
- ANSI/CP437 rendering, text/rectangular selection and copy, zoom, light/dark
  themes and additional native windows.
- **Modern message display** (the default; Settings ▸ Messages ▸ Message
  display switches to the classic BBS terminal): messages are shown as
  selectable text, in the fixed-width font unless the proportional one is
  chosen. Text wraps at the pane's width rather than at 80 columns, ANSI
  colors are kept but adjusted where they would be hard to read on the theme,
  CP437 box drawing becomes Unicode, and plain quote lines are dimmed; quotes
  of six lines or more fold below their first two lines (click to unfold; a
  search match inside opens them). Tables
  and lines with aligned columns use the fixed-width font. ANSI art is drawn
  with the BBS font on the 80 column grid, exactly as in the classic view, at
  the size of the fixed-width text: a paragraph with block graphics (▀ ▄ █ ░ ▒
  ▓ …) or colored backgrounds becomes a picture (bullets like ■ or ► stay text), together with short colored or aligned
  paragraphs up to the next piece of art (like a BBS ad); the next plain text
  paragraph switches back to text. Only text lines the terminal wrapped are
  joined again. The font (proportional or
  fixed width) and its size are set there too; Ctrl+mouse wheel over the text
  changes the size. The setting applies everywhere text is shown: messages,
  bulletins and the outbox preview.
- **Message ▸ Save Message…** stores the shown message's text exactly as it is
  in the packet (`<conference>-<number>.ans`), **Save Message as UTF-8…** as a
  `.txt` with CP437 characters converted and ANSI codes kept; both are also in
  the reader header's **⋯** menu. Handy for other viewers or to report a
  display problem.
- **Settings** (Ctrl/Cmd+,): theme, message display and zoom, and the shared monitor
  emulation (monitor color, scaling, filtering, brightness, contrast, CRT
  effects…). Changes preview live; Cancel restores the previous values. They
  are saved to `settings.toml` in the icy_mail configuration directory.
- Packets and message bodies load in the background; a failed open keeps the
  current packet and reports the error.

Drag over the reader to select text; Alt-drag selects a rectangle. Ctrl/Cmd+C
copies the selection, Ctrl/Cmd+A selects the body, and the copy button copies
the entire message. Double-click selects a word, triple-click selects a line,
and Shift-click extends the current selection. A plain mouse press or Escape clears
the selection; double/triple-clicks must be close together in both time and position.
Escape in the reader clears the selection before clearing a
message filter; search fields and open dialogs retain their own Escape behavior.

## Writing replies

Use **New** (Ctrl/Cmd+N) from the toolbar, or **Reply** (Ctrl/Cmd+R) and
**Forward** (Ctrl/Cmd+L) from the reader header, the message context menu or
**Message** in the main menu. The composer
replaces the reader pane: choose the conference and privacy, edit recipient,
subject and sender (with QWK's 25 character limits shown), and write the
message in the BBS editor described below.
Problems that would block export are listed live. Save with Ctrl/Cmd+S or
Ctrl/Cmd+Enter; Escape cancels and asks before discarding changes. Closing a
window with an unsaved composer asks as well.

### Message editor

The message text is written on an 80 column CP437 terminal, modeled after
Synchronet's SlyEdit. Lines wrap at 79 columns (a dashed line marks the
column) and are saved as they appear on screen. With the modern message display
the terminal fills the pane's width on a page in the theme's colors, like the
reading view; the classic display keeps the black terminal and the zoom setting.
An empty message shows a hint where to start. Typing inserts or, after Insert, overwrites; the terminal's status bar
shows the mode, line and column, and the current color. The tools around the
terminal are regular panels, reachable from the toolbar above it or by keyboard:

- **Colors** (Ctrl+K or the color button): a picker with the 16 foreground and
  8 background colors, blink and a preview. Click swatches or use the arrows
  (left/right foreground, up/down background), 0-F and Space. A picked color
  takes effect at once, each as its own undo step; Enter, Escape or a click
  outside closes the picker. With a selection the selected text is recolored,
  otherwise the color applies to what you type next. Colors are stored as ANSI sequences in the
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
- **Editing**: Ctrl+D deletes the line, Ctrl+Backspace/Ctrl+Delete delete the
  word before/after the caret, Ctrl+Z/Ctrl+Y undo and redo, Ctrl+A selects all,
  Shift with arrows or the mouse selects, and Ctrl+C/X/V as well as
  Ctrl+Insert, Shift+Delete and Shift+Insert copy, cut and paste on every
  platform.

Escape first closes the color picker, quote panel, character table, find field
or selection before it cancels the message.

Saved drafts appear in the **Outbox**. Select one to preview it in the message terminal
(with colors, like received mail), press Enter or
double-click to edit it, and Delete to remove it. Drafts are stored separately
for each source packet and survive restarts. Saving a draft does not send it;
the Outbox shows the remaining steps to deliver it.

**Export Replies** (Ctrl/Cmd+Shift+E) saves all drafts as a QWK `.rep` ZIP
archive containing the BBS's `.MSG` reply file. Transfer that file to the BBS
using your usual offline mail workflow. Drafts with problems are listed instead
of being exported. Export does not delete drafts. Unsupported characters and
invalid QWK fields are reported rather than silently replaced. After a successful
export, the confirmation shows the full `.rep` file path and offers **Open Folder**
so you can find the file to upload. The packet is not sent automatically.

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
- **T** or **Save Tagline** in the reader header's **⋯** menu saves the tagline of the shown
  message (its last `... ` line) to the list, like MultiMail's tagline stealer.
- Ctrl/Cmd+Shift+T or **Tools ▸ Taglines…** manages the list: add, edit
  (Enter) and delete (Delete) entries.

## Address book

Contacts are stored in `addressbook.txt` in the user data directory in
MultiMail's address book format (name line, address line, blank line; Internet
addresses are recognized by their `@`).

- **A** or **Tools ▸ Address Book…** opens it: filter, add, edit and delete
  contacts, or **Write Message** to start a new message to the selected one.
- **Shift+A** or the reader header's **⋯** menu adds the author of the
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
| Shift+M | Mark the thread read |
| Shift+←/→ | Collapse/expand all threads |
| S | Star or unstar the message |
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

German (`i18n/de`) covers the complete Icy Mail interface. To test it on Linux
without changing the desktop language, launch a new process with:

```sh
LANGUAGE=de_DE LANG=de_DE.UTF-8 cargo run -p icy_mail -- [packet.qwk]
```

Use `LANGUAGE=en LANG=en_US.UTF-8` to compare the English interface. Catalog
coverage, placeholders, German locale selection and plural forms are checked by
`cargo test -p icy_mail --test localization`.

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

An optional large-packet test checks loading message data larger than 1 GiB
without modifying the supplied packet:

```sh
ICY_MAIL_TEST_PACKET=/path/to/large.qwk \
  cargo test --release -p icy_mail --lib qwk::tests::large_packet_opens -- --ignored
```

To validate disk-cache round trips in an isolated cache directory and print
timings for one cold load and three warm loads:

```sh
ICY_MAIL_TEST_PACKET=/path/to/large.qwk \
  cargo test --release -p icy_mail --lib qwk::tests::large_packet_cache_roundtrip -- --ignored --nocapture
```

To measure an existing cache in the normal OS default cache directory, including
cached loading, Reader rebuilding and fresh thread construction:

```sh
ICY_MAIL_TEST_PACKET=/path/to/cached.qwk ICY_MAIL_PERF=1 \
  cargo test --release -p icy_mail --lib qwk::cache::tests::existing_packet_cache_timings -- --ignored --nocapture
```

This test requires an existing extraction cache, upgrades older cache metadata
when needed and prints the timings. It does not modify the source packet.

To measure the actual egui open-to-first-accepted-body pipeline in both Classic
and Modern modes without creating a GPU device:

```sh
ICY_MAIL_TEST_PACKET=/home/mkrueger/work/bbs/BEERS24.qwk ICY_MAIL_PERF=1 \
  cargo test --release -p icy_mail --bin icy_mail_egui tests::cached_packet_open_to_first_prepared_body_timings -- --ignored --nocapture
```

This uses the normal OS default extraction cache, while isolating settings,
drafts, read marks and recent packets in temporary storage. It prints cache
loading plus Reader setup, selected-body worker/acceptance time and total
open-to-first-prepared-body time. Modern preparation includes CPU art pixels and
texture registration, not GPU painting. Raw preloading is cancelled after the
first accepted body; startup does not scan or normalize every message for search.
The source packet is not modified. Set `ICY_MAIL_TEST_PACKET` to another cached
packet to measure it instead.

A reference release run with 526,386 messages (1.39 GiB of raw message data),
an already-upgraded cache and the default list view measured:

| Display | Cache load + Reader setup | Body worker + acceptance | Open to first prepared body |
| --- | ---: | ---: | ---: |
| Classic | 202.77 ms | 0.484 ms | 203.25 ms |
| Modern | 181.87 ms | 0.434 ms | 182.30 ms |

These are individual reference timings, not performance guarantees or GPU paint
times. Separate final core measurements put metadata-only warm cache opening at
35–43 ms; the frontend table additionally includes Reader and application-state
setup and selected-body acceptance. Older caches can incur a one-time metadata/manifest upgrade on first
opening; that work is not included in these warm-cache measurements.