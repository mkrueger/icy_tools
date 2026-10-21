# IcyMail Changelog

## Unreleased

### Added

- FidoNet timezone declarations (`TZUTC` and legacy `TZUTCINFO`) convert message
  dates to the reader's local timezone. New JAM messages record their timezone;
  IcyBoard's RFC 3339 dates retain precedence over tossed declarations.
- Threading uses exact FidoNet `MSGID`/`REPLY` links when numeric reply pointers
  are missing or unavailable, before falling back to subjects. Links stay within
  their area, and ambiguous duplicate IDs are logged and ignored.
- JAM message bases are read and written in place: open a `.jhr` file, or an
  IcyBoard installation's `icboard.toml` to get all of its conference areas and
  the email base. Kludge subfields appear as hidden lines and declare the
  charset; IcyBoard areas are read and written as UTF-8. There is no Outbox:
  **Save Message** writes a new message or reply straight into its area under
  JAM's lock, linked to the message number and `MSGID` it answers, and shows it.
  Messages can be marked deleted and restored (Del); they stay listed, struck
  through, until **Pack Message Base…** removes them without renumbering, along
  with old or received private messages if asked to.
- Messages of a JAM base can be edited in place (Ctrl+E), moved or copied to
  another area, and inspected with **Message Info…** (I), whose attribute flags
  can be changed. **Check Message Base…** verifies and repairs the files.
- Read marks follow the sysop's lastread records of IcyBoard bases in both
  directions; the pointer only moves past messages read without a gap.
- Netmail: IcyBoard's netmail base is listed, and netmail asks for the
  destination address, which answers take from the original.
- Message reading supports all FTS-5003.001 character sets, including national
  ISO 646 variants, DOS and Windows code pages, Macintosh, Latin and UTF-8, plus
  KOI8-R, KOI8-U and ISO-8859-5. The encoding comes from the message's CHRS,
  CHARSET or CODEPAGE kludge, with a CP437 default that can be changed globally,
  per conference or per message (**View ▸ Read Message As**). Declared encodings
  also decode message headers. Conversion tables are bundled, without depending
  on the system's iconv availability.
- Synchronet's `HEADERS.DAT` is read: `Utf8: true` marks UTF-8 messages, whose
  lines end with LF, and its `Sender`, `To` and `Subject` fields replace the
  truncated 25-character QWK header fields. Cached message indexes are rebuilt.
- UTF-8 replies for packets with `HEADERS.DAT`: the composer switches a draft
  between CP437 in the BBS editor and UTF-8 in a plain-text editor with input
  methods, the quote panel and a 79-column guide. Answers to UTF-8 messages start
  in UTF-8. Replies are exported with LF line ends and a `HEADERS.DAT` that marks
  them `Utf8: true` with their whole header fields, and import back as UTF-8 drafts.
- The reader header shows the message's encoding and whether the message
  declared it, a conference or the default chose it, or it was chosen by hand;
  clicking it offers **Read Message As**.
- **View ▸ Pipe Color Codes in Conference** renders Renegade/Telegard `|00`–`|31`
  color codes in a conference's messages, remembered per conference.
- Ignore rules hide or collapse messages by author, recipient or subject, with
  `*` and `?` wildcards; subjects match whole threads. **Ignore Author** and
  **Ignore Subject** are in the reader header's **⋯** menu, the rules in
  **Settings ▸ Ignore**. Hidden messages leave the lists and unread counts;
  messages addressed to you are only collapsed.
- Quoting rejoins paragraphs the original editor hard-wrapped and wraps them at
  the reply's margin, keeping lists, tables, indented text, signatures and art
  as they stand (**Settings ▸ Writing**, on by default).

### Fixed

- Large ANSI art in modern reading mode is split into GPU-safe textures instead
  of crashing when a picture exceeds the graphics device's texture size limit.
  Art retains its full resolution, size and zoom.
- Long requoted lines keep their quote prefix on every wrapped line.
- Replies and forwards use whole `HEADERS.DAT` recipient and subject fields.
  CP437 replies preserve their original bytes, while UTF-8 replies use decoded
  names and subjects.
- IME input in the UTF-8 editor closes quote navigation before it can consume
  composition keys, so Enter inserts a newline rather than an unwanted quote.

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
