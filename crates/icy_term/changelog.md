# IcyTerm Changelog

## Unreleased

## 0.9.1 - 2026-10-30

Changes since 0.9.0.

### Added

- Native Kermit file transfers for uploads and batch downloads.
- DoorWay mode (`CSI = 255 h`/`l`): a NUL makes the next byte a literal CP437
  glyph, and extended keys are sent as NUL plus the BIOS scan code.
- Control Representation Mode (`CSI 3 h`/`l`) with Linux console semantics:
  BEL, HT and DEL are displayed as CP437 glyphs instead of being executed, so a
  shown BEL does not ring the bell.

### Changed

- Manual Zmodem, Ymodem and Kermit downloads start directly in the configured
  download folder instead of asking for a folder first.
- Kermit negotiates long packets up to 4096 bytes and repeat compression,
  avoiding the latency bottleneck of classic short packets.
- ANSI music follows the CTerm manual for pitch, persistent playback settings,
  dotted notes and pauses, lowercase commands, and music introducer selection.

### Fixed

- Windows release builds no longer open an extra console window alongside the
  terminal, including when using the `icy_term_egui` compatibility alias.
- Scrolling text redraws correctly, and reducing the scrollback limit trims
  existing history.
- Terminal resets restore the configured music option and clear CTerm attribute
  modes; palette resets restore the terminal's default palette.
- IGS loop parameters continue across underscore line breaks.
- RIP text, erase, scroll, and palette updates invalidate cached pixels even
  when they arrive without a RIP graphics command.
- SkyPix output refreshes after incoming text and graphics instead of remaining
  blank. Its 640x200 native screen displays at 640x400 with doubled scanlines,
  including correctly sized rendering tiles and resolution resets.
- SkyPix Amiga fonts retain full glyph widths and heights, proportional spacing,
  and bearings instead of being clipped to an 8x32 terminal cell. Font resets,
  transparent text clipping, and custom-font carriage returns are corrected.

## 0.9.0 - 2026-10-10

Changes since 0.8.4.

### Added

- Native egui desktop frontend using the shared wgpu terminal renderer. The
  `icy_term` binary uses it by default; `icy_term_egui` remains a compatibility
  alias and the legacy frontend is available with the `legacy-ui` feature.
- Per-connection SOCKS5 proxies with remote DNS for Telnet, Raw TCP, Rlogin and
  SSH, including Tor, I2P and custom proxy settings.
- SyncTERM physical key press/release report support.
- System clipboard integration for text, RTF, images and Icy data.
- Redial from the dialing directory and jump from transfer counters to matching
  transfer log entries.

### Changed

- Redesigned the dialing directory, settings, terminal information, upload and
  download dialogs, with responsive layouts and shared dialog styling.
- Restored the desktop workflows for connections, search, scrollback, capture,
  replay, export, Lua scripting and the opt-in MCP server in the egui frontend.
- Shared export and keyboard shortcut dialogs, mouse-wheel zoom and updated
  application icons across the desktop tools.
- Improved rendering performance for streamed Sixel video and fractional zoom.

### Fixed

- ZMODEM downloads accept subpackets up to 8 KiB in both ZMODEM modes; ordinary
  ZMODEM uploads continue to use 1 KiB blocks.
- Closing a completed transfer dialog restores terminal keyboard focus.
- Announced transfers start automatically, and transfers that receive no files
  do not unnecessarily look up a download directory.
- Remote hangups display `NO CARRIER` only once.
- Restored the mouse-reporting toggle in the terminal information dialog.
- Improved macOS Option-key character input, Sixel scaling and terminal
  attribute serialization safety.

### Compatibility

- Existing phonebook and settings formats are retained. The egui frontend
  detects configuration conflicts rather than overwriting external changes.
- SOCKS5 proxying is not available for WebSocket connections.
