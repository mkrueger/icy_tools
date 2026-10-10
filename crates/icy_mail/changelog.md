# IcyMail Changelog

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
