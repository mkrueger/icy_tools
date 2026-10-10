# IcyTerm Changelog

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
