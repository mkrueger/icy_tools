use std::path::PathBuf;
use std::sync::Arc;

use icy_engine::{MouseEvent, Position, Selection};
use icy_engine_gui::ui::ExportDialogMessage;
use icy_net::protocol::TransferState;
use icy_parser_core::BaudEmulation;

use crate::{
    Address, TransferProtocol,
    terminal_thread::TerminalEvent,
    ui::{MainWindowMode, find_dialog, open_serial_dialog, select_bps_dialog, terminal_info_dialog, up_download_dialog},
};

#[derive(Debug, Clone)]
pub enum Message {
    DialingDirectory(crate::ui::dialogs::dialing_directory_dialog::DialingDirectoryMsg),
    SettingsDialog(crate::ui::dialogs::settings_dialog::SettingsMsg),
    CaptureDialog(crate::ui::dialogs::capture_dialog::CaptureMsg),
    ShowIemsi(crate::ui::dialogs::show_iemsi::IemsiMsg),
    FindDialog(find_dialog::FindDialogMsg),
    ExportDialog(ExportDialogMessage),
    TransferDialog(up_download_dialog::TransferMsg),
    SelectBpsMsg(select_bps_dialog::SelectBpsMsg),
    OpenSerialMsg(open_serial_dialog::OpenSerialMsg),
    TerminalInfo(terminal_info_dialog::TerminalInfoMsg),
    ApplyBaudEmulation,

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
    InitiateFileTransfer { protocol: TransferProtocol, is_download: bool },
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
    ViewportTick,                     // Update viewport animation
    SetScrollbackBufferSize(usize),   // Set scrollback buffer size
    McpCommand(Arc<crate::mcp::McpCommand>),
    ScrollbarHovered(bool), // Scrollbar hover state changed
    CursorLeftWindow,       // Cursor left the window

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
}
