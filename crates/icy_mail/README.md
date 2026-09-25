# Icy Mail

An offline QWK mail reader and reply-packet composer. The regular binary uses
egui with the existing wgpu ANSI renderer. QWK parsing, header indexing, lazy
body loading, caching and reply threading remain shared with the previous
frontend.

## Run

```sh
cargo run -p icy_mail -- [packet.qwk]
```

Open a packet from the toolbar, with Ctrl/Cmd+O, or by dropping it on the
window. ZIP archives must contain `CONTROL.DAT` and `MESSAGES.DAT`; standalone
REP archives are export files, not readable incoming packets.

The old frontend remains available:

```sh
cargo run -p icy_mail --no-default-features --features legacy-ui --bin icy_mail_legacy
```

`icy_mail_egui` is an alias of the default frontend. The isolated default build
does not depend on `icy_ui`; workspace-wide builds may enable it for other apps.

## Reading

- Resizable conference, message and reading panes; compact windows use pane tabs.
- Original compact, virtualized message rows and indented reply threads.
- Sortable conference area/name/count and message author/date/subject/line columns.
- Case-insensitive author, recipient and subject filtering. Thread mode uses the
  original reply-reference/normalized-subject grouping and activity order.
- ANSI/CP437 rendering, full-height scrolling, text/rectangular selection and copy.
- Asynchronous package and body loading with fixed workers and stale-result guards.
  A failed open leaves the previous packet intact and reports the error.
- Native additional windows, light/dark themes and fit-width/100% display.

Arrow keys, Home/End and Page Up/Down navigate the focused pane. Tab/Shift+Tab
cycle panes, and Enter advances from conferences to messages to the reader.
Ctrl/Cmd+T switches list/thread mode. Ctrl/Cmd+F focuses the filter; Escape
clears it. Ctrl/Cmd+Shift+N opens another window, Ctrl/Cmd+W closes it, and F5
reloads the current packet. Reloading selects the first message again.

Drag over the reader to select text; Alt-drag selects a rectangle. Ctrl/Cmd+C
copies the selection, Ctrl/Cmd+A selects the body, and the copy button copies
the entire message. Column tooltips retain text clipped by narrow columns.

## Writing replies

Open a QWK packet, then use **New** (Ctrl/Cmd+N), **Reply** (Ctrl/Cmd+R), or
**Forward** from the menu. Select the conference, edit the address, subject and
body, and save the draft. The menu lists drafts for later editing and deletion.
Cancelling a modified composer asks before discarding the changes. Drafts are
stored separately for each source packet and survive application restarts.

Use **Export REP** (Ctrl/Cmd+Shift+E) to save the drafts as a QWK `.rep` ZIP
archive containing the BBS's `.MSG` reply file. Transfer that file to the BBS
using your usual offline mail workflow. Export does not delete drafts; edit or
delete them explicitly when no longer needed. Unsupported characters and
invalid QWK fields are reported rather than silently replaced.

The shared `icy_engine_gui::egui` appearance, fonts, dialog shell, screen widget
and frame scheduling are also used by icy_term and icy_view. Native windows use
`AutoNoVsync`, matching those applications.

## Scope

QWK replies are exported as files, not sent over a network. Blue Wave, OMEN,
SOUP and OPX packets, address books, taglines, direct mail delivery and the
legacy frontend's composer are not implemented. The previous English UI labels
are retained. Theme, pane sizes and zoom are session settings, not written to
a user configuration. Closing the main window also closes its additional
windows.

## Validation

```sh
cargo test -p icy_mail --no-default-features --lib
cargo test -p icy_mail --bin icy_mail
cargo test -p icy_mail --no-default-features --features legacy-ui --bin icy_mail_legacy
```

GPU tests require a working wgpu adapter. They render actual terminal pixels,
check text selection/copy and scrolling, and cover desktop, 360x640, 360x240 and
HiDPI layouts in light and dark themes:

```sh
ICY_EGUI_SCREENSHOTS="$PWD/target/egui-mail" \
  cargo test -p icy_mail --bin icy_mail -- --include-ignored
```

No live network service is required. Tests use synthetic QWK packets and do not
modify user mail. The screenshot directory also receives a synthetic packet
for native startup checks. Platform file pickers and Windows/macOS window
behavior still require validation on those systems.