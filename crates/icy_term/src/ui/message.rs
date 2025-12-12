use std::path::PathBuf;
use std::sync::Arc;

use icy_engine::{MouseEvent, Position, ScreenMode, Selection};
use icy_engine_gui::ui::{ExportDialogMessage, HelpDialogMessage};
use icy_net::protocol::TransferState;
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::{BaudEmulation, MusicOption};

use crate::{
    Address, TransferProtocol,
    terminal_thread::TerminalEvent,
    ui::dialogs::about_dialog::AboutDialogMessage,
    ui::dialogs::capture_dialog::CaptureDialogMessage,
    ui::dialogs::protocol_selector::ProtocolSelectorMessage,
    ui::dialogs::terminal_info_dialog::TerminalInfoDialogMessage,
    ui::{MainWindowMode, find_dialog, open_serial_dialog, select_bps_dialog, up_download_dialog},
};

#[derive(Debug, Clone)]
pub enum Message {
    DialingDirectory(crate::ui::dialogs::dialing_directory_dialog::DialingDirectoryMsg),
    SettingsDialog(crate::ui::dialogs::settings_dialog::SettingsDialogMessage),
    CaptureDialog(CaptureDialogMessage),
    ShowIemsi(crate::ui::dialogs::show_iemsi::ShowIemsiMessage),
    ProtocolSelector(ProtocolSelectorMessage),
    FindDialog(find_dialog::FindDialogMsg),
    ExportDialog(ExportDialogMessage),
    TransferDialog(up_download_dialog::TransferMsg),
    SelectBpsDialog(select_bps_dialog::SelectBpsDialogMessage),
    OpenSerialMsg(open_serial_dialog::OpenSerialMsg),
    TerminalInfo(TerminalInfoDialogMessage),
    HelpDialog(HelpDialogMessage),
    AboutDialog(AboutDialogMessage),

    CancelFileTransfer,
    UpdateTransferState(TransferState),
    ShowExportScreenDialog,
    Connect(Address),
    Reconnect,
    CloseDialog(Box<MainWindowMode>),
    Hangup,
    ShowDialingDirectory,
    ShowSettings,
    ShowCaptureDialog,
    ShowFindDialog,
    ShowHelpDialog,
    ShowAboutDialog,
    ShowBaudEmulationDialog,
    ShowOpenSerialDialog,
    ConnectSerial,
    AutoDetectSerial,
    Upload,
    Download,
    SendLoginAndPassword(bool, bool),
    InitiateFileTransfer {
        protocol: TransferProtocol,
        is_download: bool,
    },
    OpenReleaseLink,
    StartCapture(String),
    StopCapture,
    ShowIemsiDialog,
    ShowTerminalInfoDialog,
    // Terminal thread events
    TerminalEvent(TerminalEvent),
    SendData(Vec<u8>),
    SendString(String),
    None,
    StopSound,
    ToggleFullscreen,
    OpenLink(String),
    Copy,
    Paste,
    ShiftPressed(bool),
    SelectBps(BaudEmulation),
    QuitIcyTerm,
    ClearScreen,
    ShowScrollback,
    SetFocus(bool),
    SendMouseEvent(MouseEvent),
    ScrollViewport(f32, f32),         // dx, dy in pixels
    ScrollViewportTo(bool, f32, f32), // smooth, x, y absolute position in pixels
    ScrollViewportXToImmediate(f32),  // x absolute position (horizontal scrollbar)
    ScrollViewportYToImmediate(f32),  // y absolute position (vertical scrollbar)
    ViewportTick,                     // Update viewport animation
    SetScrollbackBufferSize(usize),   // Set scrollback buffer size
    RefreshMonitorSettingsCache,      // Refresh cached monitor settings after settings change
    McpCommand(Arc<crate::mcp::McpCommand>),
    ScrollbarHovered(bool),  // Vertical scrollbar hover state changed
    HScrollbarHovered(bool), // Horizontal scrollbar hover state changed
    CursorLeftWindow,        // Cursor left the window

    // Terminal mouse events (from icy_engine_gui)
    MousePress(icy_engine_gui::TerminalMouseEvent),
    MouseRelease(icy_engine_gui::TerminalMouseEvent),
    MouseMove(icy_engine_gui::TerminalMouseEvent),
    MouseDrag(icy_engine_gui::TerminalMouseEvent),
    MouseScroll(icy_engine_gui::WheelDelta),

    // Selection messages
    StartSelection(Selection),
    UpdateSelection(Position),
    EndSelection,
    ClearSelection,

    FocusNext,
    FocusPrevious,
    RipCommand(bool, String),

    // Scripting
    ShowRunScriptDialog,
    RunScript(PathBuf),
    StopScript,

    // Terminal settings
    ApplyTerminalSettings {
        terminal_type: TerminalEmulation,
        screen_mode: ScreenMode,
        ansi_music: MusicOption,
    },

    // Zoom (unified)
    Zoom(icy_engine_gui::ZoomMessage),
}
