use core::panic;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
    vec,
};

use parking_lot::Mutex;

use crate::{
    McpHandler,
    commands::{cmd, create_icy_term_commands},
    mcp::{self, McpCommand, types::ScreenCaptureFormat},
    scripting::parse_key_string,
    ui::{
        Message,
        dialogs::{find_dialog, select_bps_dialog},
        up_download_dialog::{self, FileTransferDialogState},
    },
};

use clipboard_rs::Clipboard;
use iced::{Element, Event, Task, Theme, keyboard, window};
use icy_engine::Position;
use icy_engine_gui::{MonitorSettings, command_handler, error_dialog, music::music::SoundThread, ui::DialogStack};
use icy_net::{ConnectionType, telnet::TerminalEmulation};
use tokio::sync::mpsc;

use crate::{
    Address, AddressBook, Options,
    terminal::terminal_thread::{ConnectionConfig, TerminalCommand, TerminalEvent, create_terminal_thread},
    ui::dialogs::{capture_dialog, terminal_info_dialog},
    ui::{MainWindowState, dialing_directory_dialog, protocol_selector, settings_dialog, show_iemsi, terminal_window},
};

// Command handler for MainWindow keyboard shortcuts
command_handler!(MainWindowCommands, create_icy_term_commands(), => Message {
    // View
    cmd::VIEW_FULLSCREEN => Message::ToggleFullscreen,
    cmd::VIEW_ZOOM_IN => Message::Zoom(icy_engine_gui::ZoomMessage::In),
    cmd::VIEW_ZOOM_OUT => Message::Zoom(icy_engine_gui::ZoomMessage::Out),
    cmd::VIEW_ZOOM_RESET => Message::Zoom(icy_engine_gui::ZoomMessage::Reset),
    cmd::VIEW_ZOOM_FIT => Message::Zoom(icy_engine_gui::ZoomMessage::AutoFit),
    // Help
    cmd::HELP_SHOW => Message::ShowHelpDialog,
    cmd::HELP_ABOUT => Message::ShowAboutDialog,
    // Edit
    cmd::EDIT_COPY => Message::Copy,
    cmd::EDIT_PASTE => Message::Paste,
    // Connection
    cmd::CONNECTION_DIALING_DIRECTORY => Message::ShowDialingDirectory,
    cmd::CONNECTION_HANGUP => Message::Hangup,
    cmd::CONNECTION_SERIAL => Message::ShowOpenSerialDialog,
    // Transfer
    cmd::TRANSFER_UPLOAD => Message::Upload,
    cmd::TRANSFER_DOWNLOAD => Message::Download,
    // Login
    cmd::LOGIN_SEND_ALL => Message::SendLoginAndPassword(true, true),
    cmd::LOGIN_SEND_USER => Message::SendLoginAndPassword(true, false),
    cmd::LOGIN_SEND_PASSWORD => Message::SendLoginAndPassword(false, true),
    // Terminal
    cmd::TERMINAL_CLEAR => Message::ClearScreen,
    cmd::TERMINAL_SCROLLBACK => Message::ShowScrollback,
    cmd::TERMINAL_FIND => Message::ShowFindDialog,
    // Capture & Export
    cmd::CAPTURE_START => Message::ShowCaptureDialog,
    cmd::CAPTURE_EXPORT => Message::ShowExportScreenDialog,
    // Scripting
    cmd::SCRIPT_RUN => Message::ShowRunScriptDialog,
    // Application
    cmd::APP_SETTINGS => Message::ShowSettings,
    cmd::APP_QUIT => Message::QuitIcyTerm,
    cmd::APP_ABOUT => Message::ShowAboutDialog,
});

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum MainWindowMode {
    ShowTerminal,
    #[default]
    ShowDialingDirectory,
    FileTransfer(bool),
    ShowFindDialog,
    ShowOpenSerialDialog(bool),
}

pub struct MainWindow {
    pub id: usize,
    pub state: MainWindowState,
    pub dialing_directory: dialing_directory_dialog::DialingDirectoryState,
    pub options: Arc<Mutex<Options>>,
    pub terminal_window: terminal_window::TerminalWindow,
    pub find_dialog: find_dialog::DialogState,
    pub file_transfer_dialog: up_download_dialog::FileTransferDialogState,
    pub open_serial_dialog: super::open_serial_dialog::OpenSerialDialog,
    /// Dialog stack for trait-based modal dialogs (new unified system)
    pub dialogs: DialogStack<Message>,

    // Capture state (persisted across dialog shows)
    pub is_capturing: bool,
    pub capture_directory: String,

    // sound thread
    pub sound_thread: Arc<Mutex<SoundThread>>,

    // Terminal thread communication
    terminal_tx: mpsc::UnboundedSender<TerminalCommand>,
    terminal_rx: Option<mpsc::UnboundedReceiver<TerminalEvent>>,

    // Connection state
    is_connected: bool,
    connection_time: Option<Instant>,
    current_address: Option<Address>,
    last_address: Option<Address>,
    pause_message: Option<String>,
    terminal_emulation: TerminalEmulation,

    _is_fullscreen_mode: bool,
    _last_pos: Position,
    shift_pressed_during_selection: bool,
    _use_rip: bool,

    pub initial_upload_directory: Option<PathBuf>,
    pub show_find_dialog: bool,
    show_disconnect: bool,

    pub mcp_rx: McpHandler,
    /// Pending MCP script response channel
    pub pending_script_response: Option<crate::mcp::SenderType<crate::mcp::ScriptResult>>,
    pub title: String,
    pub effect: i32,
    commands: MainWindowCommands,
    /// Cached monitor settings as Arc for efficient rendering
    cached_monitor_settings: Arc<MonitorSettings>,
}

