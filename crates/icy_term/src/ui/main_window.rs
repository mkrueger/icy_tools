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
    mcp::{self, McpCommand, types::ScreenCaptureFormat},
    scripting::parse_key_string,
    ui::{
        Message,
        dialogs::find_dialog,
        export_screen_dialog,
        up_download_dialog::{self, FileTransferDialogState},
    },
    util::SoundThread,
};

use clipboard_rs::{Clipboard, ClipboardContent, common::RustImage};
use iced::{Element, Event, Task, Theme, keyboard, window};
use icy_engine::{Position, RenderOptions, clipboard::ICY_CLIPBOARD_TYPE};
use icy_engine_gui::{ButtonSet, ConfirmationDialog, DialogType};
use icy_net::{ConnectionType, telnet::TerminalEmulation};
use icy_parser_core::BaudEmulation;
use image::DynamicImage;
use tokio::sync::mpsc;

use crate::{
    Address, AddressBook, Options,
    terminal::terminal_thread::{ConnectionConfig, TerminalCommand, TerminalEvent, create_terminal_thread},
    ui::{MainWindowState, capture_dialog, dialing_directory_dialog, settings_dialog, show_iemsi, terminal_window},
};

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum MainWindowMode {
    ShowTerminal,
    #[default]
    ShowDialingDirectory,
    ShowSettings,
    ShowHelpDialog,
    ShowAboutDialog,
    SelectProtocol(bool),
    FileTransfer(bool),
    ShowCaptureDialog,
    ShowExportDialog,
    ShowIEMSI,
    ShowFindDialog,
    ShowBaudEmulationDialog,
    ShowOpenSerialDialog(bool),
    ShowErrorDialog(String, String, String, Box<MainWindowMode>),
}

pub struct MainWindow {
    pub id: usize,
    pub state: MainWindowState,
    pub dialing_directory: dialing_directory_dialog::DialingDirectoryState,
    pub settings_dialog: settings_dialog::SettingsDialogState,
    pub capture_dialog: capture_dialog::CaptureDialogState,
    pub terminal_window: terminal_window::TerminalWindow,
    pub iemsi_dialog: show_iemsi::ShowIemsiDialog,
    pub find_dialog: find_dialog::DialogState,
    pub export_dialog: export_screen_dialog::ExportScreenDialogState,
    pub file_transfer_dialog: up_download_dialog::FileTransferDialogState,
    pub baud_emulation_dialog: super::select_bps_dialog::SelectBpsDialog,
    pub open_serial_dialog: super::open_serial_dialog::OpenSerialDialog,
    pub help_dialog: crate::ui::dialogs::help_dialog::HelpDialog,
    pub about_dialog: crate::ui::dialogs::about_dialog::AboutDialog,

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
}

