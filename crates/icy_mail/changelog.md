# IcyMail Changelog

## 0.1.1 - 2026-10-13

### Added

- QWK replies end with an `--- IcyMail v<version>` tear line below the tagline,
  as in MultiMail. Blue Wave reply headers name IcyMail and its version so the
  door can build its tear line.
- **View ▸ Show Hidden Lines** (X) shows FidoNet hidden (Ctrl-A) lines in their
  own color.
- The tagline list can import tagline files (one per line, duplicates skipped),
  export the list and open `taglines.txt` in a text editor; changes made outside
  IcyMail show up in the open list.

### Changed

- The contact form of the address book uses the full dialog width, keeps its
  buttons together on the right and tells whether the address is FidoNet netmail
  or an Internet address. FidoNet addresses with a `@domain` are no longer saved
  as Internet addresses.
- `taglines.txt` and `addressbook.txt` are read as UTF-8 only, like all other
  data files; CP437 is used only for message text in packets.
- FidoNet hidden lines are no longer shown in messages, quotes, search or
  tagline detection unless shown with X. Synchronet packets keep Ctrl-A color codes.

### Fixed

- Large ANSI art in modern reading mode is split into GPU-safe textures instead
  of crashing when a picture exceeds the graphics device's texture size limit.
  Art retains its full resolution, size and zoom.
- Mystic QWK packets using `0xFF` for active messages retain all messages and
  populated conferences instead of appearing empty. Existing metadata indexes
  are rebuilt so an empty cached index cannot hide the corrected messages.
- QWK packets containing password-marked messages or comments to the sysop
  no longer hang or mistake message bodies for headers. Deleted messages are
  skipped by their full block lengths, and invalid block counts report a
  loading error.
- QWK conference lists accept both count-minus-one and actual-count conventions,
  retaining the last conference and screen names, including numeric welcome
  screens. Cached message indexes are rebuilt for the corrected parser.

## 0.1.0 - 2026-10-10

First release.

### Added

- Offline QWK and Blue Wave level 2/3 packet reading, with automatic format
  detection from archive contents and support for ZIP, ARJ, LHA/LZH, RAR, 7z,
  ARC, ZOO and other supported archive formats.
- Native egui frontend with the shared wgpu ANSI renderer, responsive
  three-pane and narrow-window layouts, drag-and-drop opening, recent packets
  and independent windows.
- Classic ANSI and modern message display, preserving text-art frames and
  colors, with image zoom, text selection, clipboard copying and clickable links.
- Threaded message lists, conference grouping by network, unread and starred
  filters, read marks, quote folding and referenced-message navigation.
- Background search across selected message fields, including decoded message
  bodies, with cancellation and visible loading errors.
- Welcome, news, goodbye, bulletin and new-files screens alongside messages.
- Verified packet extraction caches, persisted message indexes, lazy body
  loading and background rendering and preloading.
- New messages, replies and forwards with persistent drafts and an outbox.
- Message editing with quoting, find, color controls, a character table,
  signatures, taglines and an address book.
- QWK `.REP` and Blue Wave `.NEW` reply-packet export, plus import of reply
  archives as editable drafts in the corresponding incoming packet.
- QWKE extended reply fields for packets advertising support.
- Message and conference saving with headers and retained ANSI formatting.
- Configurable themes, zoom, monitor settings, cache retention and localized
  controls.

### Scope

- Replies are exported as files for upload to a BBS; IcyMail does not send them
  over a network.
- Standalone reply archives are not incoming mail packets. Open the matching
  QWK or Blue Wave packet before importing replies.
- Blue Wave offline door configuration, file requests and addressed netmail
  export are not supported.
- `icy_mail` uses the egui frontend by default; `icy_mail_egui` is an alias.
  The previous frontend remains available with the `legacy-ui` feature.