impl MainWindow {
    pub fn new(
        id: usize,
        mode: MainWindowMode,
        sound_thread: Arc<Mutex<SoundThread>>,
        addresses: Arc<Mutex<AddressBook>>,
        options: Arc<Mutex<Options>>,
    ) -> Self {
        let default_capture_path: PathBuf = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let terminal_window: super::TerminalWindow = terminal_window::TerminalWindow::new(sound_thread.clone());
        let edit_screen = terminal_window.terminal.screen.clone();

        let (terminal_tx, terminal_rx) = create_terminal_thread(edit_screen.clone(), addresses.clone());

        let serial = options.lock().serial.clone();
        let cached_monitor_settings = Arc::new(options.lock().monitor_settings.clone());

        Self {
            effect: 0,
            id,
            title: format!("iCY TERM {}", *crate::VERSION),
            state: MainWindowState {
                mode,
                #[cfg(test)]
                options_written: false,
            },
            dialing_directory: dialing_directory_dialog::DialingDirectoryState::new(addresses),
            options,
            terminal_window,
            find_dialog: find_dialog::DialogState::new(),
            file_transfer_dialog: FileTransferDialogState::new(),
            open_serial_dialog: super::open_serial_dialog::OpenSerialDialog::new(serial),
            dialogs: DialogStack::new(),

            is_capturing: false,
            capture_directory: default_capture_path.to_string_lossy().to_string(),

            terminal_tx,
            terminal_rx: Some(terminal_rx),

            is_connected: false,
            connection_time: None,
            current_address: None,
            last_address: None,
            pause_message: None,

            _is_fullscreen_mode: false,
            _last_pos: Position::default(),
            shift_pressed_during_selection: false,
            _use_rip: false,
            initial_upload_directory: None,
            show_find_dialog: false,
            show_disconnect: false,
            sound_thread,
            mcp_rx: None,
            pending_script_response: None,
            terminal_emulation: TerminalEmulation::Ansi,
            commands: MainWindowCommands::new(),
            cached_monitor_settings,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DialingDirectory(msg) => self.dialing_directory.update(msg),
            Message::Connect(address) => {
                let modem = if matches!(address.protocol, ConnectionType::Modem) {
                    let options = &self.options.lock();
                    // Find the modem in options that matches the modem_id
                    let modem_opt = options.modems.iter().find(|m| m.name == address.modem_id);

                    if let Some(modem_config) = modem_opt {
                        Some(modem_config.clone())
                    } else {
                        // No modem configured - show error and abort connection
                        let error_msg = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "connect-error-no-modem-configured");
                        log::error!("{}", error_msg);

                        // Display error message in terminal
                        {
                            let mut screen = self.terminal_window.terminal.screen.lock();
                            if let Some(editable) = screen.as_editable() {
                                editable.clear_screen();

                                // Write error message
                                for ch in error_msg.chars() {
                                    editable.print_char(icy_engine::AttributedChar::new(
                                        ch,
                                        icy_engine::TextAttribute::from_color(4, 0), // Red on black
                                    ));
                                }
                                editable.cr();
                                editable.lf();
                            }
                        }

                        self.state.mode = MainWindowMode::ShowTerminal;
                        return Task::none();
                    }
                } else {
                    None
                };
                let options = &self.options.lock();

                self.terminal_emulation = address.terminal_type;

                // Send connect command to terminal thread
                let config = ConnectionConfig {
                    connection_info: address.clone().into(),
                    terminal_type: address.terminal_type,
                    baud_emulation: address.baud_emulation,
                    window_size: (80, 25),
                    timeout: Duration::from_secs(30),
                    user_name: if address.user_name.is_empty() {
                        None
                    } else {
                        Some(address.user_name.clone())
                    },
                    password: if address.password.is_empty() { None } else { Some(address.password.clone()) },

                    proxy_command: None, // fill from settings if needed
                    modem,
                    ansi_music: address.ansi_music,
                    screen_mode: address.get_screen_mode(),
                    iemsi_auto_login: options.iemsi.autologin,
                    auto_login_exp: address.auto_login.clone(),
                    max_scrollback_lines: options.max_scrollback_lines,
                    transfer_protocols: options.transfer_protocols.clone(),
                    mouse_reporting_enabled: address.mouse_reporting_enabled,
                };

                let _ = self.terminal_tx.send(TerminalCommand::Connect(config));
                self.terminal_window.connect(Some(address.clone()));
                self.current_address = Some(address);
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }

            Message::Reconnect => {
                if let Some(address) = &self.last_address {
                    return self.update(Message::Connect(address.clone()));
                }
                Task::none()
            }
            Message::Hangup => {
                let _ = self.terminal_tx.send(TerminalCommand::Disconnect);
                self.terminal_window.disconnect();
                Task::none()
            }
            Message::SendData(data) => {
                self.clear_selection();
                let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
                Task::none()
            }

            Message::SendString(s) => {
                let mut screen = self.terminal_window.terminal.screen.lock();
                let _ = screen.clear_selection();
                let buffer_type = screen.buffer_type();

                drop(screen);
                let mut data: Vec<u8> = Vec::new();
                for ch in s.chars() {
                    let converted_byte = buffer_type.convert_from_unicode(ch);
                    // Skip NUL characters (used to indicate unmappable characters)
                    if converted_byte != '\0' {
                        data.push(converted_byte as u8);
                    }
                }
                let _ = self.terminal_tx.send(TerminalCommand::SendData(data));

                Task::none()
            }
            Message::RipCommand(clear_screen, cmd) => {
                let lock = &mut self.terminal_window.terminal.screen.lock();
                if clear_screen {
                    if let Some(editable) = lock.as_editable() {
                        editable.clear_screen();
                        editable.reset_terminal();
                    }
                }
                let buffer_type = lock.buffer_type();
                // Send the RIP command
                let mut data: Vec<u8> = Vec::new();
                for ch in cmd.chars() {
                    let converted_byte = buffer_type.convert_from_unicode(ch);
                    // Skip NUL characters (used to indicate unmappable characters)
                    if converted_byte != '\0' {
                        data.push(converted_byte as u8);
                    }
                }
                let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
                Task::none()
            }

            Message::TerminalEvent(event) => self.handle_terminal_event(event),
            Message::CaptureDialog(ref _msg) => {
                // Route to dialog stack
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::ShowIemsi(ref _msg) => {
                // Route to dialog stack
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::ShowIemsiDialog => {
                self.switch_to_terminal_screen();
                // Get IEMSI info from terminal if available
                if let Some(iemsi_info) = &self.terminal_window.iemsi_info {
                    self.dialogs.push(show_iemsi::show_iemsi_dialog_from_msg(
                        iemsi_info.clone(),
                        icy_engine_gui::dialog_msg!(Message::ShowIemsi),
                    ));
                }
                Task::none()
            }
            Message::SettingsDialog(ref _msg) => {
                // Route to dialog stack
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::ShowHelpDialog => {
                self.switch_to_terminal_screen();
                self.dialogs
                    .push(crate::ui::dialogs::help_dialog::help_dialog(Message::HelpDialog, |msg| match msg {
                        Message::HelpDialog(m) => Some(m),
                        _ => None,
                    }));
                Task::none()
            }
            Message::HelpDialog(_msg) => {
                // Dialog handles its own messages via DialogStack
                Task::none()
            }
            Message::ShowAboutDialog => {
                self.switch_to_terminal_screen();
                icy_engine_gui::set_default_auto_scaling_xy(true);
                self.dialogs.push(
                    crate::ui::dialogs::about_dialog::about_dialog(Message::AboutDialog, |msg| match msg {
                        Message::AboutDialog(m) => Some(m),
                        _ => None,
                    })
                    .on_cancel(|| {
                        icy_engine_gui::set_default_auto_scaling_xy(false);
                        Message::None
                    }),
                );
                Task::none()
            }
            Message::AboutDialog(ref msg) => {
                // Handle OpenLink messages from the about dialog
                if let crate::ui::dialogs::about_dialog::AboutDialogMessage::OpenLink(url) = msg {
                    if let Err(e) = open::that(url) {
                        log::error!("Failed to open URL {}: {}", url, e);
                    }
                }
                // Route to dialog stack for other messages
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::CloseDialog(mode) => {
                // Clear external transfer state when closing file transfer dialog
                self.file_transfer_dialog.clear_external_transfer();
                self.file_transfer_dialog.transfer_state = None;
                self.state.mode = *mode;
                Task::none()
            }
            Message::ShowDialingDirectory => {
                self.switch_to_terminal_screen();
                self.state.mode = MainWindowMode::ShowDialingDirectory;
                Task::none()
            }
            Message::Upload => {
                self.switch_to_terminal_screen();
                let protocols = self.options.lock().transfer_protocols.clone();
                self.dialogs.push(
                    protocol_selector::protocol_selector_dialog_from_msg(false, protocols, icy_engine_gui::dialog_msg!(Message::ProtocolSelector))
                        .on_confirm(|(protocol, is_download)| Message::InitiateFileTransfer { protocol, is_download }),
                );
                Task::none()
            }
            Message::Download => {
                self.switch_to_terminal_screen();
                let protocols = self.options.lock().transfer_protocols.clone();
                self.dialogs.push(
                    protocol_selector::protocol_selector_dialog_from_msg(true, protocols, icy_engine_gui::dialog_msg!(Message::ProtocolSelector))
                        .on_confirm(|(protocol, is_download)| Message::InitiateFileTransfer { protocol, is_download }),
                );
                Task::none()
            }
            Message::ProtocolSelector(ref _msg) => {
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::SendLoginAndPassword(login, pw) => {
                self.clear_selection();
                if self.is_connected {
                    if let Some(address) = &self.current_address {
                        if !address.user_name.is_empty() && login {
                            let username_data = address.user_name.as_bytes().to_vec();
                            let mut username_with_cr = username_data;
                            let enter_bytes = parse_key_string(self.terminal_emulation, "enter").unwrap_or(vec![b'\r']);
                            username_with_cr.extend(&enter_bytes);

                            let _ = self.terminal_tx.send(TerminalCommand::SendData(username_with_cr));

                            if pw && !address.password.is_empty() {
                                // Schedule password send after delay instead of blocking
                                let password = address.password.clone();
                                let tx = self.terminal_tx.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                    let mut password_with_cr = password.as_bytes().to_vec();
                                    password_with_cr.extend(enter_bytes);
                                    let _ = tx.send(TerminalCommand::SendData(password_with_cr));
                                });
                            }
                        } else if pw && !address.password.is_empty() {
                            let mut password_with_cr = address.password.as_bytes().to_vec();
                            let enter_bytes = parse_key_string(self.terminal_emulation, "enter").unwrap_or(vec![b'\r']);
                            password_with_cr.extend(enter_bytes);
                            let _ = self.terminal_tx.send(TerminalCommand::SendData(password_with_cr));
                        }
                    }
                }
                Task::none()
            }
            Message::TransferDialog(msg) => {
                if let Some(response) = self.file_transfer_dialog.update(msg) {
                    match response {
                        Message::CloseDialog(mode) => {
                            self.state.mode = *mode;
                        }
                        Message::CancelFileTransfer => {
                            // Send cancel command to terminal
                            let _ = self.terminal_tx.send(TerminalCommand::CancelTransfer);
                            self.state.mode = MainWindowMode::ShowTerminal;
                        }
                        _ => {}
                    }
                }
                Task::none()
            }
            Message::UpdateTransferState(state) => {
                self.file_transfer_dialog.update_transfer_state(state);
                Task::none()
            }
            Message::InitiateFileTransfer { protocol, is_download } => {
                self.initiate_file_transfer(protocol, is_download);
                Task::none()
            }
            Message::CancelFileTransfer => {
                let _ = self.terminal_tx.send(TerminalCommand::CancelTransfer);
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }
            Message::OpenReleaseLink => {
                let url = format!(
                    "https://github.com/mkrueger/icy_tools/releases/tag/IcyTerm{}",
                    crate::LATEST_VERSION.to_string()
                );
                if let Err(e) = webbrowser::open(&url) {
                    eprintln!("Failed to open release link: {}", e);
                }
                Task::none()
            }
            Message::ShowSettings => {
                self.switch_to_terminal_screen();

                // Create a fresh state for the dialog (temp_options is managed internally)
                let state = settings_dialog::SettingsDialogState::new(self.options.clone());

                self.dialogs.push(
                    settings_dialog::settings_dialog_from_msg(state, icy_engine_gui::dialog_msg!(Message::SettingsDialog)).on_save(|result| {
                        // Always refresh monitor settings cache after settings dialog saves
                        // The scrollback buffer size change will be handled separately via a batch
                        if let Some(size) = result.new_scrollback_size {
                            Message::SetScrollbackBufferSize(size)
                        } else {
                            Message::RefreshMonitorSettingsCache
                        }
                    }),
                );
                Task::none()
            }
            Message::ShowCaptureDialog => {
                self.switch_to_terminal_screen();
                // Update capture directory from options
                let capture_path = self.options.lock().capture_path();
                if !capture_path.is_empty() {
                    self.capture_directory = capture_path;
                }
                self.dialogs.push(
                    capture_dialog::capture_dialog_from_msg(
                        self.capture_directory.clone(),
                        self.is_capturing,
                        icy_engine_gui::dialog_msg!(Message::CaptureDialog),
                    )
                    .on_confirm(|result| match result {
                        capture_dialog::CaptureDialogResult::StartCapture(path) => Message::StartCapture(path),
                        capture_dialog::CaptureDialogResult::StopCapture => Message::StopCapture,
                    }),
                );
                Task::none()
            }
            Message::StartCapture(file_name) => {
                self.is_capturing = true;
                self.terminal_window.is_capturing = true;
                let _ = self.terminal_tx.send(TerminalCommand::StartCapture(file_name));
                Task::none()
            }
            Message::StopCapture => {
                self.is_capturing = false;
                self.terminal_window.is_capturing = false;
                let _ = self.terminal_tx.send(TerminalCommand::StopCapture);
                Task::none()
            }
            Message::ShowRunScriptDialog => {
                self.switch_to_terminal_screen();
                // Open file dialog to select a Lua script
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter(&i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "script-dialog-filter-lua"), &["lua"])
                            .add_filter(&i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "script-dialog-filter-all"), &["*"])
                            .set_title(&i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "script-dialog-title"))
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |result| {
                        if let Some(path) = result { Message::RunScript(path) } else { Message::None }
                    },
                );
            }
            Message::RunScript(path) => {
                log::info!("Running script: {}", path.display());
                let _ = self.terminal_tx.send(TerminalCommand::RunScript(path));
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }
            Message::StopScript => {
                let _ = self.terminal_tx.send(TerminalCommand::StopScript);
                Task::none()
            }
            Message::ApplyTerminalSettings {
                terminal_type,
                screen_mode,
                ansi_music,
            } => {
                let _ = self.terminal_tx.send(TerminalCommand::SetTerminalSettings {
                    terminal_type,
                    screen_mode,
                    ansi_music,
                });
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }
            Message::ShowFindDialog => {
                self.switch_to_terminal_screen();
                self.state.mode = MainWindowMode::ShowFindDialog;
                return self.find_dialog.focus_search_input();
            }
            Message::ShowBaudEmulationDialog => {
                self.switch_to_terminal_screen();
                let terminal_tx = self.terminal_tx.clone();
                self.dialogs.push(
                    select_bps_dialog::select_bps_dialog(self.terminal_window.baud_emulation, Message::SelectBpsDialog, |msg| match msg {
                        Message::SelectBpsDialog(m) => Some(m),
                        _ => None,
                    })
                    .on_confirm(move |baud| {
                        let _ = terminal_tx.send(TerminalCommand::SetBaudEmulation(baud));
                        Message::SelectBps(baud)
                    })
                    .on_cancel(|| Message::None),
                );
                Task::none()
            }
            Message::SelectBpsDialog(ref _msg) => {
                // Dialog handles its own messages via DialogStack
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::ShowOpenSerialDialog => {
                self.switch_to_terminal_screen();
                self.state.mode = MainWindowMode::ShowOpenSerialDialog(true);
                Task::none()
            }
            Message::OpenSerialMsg(msg) => {
                if let Some(message) = self.open_serial_dialog.update(msg) {
                    return self.update(message);
                }
                Task::none()
            }
            Message::ConnectSerial => {
                let serial = self.open_serial_dialog.serial.clone();
                // Save serial settings to options
                self.options.lock().serial = serial.clone();
                if let Err(e) = self.options.lock().store_options() {
                    log::error!("Failed to save serial settings: {}", e);
                }
                // Set serial mode for status bar
                self.terminal_window.serial_connected = Some(serial.clone());
                let _ = self.terminal_tx.send(TerminalCommand::OpenSerial(serial));
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }
            Message::AutoDetectSerial => {
                let serial = self.open_serial_dialog.serial.clone();
                let _ = self.terminal_tx.send(TerminalCommand::AutoDetectSerial(serial));
                // Hide dialog during auto-detection so terminal output is visible
                self.state.mode = MainWindowMode::ShowOpenSerialDialog(false);
                Task::none()
            }
            Message::FindDialog(msg) => {
                if let Some(close_msg) = self.find_dialog.update(msg, self.terminal_window.terminal.screen.clone()) {
                    return self.update(close_msg);
                }

                Task::none()
            }
            Message::ShowExportScreenDialog => {
                self.switch_to_terminal_screen();
                // Get the current buffer type from the terminal screen
                let buffer_type = self.terminal_window.terminal.screen.lock().buffer_type();
                // Re-initialize the export dialog with the current buffer type
                let default_export_path = directories::UserDirs::new()
                    .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
                    .join("export.icy");

                self.dialogs.push(
                    icy_engine_gui::ui::export_dialog_with_defaults_from_msg(
                        default_export_path.to_string_lossy().to_string(),
                        buffer_type,
                        self.terminal_window.terminal.screen.clone(),
                        crate::data::Options::default_capture_directory,
                        icy_engine_gui::dialog_msg!(Message::ExportDialog),
                    )
                    .on_cancel(|| Message::None),
                );
                Task::none()
            }
            Message::ExportDialog(ref _msg) => {
                // Export dialog messages are now handled by Dialog::update()
                // Route to dialog stack
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::StopSound => {
                self.sound_thread.lock().clear();
                Task::none()
            }

            Message::TerminalInfo(ref _msg) => {
                // Route to dialog stack
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::ShowTerminalInfoDialog => {
                self.switch_to_terminal_screen();
                // Gather terminal info
                let screen = self.terminal_window.terminal.screen.lock();
                let caret = screen.caret();
                let terminal_state = screen.terminal_state();

                // Get current terminal settings - use actual current state
                let terminal_type = self.terminal_window.terminal_emulation;
                let screen_mode = self.terminal_window.screen_mode;
                let ansi_music = self.terminal_window.ansi_music;

                let info = terminal_info_dialog::TerminalInfo {
                    buffer_size: terminal_state.size(),
                    screen_resolution: screen.resolution(),
                    font_size: screen.font(caret.font_page()).map(|f| f.size()).unwrap_or_default(),
                    caret_position: caret.position(),
                    caret_visible: caret.visible,
                    caret_blinking: caret.blinking,
                    caret_shape: caret.shape,
                    insert_mode: caret.insert_mode,
                    auto_wrap: terminal_state.auto_wrap_mode == icy_engine::AutoWrapMode::AutoWrap,
                    scroll_mode: terminal_state.scroll_state,
                    margins_top_bottom: terminal_state.margins_top_bottom(),
                    margins_left_right: terminal_state.margins_left_right(),
                    mouse_mode: format!("{:?}", terminal_state.mouse_state.mouse_mode),
                    inverse_mode: terminal_state.inverse_video,
                    ice_colors: screen.ice_mode() == icy_engine::IceMode::Ice,
                    baud_emulation: self.terminal_window.baud_emulation,
                    terminal_type,
                    screen_mode,
                    ansi_music,
                };
                drop(screen);

                self.dialogs.push(
                    terminal_info_dialog::terminal_info_dialog_from_msg(info, icy_engine_gui::dialog_msg!(Message::TerminalInfo)).on_confirm(|result| {
                        Message::ApplyTerminalSettings {
                            terminal_type: result.terminal_type,
                            screen_mode: result.screen_mode,
                            ansi_music: result.ansi_music,
                        }
                    }),
                );
                Task::none()
            }

            Message::None => Task::none(),

            Message::ToggleFullscreen => {
                self._is_fullscreen_mode = !self._is_fullscreen_mode;
                let mode = if self._is_fullscreen_mode {
                    iced::window::Mode::Fullscreen
                } else {
                    iced::window::Mode::Windowed
                };

                iced::window::latest().and_then(move |window| iced::window::set_mode(window, mode))
            }

            Message::OpenLink(url) => {
                if let Err(e) = webbrowser::open(&url) {
                    log::error!("Failed to open URL {}: {}", url, e);
                }
                Task::none()
            }

            Message::Copy => {
                {
                    let mut screen = self.terminal_window.terminal.screen.lock();
                    if let Err(err) = icy_engine_gui::copy_selection_to_clipboard(&mut **screen, &*crate::CLIPBOARD_CONTEXT) {
                        log::error!("Failed to copy: {err}");
                    }
                    self.shift_pressed_during_selection = false;
                }
                Task::none()
            }

            Message::Paste => {
                self.clear_selection();
                match crate::CLIPBOARD_CONTEXT.get_text() {
                    Ok(text) => {
                        let data = self.convert_clipboard_text(text);

                        // Send the data to the terminal
                        if !data.is_empty() {
                            let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to get clipboard text: {}", err);
                    }
                }
                Task::none()
            }

            Message::ShiftPressed(pressed) => {
                self.shift_pressed_during_selection = pressed;
                Task::none()
            }
            Message::SelectBps(bps) => {
                let _ = self.terminal_tx.send(TerminalCommand::SetBaudEmulation(bps));
                self.switch_to_terminal_screen();
                self.terminal_window.baud_emulation = bps;
                Task::none()
            }
            Message::QuitIcyTerm => {
                if self.is_connected {
                    let _ = self.terminal_tx.send(TerminalCommand::Disconnect);
                }

                // Stop any ongoing capture
                if self.terminal_window.is_capturing {
                    self.is_capturing = false;
                    self.terminal_window.is_capturing = false;
                    let _ = self.terminal_tx.send(TerminalCommand::StopCapture);
                }
                // Stop sound thread
                self.sound_thread.lock().clear();

                iced::exit()
            }
            Message::ClearScreen => {
                {
                    let mut edit_screen = self.terminal_window.terminal.screen.lock();
                    if let Some(editable) = edit_screen.as_editable() {
                        editable.clear_scrollback();
                        editable.clear_screen();
                    }
                }
                Task::none()
            }
            Message::ShowScrollback => {
                // Toggle scrollback mode
                if self.terminal_window.terminal.is_in_scrollback_mode() {
                    // Exit scrollback mode
                    self.terminal_window.terminal.exit_scrollback_mode();
                } else {
                    // Enter scrollback mode
                    let scrollback_opt = {
                        let mut screen = self.terminal_window.terminal.screen.lock();
                        if let Some(editable) = screen.as_editable() {
                            editable.snapshot_scrollback()
                        } else {
                            None
                        }
                    };

                    if let Some(scrollback) = scrollback_opt {
                        self.terminal_window.terminal.enter_scrollback_mode(scrollback);
                    }
                }
                Task::none()
            }
            Message::SetFocus(focus) => {
                self.terminal_window.set_focus(focus);
                Task::none()
            }
            Message::FocusNext => iced::widget::operation::focus_next(),
            Message::FocusPrevious => iced::widget::operation::focus_previous(),

            Message::SendMouseEvent(evt) => {
                let escape_sequence = evt.generate_mouse_report();
                // Send the escape sequence to the terminal if one was generated
                let buffer_type = self.terminal_window.terminal.screen.lock().buffer_type();
                if let Some(seq) = escape_sequence {
                    let mut data: Vec<u8> = Vec::new();
                    for ch in seq.chars() {
                        let converted_byte = buffer_type.convert_from_unicode(ch);
                        // Skip NUL characters (used to indicate unmappable characters)
                        if converted_byte != '\0' {
                            data.push(converted_byte as u8);
                        }
                    }
                    let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
                }
                Task::none()
            }

            Message::ScrollViewport(dx, dy) => {
                let mut vp = self.terminal_window.terminal.viewport.write();
                vp.scroll_x_by(dx);
                vp.scroll_y_by(dy);
                Task::none()
            }

            Message::ScrollViewportTo(smooth, x, y) => {
                // Scroll both axes
                let mut vp = self.terminal_window.terminal.viewport.write();
                if smooth {
                    vp.scroll_x_to_smooth(x);
                    vp.scroll_y_to_smooth(y);
                } else {
                    vp.scroll_x_to(x);
                    vp.scroll_y_to(y);
                }
                drop(vp);
                self.terminal_window.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }

            Message::ScrollViewportYToImmediate(y) => {
                // Vertical scrollbar: only change Y, keep X unchanged
                self.terminal_window.terminal.scroll_y_to(y);
                self.terminal_window.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }

            Message::ScrollViewportXToImmediate(x) => {
                // Horizontal scrollbar: only change X, keep Y unchanged
                self.terminal_window.terminal.scroll_x_to(x);
                self.terminal_window.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }

            Message::ViewportTick => {
                // Update viewport animation
                self.terminal_window.terminal.viewport.write().update_animation();
                Task::none()
            }

            Message::ScrollbarHovered(is_hovered) => {
                // Update vertical scrollbar hover state for animation
                self.terminal_window.terminal.scrollbar.set_hovered(is_hovered);
                Task::none()
            }

            Message::HScrollbarHovered(is_hovered) => {
                // Update horizontal scrollbar hover state for animation
                self.terminal_window.terminal.scrollbar.set_hovered_x(is_hovered);
                Task::none()
            }

            Message::CursorLeftWindow => {
                // Fade out scrollbars when cursor leaves window
                self.terminal_window.terminal.scrollbar.set_hovered(false);
                self.terminal_window.terminal.scrollbar.set_hovered_x(false);
                // Reset hover tracking state so next cursor move triggers hover update
                self.terminal_window
                    .terminal
                    .scrollbar_hover_state
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                self.terminal_window
                    .terminal
                    .hscrollbar_hover_state
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                Task::none()
            }

            Message::SetScrollbackBufferSize(buffer_size) => {
                {
                    let mut screen = self.terminal_window.terminal.screen.lock();
                    screen.set_scrollback_buffer_size(buffer_size);
                }
                // Also refresh monitor settings cache after settings dialog
                self.cached_monitor_settings = Arc::new(self.options.lock().monitor_settings.clone());
                Task::none()
            }

            Message::RefreshMonitorSettingsCache => {
                self.cached_monitor_settings = Arc::new(self.options.lock().monitor_settings.clone());
                Task::none()
            }

            Message::McpCommand(cmd) => {
                match cmd.as_ref() {
                    McpCommand::Connect(url) => {
                        // Parse and connect to the URL
                        match crate::ConnectionInformation::parse(url) {
                            Ok(address) => {
                                return self.update(Message::Connect(address.into()));
                            }
                            Err(e) => {
                                log::error!("Failed to parse URL {}: {}", url, e);
                            }
                        }
                    }
                    McpCommand::Disconnect => {
                        return self.update(Message::Hangup);
                    }
                    McpCommand::SendText(text) => {
                        return self.update(Message::SendString(text.clone()));
                    }
                    McpCommand::SendKey(key) => {
                        // Parse special keys and send appropriate bytes
                        let bytes = crate::scripting::parse_key_string(self.terminal_emulation, key);
                        if let Some(data) = bytes {
                            return self.update(Message::SendData(data));
                        }
                    }
                    McpCommand::CaptureScreen(format, response_tx) => {
                        // Capture the current screen in the requested format
                        let data = {
                            let mut screen = self.terminal_window.terminal.screen.lock();
                            let mut opt = icy_engine::SaveOptions::default();
                            opt.modern_terminal_output = true;
                            match format {
                                ScreenCaptureFormat::Text => screen.to_bytes("asc", &opt).unwrap_or_default(),
                                ScreenCaptureFormat::Ansi => screen.to_bytes("ans", &opt).unwrap_or_default(),
                            }
                        };

                        // Take the sender out of the Arc<Mutex> and send the response
                        {
                            let mut tx_guard = response_tx.lock();
                            if let Some(tx) = tx_guard.take() {
                                let _ = tx.send(data);
                            }
                        }
                    }
                    McpCommand::UploadFile { protocol, file_path } => {
                        // Start file upload - lookup protocol by id
                        let transfer_protocol = self
                            .options
                            .lock()
                            .transfer_protocols
                            .iter()
                            .find(|p| p.id == *protocol)
                            .cloned()
                            .or_else(|| crate::TransferProtocol::from_internal_id(protocol));

                        if let Some(transfer_protocol) = transfer_protocol {
                            let path = PathBuf::from(file_path);
                            if path.exists() {
                                let _ = self.terminal_tx.send(TerminalCommand::StartUpload(transfer_protocol, vec![path]));
                                self.state.mode = MainWindowMode::FileTransfer(false);
                            }
                        }
                    }
                    McpCommand::DownloadFile { protocol, save_path } => {
                        // Start file download - lookup protocol by id
                        let transfer_protocol = self
                            .options
                            .lock()
                            .transfer_protocols
                            .iter()
                            .find(|p| p.id == *protocol)
                            .cloned()
                            .or_else(|| crate::TransferProtocol::from_internal_id(protocol));

                        if let Some(transfer_protocol) = transfer_protocol {
                            let _ = self
                                .terminal_tx
                                .send(TerminalCommand::StartDownload(transfer_protocol, Some(save_path.clone())));
                            self.state.mode = MainWindowMode::FileTransfer(true);
                        }
                    }
                    McpCommand::RunMacro { name: _, commands } => {
                        // Execute macro commands sequentially
                        for command in commands {
                            // Parse and execute each command
                            // This could be sending text, keys, or other actions
                            let _ = self.update(Message::SendString(command.clone()));
                        }
                    }
                    McpCommand::SearchBuffer {
                        pattern,
                        case_sensitive,
                        regex: _,
                    } => {
                        // Set up search parameters and trigger search
                        // The find_dialog doesn't have search_text or use_regex fields, we need to handle this differently
                        // For now, just set case_sensitive
                        self.find_dialog.case_sensitive = *case_sensitive;
                        // We'll need to modify the find dialog to support setting search text programmatically
                        log::info!("Search requested for pattern: {}", pattern);
                        return self.update(Message::FindDialog(find_dialog::FindDialogMsg::FindNext));
                    }
                    McpCommand::ClearScreen => {
                        return self.update(Message::ClearScreen);
                    }

                    McpCommand::GetState(response_tx) => {
                        // Gather current terminal state
                        let state = {
                            let screen = self.terminal_window.terminal.screen.lock();
                            let cursor = screen.caret_position();
                            mcp::types::TerminalState {
                                cursor_position: (cursor.x as usize, cursor.y as usize),
                                screen_size: (screen.size().width as usize, screen.size().height as usize),
                                current_buffer: String::new(),
                                is_connected: self.is_connected,
                                current_bbs: self.current_address.as_ref().map(|addr| addr.system_name.clone()),
                            }
                        };

                        // Take the sender out of the Arc<Mutex> and send the response
                        {
                            let mut tx_guard = response_tx.lock();
                            if let Some(tx) = tx_guard.take() {
                                let _ = tx.send(state);
                            }
                        }
                    }

                    McpCommand::ListAddresses(response_tx) => {
                        // Get addresses from the address book
                        let addresses = {
                            let book = self.dialing_directory.addresses.lock();
                            book.addresses.clone()
                        };

                        // Take the sender out of the Arc<Mutex> and send the response
                        {
                            let mut tx_guard = response_tx.lock();
                            if let Some(tx) = tx_guard.take() {
                                let _ = tx.send(addresses);
                            }
                        }
                    }

                    McpCommand::RunScript(script, response_tx) => {
                        // Store the response channel to send result when script finishes
                        self.pending_script_response = response_tx.clone();
                        // Run the Lua script code directly
                        let _ = self.terminal_tx.send(TerminalCommand::RunScriptCode(script.clone()));
                    }
                }
                Task::none()
            }

            Message::MousePress(evt) => self.handle_mouse_press(evt),
            Message::MouseRelease(evt) => self.handle_mouse_release(evt),
            Message::MouseMove(evt) => self.handle_mouse_move(evt),
            Message::MouseDrag(evt) => self.handle_mouse_drag(evt),
            Message::MouseScroll(delta) => self.handle_mouse_scroll(delta),

            Message::StartSelection(sel) => {
                {
                    let mut screen = self.terminal_window.terminal.screen.lock();
                    let _ = screen.set_selection(sel);
                }
                Task::none()
            }

            Message::UpdateSelection(pos) => {
                {
                    let mut screen = self.terminal_window.terminal.screen.lock();
                    if let Some(mut sel) = screen.selection().clone() {
                        if !sel.locked {
                            sel.lead = pos;
                            let _ = screen.set_selection(sel);
                        }
                    }
                }
                Task::none()
            }

            Message::EndSelection => {
                {
                    let mut screen = self.terminal_window.terminal.screen.lock();
                    if let Some(mut sel) = screen.selection().clone() {
                        sel.locked = true;
                        let _ = screen.set_selection(sel);
                    }
                }
                Task::none()
            }

            Message::ClearSelection => {
                {
                    let mut screen = self.terminal_window.terminal.screen.lock();
                    let _ = screen.clear_selection();
                }
                Task::none()
            }

            // Unified zoom control
            Message::Zoom(zoom_msg) => {
                let mut opts = self.options.lock();
                let use_integer = opts.monitor_settings.use_integer_scaling;
                let current_zoom = self.terminal_window.terminal.get_zoom();
                let new_scaling = opts.monitor_settings.scaling_mode.apply_zoom(zoom_msg, current_zoom, use_integer);
                if let icy_engine_gui::ScalingMode::Manual(z) = new_scaling {
                    self.terminal_window.terminal.set_zoom(z);
                }
                opts.monitor_settings.scaling_mode = new_scaling;
                // Update cached monitor settings
                self.cached_monitor_settings = Arc::new(opts.monitor_settings.clone());
                Task::none()
            }
        }
    }

    fn convert_clipboard_text(&mut self, text: String) -> Vec<u8> {
        let buffer_type = self.terminal_window.terminal.screen.lock().buffer_type();
        let enter_bytes = parse_key_string(self.terminal_emulation, "enter").unwrap_or(vec![b'\r']);

        let mut result = Vec::new();
        // Normalize line endings: replace \r\n and \n with terminal-specific enter
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\r' {
                // Check if this is \r\n (Windows EOL)
                if chars.peek() == Some(&'\n') {
                    chars.next(); // consume the \n
                }
                // Add terminal-specific enter sequence
                result.extend(&enter_bytes);
            } else if ch == '\n' {
                // Unix EOL
                result.extend(&enter_bytes);
            } else {
                let converted_byte = buffer_type.convert_from_unicode(ch);
                // Skip NUL characters (used to indicate unmappable characters)
                if converted_byte != '\0' {
                    result.push(converted_byte as u8);
                }
            }
        }
        result
    }