impl MainWindow {
    pub fn new(
        id: usize,
        mode: MainWindowMode,
        sound_thread: Arc<Mutex<SoundThread>>,
        addresses: Arc<Mutex<AddressBook>>,
        options: Arc<Mutex<Options>>,
        temp_options: Arc<Mutex<Options>>,
    ) -> Self {
        let default_capture_path: PathBuf = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let default_export_path = directories::UserDirs::new()
            .and_then(|dirs: directories::UserDirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("export.icy");

        let terminal_window: super::TerminalWindow = terminal_window::TerminalWindow::new(sound_thread.clone());
        let edit_screen = terminal_window.terminal.screen.clone();

        let (terminal_tx, terminal_rx) = create_terminal_thread(edit_screen.clone(), addresses.clone());

        let serial = options.lock().serial.clone();

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
            settings_dialog: settings_dialog::SettingsDialogState::new(options, temp_options),
            capture_dialog: capture_dialog::CaptureDialogState::new(default_capture_path.to_string_lossy().to_string()),
            terminal_window,
            iemsi_dialog: show_iemsi::ShowIemsiDialog::new(icy_net::iemsi::EmsiISI::default()),
            find_dialog: find_dialog::DialogState::new(),
            export_dialog: export_screen_dialog::ExportScreenDialogState::new(default_export_path.to_string_lossy().to_string()),
            file_transfer_dialog: FileTransferDialogState::new(),
            baud_emulation_dialog: super::select_bps_dialog::SelectBpsDialog::new(BaudEmulation::Off),
            open_serial_dialog: super::open_serial_dialog::OpenSerialDialog::new(serial),
            help_dialog: crate::ui::dialogs::help_dialog::HelpDialog::new(),
            about_dialog: crate::ui::dialogs::about_dialog::AboutDialog::new(super::about_dialog::ABOUT_ANSI),

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
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DialingDirectory(msg) => self.dialing_directory.update(msg),
            Message::Connect(address) => {
                let modem = if matches!(address.protocol, ConnectionType::Modem) {
                    let options = &self.settings_dialog.original_options.lock();
                    // Find the modem in options that matches the address
                    let modem_opt = options.modems.iter().find(|m| m.name == address.address);

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
                let options = &self.settings_dialog.original_options.lock();

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
                    data.push(converted_byte as u8);
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
                    data.push(converted_byte as u8);
                }
                let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
                Task::none()
            }

            Message::TerminalEvent(event) => self.handle_terminal_event(event),
            Message::CaptureDialog(msg) => {
                if let Some(close_msg) = self.capture_dialog.update(msg) {
                    return self.update(close_msg);
                }
                Task::none()
            }
            Message::ShowIemsi(msg) => {
                if let Some(response) = self.iemsi_dialog.update(msg) {
                    return self.update(response);
                }
                Task::none()
            }
            Message::ShowIemsiDialog => {
                self.switch_to_terminal_screen();
                // Get IEMSI info from terminal if available
                if let Some(iemsi_info) = &self.terminal_window.iemsi_info {
                    self.iemsi_dialog = show_iemsi::ShowIemsiDialog::new(iemsi_info.clone());
                    self.state.mode = MainWindowMode::ShowIEMSI;
                }
                Task::none()
            }
            Message::SettingsDialog(msg) => {
                if let Some(close_msg) = self.settings_dialog.update(msg) {
                    let c = self.update(close_msg);
                    return c;
                }
                Task::none()
            }
            Message::ShowHelpDialog => {
                self.switch_to_terminal_screen();
                /*
                let r = self.sound_thread.lock().play_igs(Box::new(IgsCommand::BellsAndWhistles {
                    sound_effect: icy_parser_core::SoundEffect::try_from(self.effect).unwrap(),
                }));
                self.effect = (self.effect + 1) % 20;
                if let Err(r) = r {
                    log::error!("TerminalEvent::PlayMusic: {r}");
                }*/

                self.state.mode = MainWindowMode::ShowHelpDialog;

                Task::none()
            }
            Message::ShowAboutDialog => {
                self.switch_to_terminal_screen();
                self.state.mode = MainWindowMode::ShowAboutDialog;
                Task::none()
            }
            Message::CloseDialog(mode) => {
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
                self.state.mode = MainWindowMode::SelectProtocol(false);
                Task::none()
            }
            Message::Download => {
                self.switch_to_terminal_screen();
                self.state.mode = MainWindowMode::SelectProtocol(true);
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
                self.state.mode = MainWindowMode::ShowSettings;
                Task::none()
            }
            Message::ShowCaptureDialog => {
                self.switch_to_terminal_screen();
                // Update capture dialog with current capture path from options
                let capture_path = self.settings_dialog.original_options.lock().capture_path();
                self.capture_dialog.reset(&capture_path, self.capture_dialog.is_capturing());
                self.state.mode = MainWindowMode::ShowCaptureDialog;
                Task::none()
            }
            Message::StartCapture(file_name) => {
                self.state.mode = MainWindowMode::ShowTerminal;
                self.capture_dialog.capture_session = true;
                self.terminal_window.is_capturing = true;
                let _ = self.terminal_tx.send(TerminalCommand::StartCapture(file_name));
                Task::none()
            }
            Message::StopCapture => {
                self.capture_dialog.capture_session = false;
                self.terminal_window.is_capturing = false;
                let _ = self.terminal_tx.send(TerminalCommand::StopCapture);
                self.state.mode = MainWindowMode::ShowTerminal;
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
            Message::ShowFindDialog => {
                self.switch_to_terminal_screen();
                self.state.mode = MainWindowMode::ShowFindDialog;
                return self.find_dialog.focus_search_input();
            }
            Message::ShowBaudEmulationDialog => {
                self.switch_to_terminal_screen();
                self.state.mode = MainWindowMode::ShowBaudEmulationDialog;
                self.baud_emulation_dialog.set_emulation(self.terminal_window.baud_emulation);
                Task::none()
            }
            Message::SelectBpsMsg(msg) => {
                if let Some(message) = self.baud_emulation_dialog.update(msg) {
                    return self.update(message);
                }
                Task::none()
            }
            Message::ApplyBaudEmulation => {
                let baud: BaudEmulation = self.baud_emulation_dialog.get_emulation();
                self.terminal_window.baud_emulation = baud;
                let _ = self.terminal_tx.send(TerminalCommand::SetBaudEmulation(baud));
                self.state.mode = MainWindowMode::ShowTerminal;
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
                self.settings_dialog.original_options.lock().serial = serial.clone();
                if let Err(e) = self.settings_dialog.original_options.lock().store_options() {
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
                self.state.mode = MainWindowMode::ShowExportDialog;
                Task::none()
            }
            Message::ExportDialog(msg) => {
                if let Some(response) = self.export_dialog.update(msg, self.terminal_window.terminal.screen.clone()) {
                    match response {
                        Message::CloseDialog(mode) => {
                            self.state.mode = *mode;
                        }
                        _ => {}
                    }
                    return Task::none();
                }
                Task::none()
            }
            Message::StopSound => {
                self.sound_thread.lock().clear();
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

                    let text = match screen.get_copy_text() {
                        Some(t) => t,
                        None => return Task::none(),
                    };

                    let mut contents = Vec::with_capacity(4);

                    // On windows the ordering is important - text must be last to be recognized properly
                    if let Some(data) = screen.get_clipboard_data() {
                        contents.push(ClipboardContent::Other(ICY_CLIPBOARD_TYPE.into(), data));
                    }

                    if let Some(selection) = screen.get_selection() {
                        let (size, data) = screen.render_to_rgba(&RenderOptions {
                            rect: selection,
                            blink_on: true,
                            selection: None,
                            selection_fg: None,
                            selection_bg: None,
                            override_scan_lines: None,
                        });
                        let dynamic_image =
                            DynamicImage::ImageRgba8(image::ImageBuffer::from_raw(size.width as u32, size.height as u32, data).expect("rgba create"));
                        let img = clipboard_rs::RustImageData::from_dynamic_image(dynamic_image);
                        contents.push(ClipboardContent::Image(img));
                    }

                    if let Some(rich_text) = screen.get_copy_rich_text() {
                        contents.push(ClipboardContent::Rtf(rich_text));
                    }

                    contents.push(ClipboardContent::Text(text));

                    if let Err(err) = crate::CLIPBOARD_CONTEXT.set(contents) {
                        log::error!("Failed to set clipboard: {err}");
                    }

                    let _ = screen.clear_selection();
                    self.shift_pressed_during_selection = false;
                }
                Task::none()
            }

            Message::Paste => {
                self.clear_selection();
                match crate::CLIPBOARD_CONTEXT.get_text() {
                    Ok(text) => {
                        // Convert text to bytes using the current unicode converter
                        let mut data: Vec<u8> = Vec::new();
                        let buffer_type = self.terminal_window.terminal.screen.lock().buffer_type();
                        for ch in text.chars() {
                            let converted_byte = buffer_type.convert_from_unicode(ch);
                            data.push(converted_byte as u8);
                        }

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
                    self.capture_dialog.capture_session = false;
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
                        data.push(converted_byte as u8);
                    }
                    let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
                }
                Task::none()
            }

            Message::ScrollViewport(dx, dy) => {
                self.terminal_window.terminal.viewport.scroll_by(dx, dy);
                Task::none()
            }

            Message::ScrollViewportTo(smooth, x, y) => {
                // Immediate scroll for scrollbar interaction (no smooth animation)
                if smooth {
                    self.terminal_window.terminal.viewport.scroll_to(x, y);
                } else {
                    self.terminal_window.terminal.viewport.scroll_to_immediate(x, y);
                }
                self.terminal_window.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }

            Message::ViewportTick => {
                // Update viewport animation
                self.terminal_window.terminal.viewport.update_animation();
                Task::none()
            }

            Message::ScrollbarHovered(is_hovered) => {
                // Update scrollbar hover state for animation
                self.terminal_window.terminal.scrollbar.set_hovered(is_hovered);
                Task::none()
            }

            Message::CursorLeftWindow => {
                // Fade out scrollbar when cursor leaves window
                self.terminal_window.terminal.scrollbar.set_hovered(false);
                // Reset hover tracking state so next cursor move triggers hover update
                self.terminal_window
                    .terminal
                    .scrollbar_hover_state
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                Task::none()
            }

            Message::SetScrollbackBufferSize(buffer_size) => {
                {
                    let mut screen = self.terminal_window.terminal.screen.lock();
                    screen.set_scrollback_buffer_size(buffer_size);
                }
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
                        // Start file upload
                        if let Ok(protocol_type) = protocol.parse::<icy_net::protocol::TransferProtocolType>() {
                            let path = PathBuf::from(file_path);
                            if path.exists() {
                                let _ = self.terminal_tx.send(TerminalCommand::StartUpload(protocol_type, vec![path]));
                                self.state.mode = MainWindowMode::FileTransfer(false);
                            }
                        }
                    }
                    McpCommand::DownloadFile { protocol, save_path } => {
                        // Start file download
                        if let Ok(protocol_type) = protocol.parse::<icy_net::protocol::TransferProtocolType>() {
                            let _ = self.terminal_tx.send(TerminalCommand::StartDownload(protocol_type, Some(save_path.clone())));
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
                                screen_size: (screen.get_size().width as usize, screen.get_size().height as usize),
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
                    if let Some(mut sel) = screen.get_selection().clone() {
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
                    if let Some(mut sel) = screen.get_selection().clone() {
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
        }
    }

    fn initiate_file_transfer(&mut self, protocol: icy_net::protocol::TransferProtocolType, is_download: bool) {
        if is_download {
            // Set download directory from options
            let download_path = self.settings_dialog.original_options.lock().download_path();
            let _ = self.terminal_tx.send(TerminalCommand::SetDownloadDirectory(PathBuf::from(download_path)));
            let _ = self.terminal_tx.send(TerminalCommand::StartDownload(protocol, None));
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
            TerminalEvent::Error(error, txt) => {
                self.state.mode = MainWindowMode::ShowErrorDialog("Terminal Error".to_string(), error, txt, Box::new(MainWindowMode::ShowTerminal));
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
                let dial_tone = self.settings_dialog.original_options.lock().dial_tone;
                let r = self.sound_thread.lock().start_line_sound(dial_tone);
                if let Err(r) = r {
                    log::error!("TerminalEvent::OpenLineSound: {r}");
                }
                Task::none()
            }

            TerminalEvent::OpenDialSound(tone_dial, phone_number) => {
                let dial_tone = self.settings_dialog.original_options.lock().dial_tone;
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
            TerminalEvent::AutoTransferTriggered(protocol, is_download, _) => {
                self.initiate_file_transfer(protocol, is_download);
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
                        self.state.mode = MainWindowMode::ShowErrorDialog(
                            i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-script-title"),
                            i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-script-execution-failed"),
                            e,
                            Box::new(MainWindowMode::ShowTerminal),
                        );
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
        if self.get_mode() == MainWindowMode::ShowSettings {
            self.settings_dialog.temp_options.lock().monitor_settings.get_theme()
        } else {
            self.settings_dialog.original_options.lock().monitor_settings.get_theme()
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.state.mode {
            MainWindowMode::ShowDialingDirectory => return self.dialing_directory.view(&self.settings_dialog.original_options.lock()),
            MainWindowMode::ShowAboutDialog => return self.about_dialog.view(),
            _ => {}
        }

        let terminal_view = {
            let settings = if self.get_mode() == MainWindowMode::ShowSettings {
                &self.settings_dialog.temp_options.lock()
            } else {
                &self.settings_dialog.original_options.lock()
            };
            self.terminal_window.view(settings, &self.pause_message)
        };

        match &self.state.mode {
            MainWindowMode::ShowTerminal => terminal_view,
            MainWindowMode::ShowSettings => self.settings_dialog.view(terminal_view),
            MainWindowMode::SelectProtocol(download) => crate::ui::dialogs::protocol_selector::view_selector(*download, terminal_view),
            MainWindowMode::FileTransfer(download) => self.file_transfer_dialog.view(*download, terminal_view),
            MainWindowMode::ShowCaptureDialog => self.capture_dialog.view(terminal_view),
            MainWindowMode::ShowExportDialog => self.export_dialog.view(terminal_view),
            MainWindowMode::ShowIEMSI => self.iemsi_dialog.view(terminal_view),
            MainWindowMode::ShowFindDialog => find_dialog::find_dialog_overlay(&self.find_dialog, terminal_view),
            MainWindowMode::ShowBaudEmulationDialog => self.baud_emulation_dialog.view(terminal_view),
            MainWindowMode::ShowOpenSerialDialog(visible) => {
                if *visible {
                    self.open_serial_dialog.view(terminal_view)
                } else {
                    terminal_view
                }
            }
            MainWindowMode::ShowHelpDialog => self.help_dialog.view(terminal_view),
            MainWindowMode::ShowErrorDialog(title, secondary_msg, error_message, _) => {
                let dialog = ConfirmationDialog::new(title, error_message)
                    .dialog_type(DialogType::Error)
                    .secondary_message(secondary_msg)
                    .buttons(ButtonSet::Close);

                dialog.view(terminal_view, |_result| Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
            _ => {
                panic!("Unhandled main window mode in view()")
            }
        }
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

    fn switch_to_terminal_screen(&mut self) {
        self.state.mode = MainWindowMode::ShowTerminal;
    }

    pub fn handle_event(&self, event: &Event) -> Option<Message> {
        match event {
            Event::Window(window::Event::Focused) => {
                return Some(Message::SetFocus(true));
            }
            Event::Window(window::Event::Unfocused) => {
                return Some(Message::SetFocus(false));
            }
            Event::Mouse(iced::mouse::Event::CursorLeft) => {
                return Some(Message::CursorLeftWindow);
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
                    return Some(Message::ScrollViewport(0.0, scroll_amount));
                }
            }

            _ => {}
        }

        match &self.state.mode {
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
            MainWindowMode::SelectProtocol(_) => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
                    _ => None,
                },
                _ => None,
            },
            MainWindowMode::ShowSettings => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::SettingsDialog(settings_dialog::SettingsMsg::Cancel)),
                    _ => self.dialing_directory.handle_event(event),
                },
                _ => None,
            },
            MainWindowMode::ShowCaptureDialog => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CaptureDialog(capture_dialog::CaptureMsg::Cancel)),
                    keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::CaptureDialog(capture_dialog::CaptureMsg::StartCapture)),
                    _ => self.dialing_directory.handle_event(event),
                },
                _ => None,
            },
            MainWindowMode::ShowIEMSI => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::ShowIemsi(show_iemsi::IemsiMsg::Close)),
                    _ => None,
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
                        // Handle scrollback mode navigation
                        if self.terminal_window.terminal.is_in_scrollback_mode() {
                            // Get font height for line-based scrolling
                            let line_height = self.terminal_window.terminal.char_height;
                            let page_height = self.terminal_window.terminal.viewport.visible_height;

                            match key {
                                // ESC exits scrollback mode
                                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                                    return Some(Message::ShowScrollback);
                                }
                                // Arrow Up: scroll up one line
                                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                                    return Some(Message::ScrollViewport(0.0, -line_height));
                                }
                                // Arrow Down: scroll down one line
                                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                                    return Some(Message::ScrollViewport(0.0, line_height));
                                }
                                // Page Up: scroll up one screen
                                keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                                    return Some(Message::ScrollViewport(0.0, -page_height));
                                }
                                // Page Down: scroll down one screen
                                keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                                    return Some(Message::ScrollViewport(0.0, page_height));
                                }
                                // Home: scroll to top
                                keyboard::Key::Named(keyboard::key::Named::Home) => {
                                    return Some(Message::ScrollViewportTo(true, 0.0, 0.0));
                                }
                                // End: scroll to bottom
                                keyboard::Key::Named(keyboard::key::Named::End) => {
                                    let max_y = self.terminal_window.terminal.viewport.max_scroll_y();
                                    return Some(Message::ScrollViewportTo(true, 0.0, max_y));
                                }
                                // Any other key exits scrollback mode
                                _ => {
                                    return Some(Message::ShowScrollback);
                                }
                            }
                        }

                        #[cfg(target_os = "macos")]
                        let cmd_key = modifiers.command();
                        #[cfg(not(target_os = "macos"))]
                        let cmd_key = modifiers.alt();

                        // Handle Alt+Enter for fullscreen toggle
                        if cmd_key && matches!(key, keyboard::Key::Named(keyboard::key::Named::Enter)) {
                            return Some(Message::ToggleFullscreen);
                        }

                        if cmd_key {
                            match &key {
                                keyboard::Key::Named(named) => match named {
                                    keyboard::key::Named::PageUp => return Some(Message::Upload),
                                    keyboard::key::Named::PageDown => return Some(Message::Download),
                                    _ => {}
                                },
                                keyboard::Key::Character(s) => match s.to_lowercase().as_str() {
                                    "f" => return Some(Message::ShowFindDialog),
                                    "i" => return Some(Message::ShowExportScreenDialog),
                                    "d" => return Some(Message::ShowDialingDirectory),
                                    "h" => return Some(Message::Hangup),
                                    "l" => return Some(Message::SendLoginAndPassword(true, true)),
                                    "u" => return Some(Message::SendLoginAndPassword(true, false)),
                                    "s" => return Some(Message::SendLoginAndPassword(false, true)),
                                    "o" => return Some(Message::ShowSettings),
                                    "p" => return Some(Message::ShowCaptureDialog),
                                    "x" => return Some(Message::QuitIcyTerm),
                                    "a" => return Some(Message::ShowAboutDialog),
                                    #[cfg(not(target_os = "macos"))]
                                    "c" => return Some(Message::ClearScreen),
                                    "b" => return Some(Message::ShowScrollback),
                                    "r" => return Some(Message::ShowRunScriptDialog),
                                    "t" => return Some(Message::ShowOpenSerialDialog),
                                    _ => {}
                                },
                                _ => {}
                            }
                        } else if modifiers.control() {
                            #[cfg(not(target_os = "macos"))]
                            if let keyboard::Key::Character(s) = &key {
                                match s.to_lowercase().as_str() {
                                    "c" => return Some(Message::Copy),
                                    "v" => return Some(Message::Paste),
                                    _ => {}
                                }
                            }
                        } else if modifiers.alt() {
                            // On macOS, use Option (Alt) for clear screen to avoid conflict with Cmd+C (copy)
                            #[cfg(target_os = "macos")]
                            if let keyboard::Key::Character(s) = &key {
                                if s.to_lowercase() == "c" {
                                    return Some(Message::ClearScreen);
                                }
                            }
                        } else if modifiers.is_empty() {
                            // Handle function keys without modifiers
                            match &key {
                                keyboard::Key::Named(named) => match named {
                                    keyboard::key::Named::F1 => return Some(Message::ShowHelpDialog),
                                    _ => {}
                                },
                                _ => {}
                            }
                        }

                        // Try to map the key with modifiers using the key map
                        if let Some(bytes) = Self::map_key_event_to_bytes(self.terminal_emulation, &key, &physical_key, *modifiers) {
                            return Some(Message::SendData(bytes));
                        }

                        if let Some(text) = text {
                            Some(Message::SendString(text.to_string()))
                        } else {
                            None
                        }
                    } /*
                    Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Shift),
                    ..
                    }) => Some(Message::ShiftPressed(true)),
                    Event::Keyboard(keyboard::Event::KeyReleased {
                    key: keyboard::Key::Named(keyboard::key::Named::Shift),
                    ..
                    }) => Some(Message::ShiftPressed(false)),*/
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
            MainWindowMode::ShowExportDialog => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::ExportDialog(export_screen_dialog::ExportScreenMsg::Cancel)),
                    _ => self.dialing_directory.handle_event(event),
                },
                _ => None,
            },
            MainWindowMode::ShowBaudEmulationDialog => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
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
            MainWindowMode::ShowHelpDialog | MainWindowMode::ShowAboutDialog => match event {
                Event::Keyboard(keyboard::Event::KeyPressed { .. }) => Some(Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
                _ => None,
            },
            _ => {
                // Handle global shortcuts that work in any mode
                match event {
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        key: keyboard::Key::Named(keyboard::key::Named::Enter),
                        modifiers,
                        ..
                    }) => {
                        #[cfg(target_os = "macos")]
                        let cmd_key = modifiers.command();
                        #[cfg(not(target_os = "macos"))]
                        let cmd_key = modifiers.alt();

                        if cmd_key { Some(Message::ToggleFullscreen) } else { None }
                    }
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        key: keyboard::Key::Named(keyboard::key::Named::Shift),
                        ..
                    }) => Some(Message::ShiftPressed(true)),
                    Event::Keyboard(keyboard::Event::KeyReleased {
                        key: keyboard::Key::Named(keyboard::key::Named::Shift),
                        ..
                    }) => Some(Message::ShiftPressed(false)),
                    _ => None,
                }
            }
        }
    }
}