    fn initiate_file_transfer(&mut self, protocol: crate::TransferProtocol, is_download: bool) {
        if is_download {
            // Set download directory from options
            let download_path = self.options.lock().download_path();

            // If protocol requires asking for download location, show file dialog
            if protocol.ask_for_download_location {
                let file = rfd::FileDialog::new()
                    .set_title(&i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "file-dialog-save-download-as"))
                    .set_directory(&download_path)
                    .set_file_name("download.bin")
                    .save_file();

                if let Some(save_path) = file {
                    // Use the parent directory as download directory
                    if let Some(parent) = save_path.parent() {
                        let _ = self.terminal_tx.send(TerminalCommand::SetDownloadDirectory(parent.to_path_buf()));
                    }
                    // Use the filename for the download
                    let filename = save_path.file_name().and_then(|n| n.to_str()).map(|s| s.to_string());
                    let _ = self.terminal_tx.send(TerminalCommand::StartDownload(protocol, filename));
                    self.state.mode = MainWindowMode::FileTransfer(is_download);
                } else {
                    // User cancelled - don't start download
                    self.state.mode = MainWindowMode::ShowTerminal;
                }
            } else {
                let _ = self.terminal_tx.send(TerminalCommand::SetDownloadDirectory(PathBuf::from(download_path)));
                let _ = self.terminal_tx.send(TerminalCommand::StartDownload(protocol, None));
                self.state.mode = MainWindowMode::FileTransfer(is_download);
            }
        } else {
            let files = rfd::FileDialog::new()
                .set_title("Select Files to Upload")
                .set_directory(self.initial_upload_directory.as_ref().and_then(|p| p.to_str()).unwrap_or("."))
                .pick_files();

            if let Some(files) = files {
                let _ = self.terminal_tx.send(TerminalCommand::StartUpload(protocol, files));
                self.state.mode = MainWindowMode::FileTransfer(is_download);
            } else {
                self.state.mode = MainWindowMode::ShowTerminal;
            }
        }
    }

    fn handle_terminal_event(&mut self, event: TerminalEvent) -> Task<Message> {
        match event {
            TerminalEvent::Connected => {
                self.is_connected = true;
                self.last_address = self.current_address.clone();
                self.terminal_window.is_connected = true;
                self.connection_time = Some(Instant::now());
                self.show_disconnect = false;
                Task::none()
            }
            TerminalEvent::Disconnected(_error) => {
                self.is_connected = false;
                self.terminal_window.disconnect();
                self.connection_time = None;
                Task::none()
            }
            TerminalEvent::Reconnect => {
                return self.update(Message::Reconnect);
            }
            TerminalEvent::Connect(name_or_url) => {
                let addresses = self.dialing_directory.addresses.lock();
                if let Some(address) = addresses
                    .addresses
                    .iter()
                    .find(|addr| addr.system_name.eq_ignore_ascii_case(&name_or_url) || addr.address.eq_ignore_ascii_case(&name_or_url))
                {
                    let address = address.clone();
                    drop(addresses);
                    return self.update(Message::Connect(address));
                }
                drop(addresses);

                // First try to parse as URL
                if let Ok(e) = crate::ConnectionInformation::parse(&name_or_url) {
                    return self.update(Message::Connect(e.into()));
                }

                // Not found - log error
                log::warn!("Script connect: '{}' not found in address book and not a valid URL", name_or_url);
                Task::none()
            }
            TerminalEvent::SendCredentials(mode) => {
                // Send credentials from current_address
                // Mode: 0 = username + password, 1 = username only, 2 = password only
                let send_login = mode == 0 || mode == 1;
                let send_password = mode == 0 || mode == 2;
                return self.update(Message::SendLoginAndPassword(send_login, send_password));
            }

            TerminalEvent::TransferStarted(_state, is_download) => {
                self.state.mode = MainWindowMode::FileTransfer(is_download);
                self.file_transfer_dialog.transfer_state = Some(_state);
                Task::none()
            }
            TerminalEvent::TransferProgress(_state) => {
                self.file_transfer_dialog.transfer_state = Some(_state);
                Task::none()
            }
            TerminalEvent::TransferCompleted(_state) => {
                self.file_transfer_dialog.transfer_state = Some(_state);
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }
            TerminalEvent::ExternalTransferStarted(protocol_name, is_download) => {
                self.file_transfer_dialog.set_external_transfer(protocol_name, is_download);
                self.state.mode = MainWindowMode::FileTransfer(is_download);
                Task::none()
            }
            TerminalEvent::ExternalTransferCompleted(_protocol_name, _is_download, success, error_message) => {
                self.file_transfer_dialog.complete_external_transfer(success, error_message);
                // Keep the dialog open so user can see the result
                Task::none()
            }
            TerminalEvent::Error(error, txt) => {
                let mut dialog = error_dialog("Terminal Error", txt, |_| Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)));
                dialog.dialog = dialog.dialog.secondary_message(error);
                self.dialogs.push(dialog);
                Task::none()
            }
            TerminalEvent::PlayMusic(music) => {
                let r = self.sound_thread.lock().play_music(music);
                if let Err(r) = r {
                    log::error!("TerminalEvent::PlayMusic: {r}");
                }
                Task::none()
            }
            TerminalEvent::PlayGist(sound_data) => {
                let r = self.sound_thread.lock().play_gist(sound_data);
                if let Err(r) = r {
                    log::error!("TerminalEvent::PlayGist: {r}");
                }
                Task::none()
            }
            TerminalEvent::PlayChipMusic {
                sound_data,
                voice,
                volume,
                pitch,
            } => {
                let r = self.sound_thread.lock().play_chip_music(sound_data, voice, volume, pitch);
                if let Err(r) = r {
                    log::error!("TerminalEvent::PlayChipMusic: {r}");
                }
                Task::none()
            }
            TerminalEvent::SndOff(voice) => {
                let r = self.sound_thread.lock().snd_off(voice);
                if let Err(r) = r {
                    log::error!("TerminalEvent::SndOff: {r}");
                }
                Task::none()
            }
            TerminalEvent::StopSnd(voice) => {
                let r = self.sound_thread.lock().stop_snd(voice);
                if let Err(r) = r {
                    log::error!("TerminalEvent::StopSnd: {r}");
                }
                Task::none()
            }
            TerminalEvent::SndOffAll => {
                let r = self.sound_thread.lock().snd_off_all();
                if let Err(r) = r {
                    log::error!("TerminalEvent::SndOffAll: {r}");
                }
                Task::none()
            }
            TerminalEvent::StopSndAll => {
                let r = self.sound_thread.lock().stop_snd_all();
                if let Err(r) = r {
                    log::error!("TerminalEvent::StopSndAll: {r}");
                }
                Task::none()
            }
            TerminalEvent::InformDelay(ms) => {
                self.pause_message = Some(format!("Pause {}ms", ms));
                Task::none()
            }
            TerminalEvent::ContinueAfterDelay => {
                self.pause_message = None;
                Task::none()
            }
            TerminalEvent::Beep => {
                let r = self.sound_thread.lock().beep();
                if let Err(r) = r {
                    log::error!("TerminalEvent::Beep: {r}");
                }
                Task::none()
            }
            TerminalEvent::OpenLineSound => {
                let dial_tone = self.options.lock().dial_tone;
                let r = self.sound_thread.lock().start_line_sound(dial_tone);
                if let Err(r) = r {
                    log::error!("TerminalEvent::OpenLineSound: {r}");
                }
                Task::none()
            }

            TerminalEvent::OpenDialSound(tone_dial, phone_number) => {
                let dial_tone = self.options.lock().dial_tone;
                let r = self.sound_thread.lock().start_dial_sound(tone_dial, dial_tone, &phone_number);
                if let Err(r) = r {
                    log::error!("TerminalEvent::OpenDialSound: {r}");
                }
                Task::none()
            }

            TerminalEvent::StopSound => {
                let r = self.sound_thread.lock().stop_line_sound();
                if let Err(r) = r {
                    log::error!("TerminalEvent::StopSound: {r}");
                }
                Task::none()
            }
            TerminalEvent::AutoTransferTriggered(protocol_id, is_download, _) => {
                // First check user-configured protocols
                let protocol = self
                    .options
                    .lock()
                    .transfer_protocols
                    .iter()
                    .find(|p| p.id == protocol_id)
                    .cloned()
                    .or_else(|| crate::TransferProtocol::from_internal_id(&protocol_id));

                if let Some(protocol) = protocol {
                    self.initiate_file_transfer(protocol, is_download);
                } else {
                    log::error!("Unknown protocol id for auto-transfer: {}", protocol_id);
                }
                Task::none()
            }
            TerminalEvent::EmsiLogin(isi) => {
                self.terminal_window.iemsi_info = Some(*isi);
                Task::none()
            }
            TerminalEvent::ScriptStarted(path) => {
                log::info!("Script started: {}", path.display());
                Task::none()
            }
            TerminalEvent::ScriptFinished(result) => {
                // Send result to MCP if there's a pending response channel
                if let Some(response_tx) = self.pending_script_response.take() {
                    if let Some(tx) = response_tx.lock().take() {
                        let mcp_result = match &result {
                            Ok(()) => Ok(String::new()),
                            Err(e) => Err(e.clone()),
                        };
                        let _ = tx.send(mcp_result);
                    }
                }

                match result {
                    Ok(()) => {
                        log::info!("Script finished successfully");
                    }
                    Err(e) => {
                        log::error!("Script error: {}", e);
                        let mut dialog = error_dialog(i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-script-title"), e, |_| {
                            Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))
                        });
                        dialog.dialog = dialog
                            .dialog
                            .secondary_message(i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-script-execution-failed"));
                        self.dialogs.push(dialog);
                    }
                }
                Task::none()
            }
            TerminalEvent::Quit => {
                return self.update(Message::QuitIcyTerm);
            }
            TerminalEvent::SerialBaudDetected(baud_rate) => {
                // Update the open serial dialog with detected baud rate
                self.open_serial_dialog.serial.baud_rate = baud_rate;
                Task::none()
            }
            TerminalEvent::SerialAutoDetectComplete => {
                // Show dialog again after auto-detection completes
                self.state.mode = MainWindowMode::ShowOpenSerialDialog(true);
                Task::none()
            }
            TerminalEvent::TerminalSettingsChanged {
                terminal_type,
                screen_mode,
                ansi_music,
            } => {
                // Update the terminal window's settings
                self.terminal_window.terminal_emulation = terminal_type;
                self.terminal_window.screen_mode = screen_mode;
                self.terminal_window.ansi_music = ansi_music;
                Task::none()
            }
        }
    }

    pub fn get_mcp_commands(&mut self) -> Vec<McpCommand> {
        let mut mcp_commands = Vec::new();
        if let Some(rx) = &mut self.mcp_rx {
            while let Ok(cmd) = rx.try_recv() {
                mcp_commands.push(cmd);
            }
        }
        mcp_commands
    }

    pub fn get_terminal_commands(&mut self) -> Vec<TerminalEvent> {
        let mut events = Vec::new();
        if let Some(rx) = &mut self.terminal_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        events
    }

    pub fn theme(&self) -> Theme {
        // Check if dialog stack has a dialog with custom theme (e.g., settings dialog with live preview)
        if let Some(theme) = self.dialogs.theme() {
            return theme;
        }
        self.options.lock().monitor_settings.get_theme()
    }

    /// Get a string representing the current zoom level for display in title bar
    pub fn get_zoom_info_string(&self) -> String {
        let opts = self.options.lock();
        opts.monitor_settings.scaling_mode.format_zoom_string()
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.state.mode {
            MainWindowMode::ShowDialingDirectory => return self.dialogs.view(self.dialing_directory.view(&self.options.lock())),
            _ => {}
        }

        let terminal_view = self
            .terminal_window
            .view(self.cached_monitor_settings.clone(), &self.options.lock(), &self.pause_message);

        let mode_view = match &self.state.mode {
            MainWindowMode::ShowTerminal => terminal_view,
            MainWindowMode::FileTransfer(download) => self.file_transfer_dialog.view(*download, terminal_view),
            MainWindowMode::ShowFindDialog => find_dialog::find_dialog_overlay(&self.find_dialog, terminal_view),
            MainWindowMode::ShowOpenSerialDialog(visible) => {
                if *visible {
                    self.open_serial_dialog.view(terminal_view)
                } else {
                    terminal_view
                }
            }
            _ => {
                panic!("Unhandled main window mode in view()")
            }
        };

        // Wrap with dialog stack (for new trait-based dialogs)
        self.dialogs.view(mode_view)
    }

    pub fn get_mode(&self) -> MainWindowMode {
        self.state.mode.clone()
    }

    fn map_key_event_to_bytes(
        terminal_type: TerminalEmulation,
        key: &keyboard::Key,
        physical: &keyboard::key::Physical,
        modifiers: keyboard::Modifiers,
    ) -> Option<Vec<u8>> {
        let key_map = match terminal_type {
            icy_net::telnet::TerminalEmulation::PETscii => icy_engine_gui::key_map::C64_KEY_MAP,
            icy_net::telnet::TerminalEmulation::ViewData => icy_engine_gui::key_map::VIDEOTERM_KEY_MAP,
            icy_net::telnet::TerminalEmulation::Mode7 => icy_engine_gui::key_map::MODE7_KEY_MAP,
            icy_net::telnet::TerminalEmulation::ATAscii => icy_engine_gui::key_map::ATASCII_KEY_MAP,
            icy_net::telnet::TerminalEmulation::AtariST => icy_engine_gui::key_map::ATARI_ST_KEY_MAP,
            _ => icy_engine_gui::key_map::ANSI_KEY_MAP,
        };

        // Use the lookup_key function from the key_map module
        icy_engine_gui::key_map::lookup_key(key, physical, modifiers, key_map)
    }

    fn clear_selection(&mut self) {
        let mut edit_screen = self.terminal_window.terminal.screen.lock();
        let _ = edit_screen.clear_selection();
        self.shift_pressed_during_selection = false;
    }

    /// Handle mouse move (no button pressed)
    fn handle_mouse_move(&mut self, evt: icy_engine_gui::TerminalMouseEvent) -> Task<Message> {
        use iced::mouse;
        use icy_engine::{MouseButton, MouseEvent, MouseEventType};

        let screen = self.terminal_window.terminal.screen.lock();
        let mouse_state = screen.terminal_state().mouse_state.clone();

        // Determine cursor based on what's under it
        let new_cursor = if let Some(cell) = evt.text_position {
            // Check hyperlinks first
            let mut cursor = None;
            for hyperlink in screen.hyperlinks() {
                if screen.is_position_in_range(cell, hyperlink.position, hyperlink.length) {
                    cursor = Some(mouse::Interaction::Pointer);
                    break;
                }
            }
            // Check RIP fields if no hyperlink found
            if cursor.is_none() {
                for mouse_field in screen.mouse_fields() {
                    if mouse_field.style.is_mouse_button() && mouse_field.contains(cell.x, cell.y) {
                        cursor = Some(mouse::Interaction::Pointer);
                        break;
                    }
                }
            }
            // Default to text cursor for terminal
            cursor.unwrap_or(mouse::Interaction::Text)
        } else {
            mouse::Interaction::default()
        };
        drop(screen);

        // Update terminal cursor icon
        *self.terminal_window.terminal.cursor_icon.write() = Some(new_cursor);

        if mouse_state.tracking_enabled() {
            if let Some(cell) = evt.text_position {
                if matches!(mouse_state.mouse_mode, icy_engine::MouseMode::AnyEvents) {
                    let mouse_event = MouseEvent {
                        mouse_state,
                        event_type: MouseEventType::Motion,
                        position: cell,
                        button: MouseButton::None,
                        modifiers: evt.modifiers,
                    };
                    return self.update(Message::SendMouseEvent(mouse_event));
                }
            }
        }
        Task::none()
    }

    /// Handle mouse drag (button held while moving)
    fn handle_mouse_drag(&mut self, evt: icy_engine_gui::TerminalMouseEvent) -> Task<Message> {
        use iced::mouse;
        use icy_engine::{MouseButton, MouseEvent, MouseEventType};

        // Set crosshair cursor during drag/selection
        *self.terminal_window.terminal.cursor_icon.write() = Some(mouse::Interaction::Crosshair);

        let screen = self.terminal_window.terminal.screen.lock();
        let mouse_state = screen.terminal_state().mouse_state.clone();
        let mouse_tracking_enabled = mouse_state.tracking_enabled();
        drop(screen);

        if mouse_tracking_enabled {
            if let Some(cell) = evt.text_position {
                let should_report = matches!(mouse_state.mouse_mode, icy_engine::MouseMode::ButtonEvents | icy_engine::MouseMode::AnyEvents);
                if should_report {
                    let mouse_event = MouseEvent {
                        mouse_state,
                        event_type: MouseEventType::Motion,
                        position: cell,
                        button: MouseButton::Left,
                        modifiers: evt.modifiers,
                    };
                    return self.update(Message::SendMouseEvent(mouse_event));
                }
            }
        } else {
            // Update selection during drag
            if let Some(cell) = evt.text_position {
                return self.update(Message::UpdateSelection(cell));
            }
        }
        Task::none()
    }

    /// Handle mouse button press
    fn handle_mouse_press(&mut self, evt: icy_engine_gui::TerminalMouseEvent) -> Task<Message> {
        use icy_engine::{MouseButton, MouseEvent, MouseEventType, Selection, Shape};

        // First, gather info from the screen (with lock held), then release and act
        let mut rip_command: Option<(bool, String)> = None;
        let mut hyperlink_url: Option<String> = None;
        let mut has_selection = false;
        let mouse_state;
        let mouse_tracking_enabled;

        {
            let screen = self.terminal_window.terminal.screen.lock();
            mouse_state = screen.terminal_state().mouse_state.clone();
            mouse_tracking_enabled = mouse_state.tracking_enabled();

            if let Some(_cell) = evt.text_position {
                // Check RIP fields
                rip_command = evt.get_rip_field(&**screen);

                // Check hyperlinks (only if no RIP command and left-click)
                if rip_command.is_none() && matches!(evt.button, MouseButton::Left) {
                    hyperlink_url = evt.get_hyperlink(&**screen);
                }

                has_selection = screen.selection().is_some();
            }
        }

        // Handle RIP commands
        if let Some((clear_screen, cmd)) = rip_command {
            return self.update(Message::RipCommand(clear_screen, cmd));
        }

        // Handle hyperlinks
        if let Some(url) = hyperlink_url {
            return self.update(Message::OpenLink(url));
        }

        // Handle mouse tracking
        if mouse_tracking_enabled {
            if let Some(cell) = evt.text_position {
                let mouse_event = MouseEvent {
                    mouse_state,
                    event_type: MouseEventType::Press,
                    position: cell,
                    button: evt.button,
                    modifiers: evt.modifiers,
                };
                return self.update(Message::SendMouseEvent(mouse_event));
            }
        } else {
            // Handle selection
            match evt.button {
                MouseButton::Left => {
                    if let Some(cell) = evt.text_position {
                        if !evt.modifiers.shift {
                            let mut sel = Selection::new(cell);
                            sel.shape = if evt.modifiers.alt { Shape::Rectangle } else { Shape::Lines };
                            sel.locked = false;
                            return self.update(Message::StartSelection(sel));
                        }
                    } else {
                        // Clicked outside terminal area
                        return self.update(Message::ClearSelection);
                    }
                }
                MouseButton::Middle => {
                    if has_selection {
                        return self.update(Message::Copy);
                    } else {
                        return self.update(Message::Paste);
                    }
                }
                _ => {}
            }
        }
        Task::none()
    }

    /// Handle mouse button release  
    fn handle_mouse_release(&mut self, evt: icy_engine_gui::TerminalMouseEvent) -> Task<Message> {
        use icy_engine::{MouseButton, MouseEvent, MouseEventType};

        let screen = self.terminal_window.terminal.screen.lock();
        let mouse_state = screen.terminal_state().mouse_state.clone();
        let mouse_tracking_enabled = mouse_state.tracking_enabled();
        drop(screen);

        if mouse_tracking_enabled {
            if let Some(cell) = evt.text_position {
                let mouse_event = MouseEvent {
                    mouse_state,
                    event_type: MouseEventType::Release,
                    position: cell,
                    button: evt.button,
                    modifiers: evt.modifiers,
                };
                return self.update(Message::SendMouseEvent(mouse_event));
            }
        } else if matches!(evt.button, MouseButton::Left) {
            // End selection
            self.shift_pressed_during_selection = evt.modifiers.shift;
            return self.update(Message::EndSelection);
        }
        Task::none()
    }

    /// Handle mouse scroll
    fn handle_mouse_scroll(&mut self, delta: icy_engine_gui::WheelDelta) -> Task<Message> {
        use icy_engine::{MouseButton, MouseEvent, MouseEventType};

        let (scroll_x, scroll_y) = match delta {
            icy_engine_gui::WheelDelta::Lines { x, y } => (x, y),
            icy_engine_gui::WheelDelta::Pixels { x, y } => (x / 20.0, y / 20.0),
        };

        let screen = self.terminal_window.terminal.screen.lock();
        let mouse_state = screen.terminal_state().mouse_state.clone();
        let mouse_tracking_enabled = mouse_state.tracking_enabled();
        // For wheel events, we don't have text position in the delta, use center
        // TODO: We might want to pass modifiers and position through the Scroll message
        drop(screen);

        // Note: We don't have modifiers in WheelDelta, so we can't check for Ctrl+Scroll zoom here
        // That's handled by the Zoom message separately

        if mouse_tracking_enabled {
            if scroll_y != 0.0 {
                let button = if scroll_y > 0.0 { MouseButton::WheelUp } else { MouseButton::WheelDown };
                let mouse_event = MouseEvent {
                    mouse_state,
                    event_type: MouseEventType::Press,
                    position: icy_engine::Position::default(),
                    button,
                    modifiers: Default::default(),
                };
                return self.update(Message::SendMouseEvent(mouse_event));
            }
        } else {
            // Viewport-based scrolling
            let scroll_px_x = -scroll_x * 10.0;
            let scroll_px_y = -scroll_y * 20.0;
            return self.update(Message::ScrollViewport(scroll_px_x, scroll_px_y));
        }
        Task::none()
    }

    fn switch_to_terminal_screen(&mut self) {
        self.state.mode = MainWindowMode::ShowTerminal;
    }

    pub fn handle_event(&mut self, event: &Event) -> (Option<Message>, Task<Message>) {
        // First, let the dialog stack handle events if it has dialogs
        if !self.dialogs.is_empty() {
            let task = self.dialogs.handle_event(event);
            // If the stack has dialogs, consume the event
            return (None, task);
        }

        match event {
            Event::Window(window::Event::Focused) => {
                return (Some(Message::SetFocus(true)), Task::none());
            }
            Event::Window(window::Event::Unfocused) => {
                return (Some(Message::SetFocus(false)), Task::none());
            }
            Event::Mouse(iced::mouse::Event::CursorLeft) => {
                return (Some(Message::CursorLeftWindow), Task::none());
            }
            Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                // Only handle mouse wheel in scrollback mode
                if self.terminal_window.terminal.is_in_scrollback_mode() {
                    let line_height = self.terminal_window.terminal.char_height;
                    let scroll_amount = match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. } => {
                            // Each line of scroll = one character line
                            -y * line_height
                        }
                        iced::mouse::ScrollDelta::Pixels { y, .. } => {
                            // Direct pixel scrolling
                            -y
                        }
                    };
                    return (Some(Message::ScrollViewport(0.0, scroll_amount)), Task::none());
                }
            }

            _ => {}
        }

        let msg = match &self.state.mode {
            MainWindowMode::ShowDialingDirectory => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::NavigateUp))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::NavigateDown))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::ConnectSelected))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::Close)),
                    _ => self.dialing_directory.handle_event(event),
                },
                _ => None,
            },
            MainWindowMode::ShowTerminal => {
                match event {
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        key,
                        modifiers,
                        text,
                        physical_key,
                        ..
                    }) => {
                        // Handle scrollback mode navigation (context-specific, not in command handler)
                        if self.terminal_window.terminal.is_in_scrollback_mode() {
                            // Get font height for line-based scrolling
                            let line_height = self.terminal_window.terminal.char_height;
                            let page_height = self.terminal_window.terminal.viewport.read().visible_height;

                            match key {
                                // ESC exits scrollback mode
                                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                                    return (Some(Message::ShowScrollback), Task::none());
                                }
                                // Arrow Up: scroll up one line
                                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                                    return (Some(Message::ScrollViewport(0.0, -line_height)), Task::none());
                                }
                                // Arrow Down: scroll down one line
                                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                                    return (Some(Message::ScrollViewport(0.0, line_height)), Task::none());
                                }
                                // Page Up: scroll up one screen
                                keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                                    return (Some(Message::ScrollViewport(0.0, -page_height)), Task::none());
                                }
                                // Page Down: scroll down one screen
                                keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                                    return (Some(Message::ScrollViewport(0.0, page_height)), Task::none());
                                }
                                // Home: scroll to top
                                keyboard::Key::Named(keyboard::key::Named::Home) => {
                                    return (Some(Message::ScrollViewportTo(true, 0.0, 0.0)), Task::none());
                                }
                                // End: scroll to bottom
                                keyboard::Key::Named(keyboard::key::Named::End) => {
                                    let max_y = self.terminal_window.terminal.viewport.read().max_scroll_y();
                                    return (Some(Message::ScrollViewportTo(true, 0.0, max_y)), Task::none());
                                }
                                // Any other key exits scrollback mode
                                _ => {
                                    return (Some(Message::ShowScrollback), Task::none());
                                }
                            }
                        }

                        // Try command handler for keyboard shortcuts
                        if let Some(msg) = self.commands.handle(event) {
                            return (Some(msg), Task::none());
                        }

                        // Try to map the key with modifiers using the key map (for terminal input)
                        if let Some(bytes) = Self::map_key_event_to_bytes(self.terminal_emulation, key, physical_key, *modifiers) {
                            return (Some(Message::SendData(bytes)), Task::none());
                        }

                        if let Some(text) = text {
                            Some(Message::SendString(text.to_string()))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            MainWindowMode::ShowFindDialog => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::FindDialog(find_dialog::FindDialogMsg::CloseDialog)),
                    keyboard::Key::Named(keyboard::key::Named::PageUp) => Some(Message::FindDialog(find_dialog::FindDialogMsg::FindPrev)),
                    keyboard::Key::Named(keyboard::key::Named::PageDown) => Some(Message::FindDialog(find_dialog::FindDialogMsg::FindNext)),
                    keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::FindDialog(find_dialog::FindDialogMsg::FindNext)),
                    _ => None,
                },
                _ => None,
            },
            MainWindowMode::ShowOpenSerialDialog(visible) => {
                if *visible {
                    match event {
                        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
                            keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::ConnectSerial),
                            _ => None,
                        },
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => {
                // Handle global shortcuts that work in any mode
                match event {
                    // Try command handler for global shortcuts
                    Event::Keyboard(keyboard::Event::KeyPressed { .. }) => self.commands.handle(event),
                    Event::Keyboard(keyboard::Event::KeyReleased {
                        key: keyboard::Key::Named(keyboard::key::Named::Shift),
                        ..
                    }) => Some(Message::ShiftPressed(false)),
                    _ => None,
                }
            }
        };
        (msg, Task::none())
    }
}

// Implement the Window trait for use with shared WindowManager helpers
impl icy_engine_gui::Window for MainWindow {
    type Message = Message;

    fn id(&self) -> usize {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn get_zoom_info_string(&self) -> String {
        let opts = self.options.lock();
        opts.monitor_settings.scaling_mode.format_zoom_string()
    }

    fn update(&mut self, msg: Self::Message) -> Task<Self::Message> {
        self.update(msg)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.view()
    }

    fn theme(&self) -> Theme {
        self.theme()
    }

    fn handle_event(&mut self, event: &iced::Event) -> (Option<Self::Message>, Task<Self::Message>) {
        self.handle_event(event)
    }

    fn needs_animation(&self) -> bool {
        false
    }
}
