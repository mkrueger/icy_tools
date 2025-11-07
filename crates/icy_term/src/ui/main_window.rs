use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
    vec,
};

use crate::{
    McpHandler, get_unicode_converter,
    mcp::{self, McpCommand, types::ScreenCaptureFormat},
    ui::{
        Message,
        dialogs::find_dialog,
        error_dialog, export_screen_dialog,
        up_download_dialog::{self, FileTransferDialogState},
    },
    util::SoundThread,
};

use clipboard_rs::{Clipboard, ClipboardContent, common::RustImage};
use iced::{Element, Event, Task, Theme, keyboard, window};
use icy_engine::{Position, RenderOptions, TextPane, UnicodeConverter, ansi::BaudEmulation, editor::ICY_CLIPBOARD_TYPE};
use icy_net::{ConnectionType, telnet::TerminalEmulation};
use image::DynamicImage;
use tokio::sync::mpsc;

use crate::{
    Address, AddressBook, Options,
    terminal::terminal_thread::{ConnectionConfig, TerminalCommand, TerminalEvent, create_terminal_thread},
    terminal_thread::ModemConfig,
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

    unicode_converter: Box<dyn UnicodeConverter>,

    // Capture state
    capture_file: Option<PathBuf>,
    captured_data: Vec<u8>,

    _is_fullscreen_mode: bool,
    _last_pos: Position,
    shift_pressed_during_selection: bool,
    _use_rip: bool,

    pub initial_upload_directory: Option<PathBuf>,
    pub show_find_dialog: bool,
    show_disconnect: bool,

    pub mcp_rx: McpHandler,
    pub title: String,
}

static mut TERM_EMULATION: TerminalEmulation = TerminalEmulation::Ansi;

impl MainWindow {
    pub fn new(
        id: usize,
        mode: MainWindowMode,
        sound_thread: Arc<Mutex<SoundThread>>,
        addresses: Arc<Mutex<AddressBook>>,
        options: Arc<Mutex<Options>>,
        temp_options: Arc<Mutex<Options>>,
    ) -> Self {
        let default_capture_path = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("capture.ans");

        let default_export_path = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("export.icy");

        let terminal_window: super::TerminalWindow = terminal_window::TerminalWindow::new(sound_thread.clone());
        let edit_state = terminal_window.terminal.edit_state.clone();

        // Create terminal thread
        let (terminal_tx, terminal_rx) = create_terminal_thread(edit_state.clone(), icy_net::telnet::TerminalEmulation::Ansi);

        Self {
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
            help_dialog: crate::ui::dialogs::help_dialog::HelpDialog::new(),
            about_dialog: crate::ui::dialogs::about_dialog::AboutDialog::new(super::about_dialog::ABOUT_ANSI),

            terminal_tx,
            terminal_rx: Some(terminal_rx),

            is_connected: false,
            connection_time: None,
            current_address: None,
            last_address: None,

            capture_file: None,
            captured_data: Vec::new(),

            unicode_converter: get_unicode_converter(&icy_net::telnet::TerminalEmulation::Ansi),

            _is_fullscreen_mode: false,
            _last_pos: Position::default(),
            shift_pressed_during_selection: false,
            _use_rip: false,
            initial_upload_directory: None,
            show_find_dialog: false,
            show_disconnect: false,
            sound_thread,
            mcp_rx: None,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DialingDirectory(msg) => self.dialing_directory.update(msg),
            Message::Connect(address) => {
                let modem = if matches!(address.protocol, ConnectionType::Modem) {
                    let options = &self.settings_dialog.original_options.lock().unwrap();
                    // Find the modem in options that matches the address
                    let modem_opt = options.modems.iter().find(|m| m.name == address.address);

                    if let Some(modem_config) = modem_opt {
                        Some(ModemConfig {
                            device: modem_config.device.clone(),
                            baud_rate: modem_config.baud_rate,
                            char_size: modem_config.char_size,
                            parity: modem_config.parity,
                            stop_bits: modem_config.stop_bits,
                            flow_control: modem_config.flow_control,
                            init_string: modem_config.init_string.clone(),
                            dial_string: modem_config.dial_string.clone(),
                        })
                    } else {
                        // No modem configured - show error and abort connection
                        let error_msg = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "connect-error-no-modem-configured");
                        log::error!("{}", error_msg);

                        // Display error message in terminal
                        if let Ok(mut edit_state) = self.terminal_window.terminal.edit_state.lock() {
                            let (buffer, caret, _) = edit_state.get_buffer_and_caret_mut();
                            buffer.clear_screen(0, caret);

                            // Write error message
                            for ch in error_msg.chars() {
                                buffer.print_char(
                                    0,
                                    caret,
                                    icy_engine::AttributedChar::new(
                                        ch,
                                        icy_engine::TextAttribute::from_color(4, 0), // Red on black
                                    ),
                                );
                            }
                            caret.cr(buffer);
                            caret.lf(buffer, 0);
                        }

                        self.state.mode = MainWindowMode::ShowTerminal;
                        return Task::none();
                    }
                } else {
                    None
                };
                let options = &self.settings_dialog.original_options.lock().unwrap();

                unsafe {
                    TERM_EMULATION = address.terminal_type;
                }
                self.unicode_converter = get_unicode_converter(&address.terminal_type);

                // Send connect command to terminal thread
                let config = ConnectionConfig {
                    connection_info: address.clone().into(),
                    terminal_type: address.terminal_type,
                    baud_emulation: address.baud_emulation,
                    window_size: (80, 25),
                    timeout: web_time::Duration::from_secs(30),
                    user_name: if address.user_name.is_empty() {
                        None
                    } else {
                        Some(address.user_name.clone())
                    },
                    password: if address.password.is_empty() { None } else { Some(address.password.clone()) },

                    proxy_command: None, // fill from settings if needed
                    modem,
                    music_option: address.ansi_music,
                    screen_mode: address.get_screen_mode(),
                    iemsi_auto_login: options.iemsi.autologin,
                    login_exp: address.auto_login.clone(),
                };

                let screen_mode = address.get_screen_mode();
                if let Ok(mut state) = self.terminal_window.terminal.edit_state.lock() {
                    let (buffer, caret, _) = state.get_buffer_and_caret_mut();
                    buffer.clear_screen(0, caret);
                    unsafe {
                        // Clear all sixel layers on connect
                        buffer.layers.set_len(1);
                    }
                    caret.set_is_visible(true);
                    screen_mode.apply_to_edit_state(&mut state);
                }
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
                self.terminal_window.terminal.edit_state.lock().unwrap().set_scroll_position(0);
                let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
                Task::none()
            }

            Message::SendString(s) => {
                self.clear_selection();
                self.terminal_window.terminal.edit_state.lock().unwrap().set_scroll_position(0);
                let mut data: Vec<u8> = Vec::new();
                for ch in s.chars() {
                    let converted_byte = self.unicode_converter.convert_from_unicode(ch, 0);
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
                        // Check if we have username and password
                        if !address.user_name.is_empty() || !address.password.is_empty() {
                            let mut data_to_send = Vec::new();

                            // Send username if available
                            if !address.user_name.is_empty() {
                                data_to_send.extend_from_slice(address.user_name.as_bytes());
                                data_to_send.push(b'\r'); // Send carriage return after username
                            }

                            // Small delay between username and password (some BBSs need this)
                            // We'll handle this by sending them as separate commands
                            if !address.user_name.is_empty() && !address.password.is_empty() {
                                if login {
                                    let username_data = address.user_name.as_bytes().to_vec();
                                    let mut username_with_cr = username_data;
                                    username_with_cr.push(b'\r');
                                    let _ = self.terminal_tx.send(TerminalCommand::SendData(username_with_cr));
                                    if pw {
                                        // Send password after a small delay
                                        // Note: In a real implementation, you might want to add a proper delay mechanism
                                        // For now, we'll send it immediately and rely on the terminal's buffering
                                        std::thread::sleep(std::time::Duration::from_millis(500));
                                    }
                                }

                                if pw {
                                    let password_data = address.password.as_bytes().to_vec();
                                    let mut password_with_cr = password_data;
                                    password_with_cr.push(b'\r');
                                    let _ = self.terminal_tx.send(TerminalCommand::SendData(password_with_cr));
                                }
                            } else if !address.user_name.is_empty() && login {
                                // Only username
                                let _ = self.terminal_tx.send(TerminalCommand::SendData(data_to_send));
                            } else if !address.password.is_empty() {
                                // Only password (unusual but possible)
                                data_to_send.extend_from_slice(address.password.as_bytes());
                                data_to_send.push(b'\r');
                                let _ = self.terminal_tx.send(TerminalCommand::SendData(data_to_send));
                            }
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
                if let Err(e) = webbrowser::open("https://github.com/mkrueger/icy_tools/releases") {
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
                self.state.mode = MainWindowMode::ShowCaptureDialog;
                Task::none()
            }
            Message::StartCapture(file_name) => {
                self.state.mode = MainWindowMode::ShowTerminal;
                self.capture_dialog.capture_session = true;
                self.terminal_window.is_capturing = true;
                self.capture_file = Some(PathBuf::from(file_name));
                self.captured_data.clear();
                Task::none()
            }
            Message::StopCapture => {
                self.capture_dialog.capture_session = false;
                self.terminal_window.is_capturing = false;

                // Save captured data to file if we have any
                if let Some(capture_file) = &self.capture_file {
                    if !self.captured_data.is_empty() {
                        if let Err(e) = std::fs::write(capture_file, &self.captured_data) {
                            log::error!("Failed to save capture file: {}", e);
                        }
                    }
                }

                self.capture_file = None;
                self.captured_data.clear();
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
            Message::FindDialog(msg) => {
                if let Some(close_msg) = self.find_dialog.update(msg, self.terminal_window.terminal.edit_state.clone()) {
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
                if let Some(response) = self.export_dialog.update(msg, self.terminal_window.terminal.edit_state.clone()) {
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
                self.sound_thread.lock().unwrap().clear();
                Task::none()
            }
            Message::None => Task::none(),

            Message::ScrollRelative(lines) => {
                let mut state = self.terminal_window.terminal.edit_state.lock().unwrap();
                let current_offset = state.scrollback_offset as i32;
                let max_offset = state.get_max_scrollback_offset() as i32;
                let new_offset = (current_offset - lines).clamp(0, max_offset) as usize;
                state.set_scroll_position(new_offset);
                Task::none()
            }

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
                // Implement clipboard copy from selection
                if let Ok(mut edit_state) = self.terminal_window.terminal.edit_state.lock() {
                    let mut vec = vec![];
                    if let Some(text) = edit_state.get_copy_text() {
                        vec.push(ClipboardContent::Text(text.clone()));
                    } else {
                        return Task::none();
                    }
                    if let Some(rich_text) = edit_state.get_copy_rich_text() {
                        vec.push(ClipboardContent::Rtf(rich_text));
                    }

                    if let Some(selection) = edit_state.get_selection() {
                        let (size, data) = edit_state.get_display_buffer().render_to_rgba(&RenderOptions {
                            rect: selection,
                            blink_on: true,
                            selection: None,
                            selection_fg: None,
                            selection_bg: None,
                        });

                        let dynamic_image = DynamicImage::ImageRgba8(
                            image::ImageBuffer::from_raw(size.width as u32, size.height as u32, data).expect("Failed to create image buffer from raw data"),
                        );
                        let img = clipboard_rs::RustImageData::from_dynamic_image(dynamic_image);
                        vec.push(ClipboardContent::Image(img));
                    }

                    if let Some(data) = edit_state.get_clipboard_data() {
                        vec.push(ClipboardContent::Other(ICY_CLIPBOARD_TYPE.into(), data));
                    }

                    if let Err(err) = crate::CLIPBOARD_CONTEXT.set(vec) {
                        log::error!("Failed to set clipboard content: {}", err);
                    }
                    // Clear selection after copy
                    let _ = edit_state.clear_selection();
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
                        for ch in text.chars() {
                            let converted_byte = self.unicode_converter.convert_from_unicode(ch, 0);
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

                    // Save captured data to file if we have any
                    if let Some(capture_file) = &self.capture_file {
                        if !self.captured_data.is_empty() {
                            if let Err(e) = std::fs::write(capture_file, &self.captured_data) {
                                log::error!("Failed to save capture file: {}", e);
                            }
                        }
                    }
                }
                // Stop sound thread
                self.sound_thread.lock().unwrap().clear();

                iced::exit()
            }
            Message::ClearScreen => {
                if let Ok(mut edit_state) = self.terminal_window.terminal.edit_state.lock() {
                    edit_state.clear_scrollback_buffer();
                    let (buffer, caret, _) = edit_state.get_buffer_and_caret_mut();
                    buffer.clear_screen(0, caret);
                    caret.set_position(Position::new(0, 0));
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
                if let Some(seq) = escape_sequence {
                    let mut data: Vec<u8> = Vec::new();
                    for ch in seq.chars() {
                        let converted_byte = self.unicode_converter.convert_from_unicode(ch, 0);
                        data.push(converted_byte as u8);
                    }
                    let _ = self.terminal_tx.send(TerminalCommand::SendData(data));
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
                        let bytes = self.parse_key_string(key);
                        if let Some(data) = bytes {
                            return self.update(Message::SendData(data));
                        }
                    }
                    McpCommand::CaptureScreen(format, response_tx) => {
                        // Capture the current screen in the requested format
                        let data = if let Ok(mut state) = self.terminal_window.terminal.edit_state.lock() {
                            let buffer = state.get_buffer_mut();
                            let mut opt = icy_engine::SaveOptions::default();
                            opt.modern_terminal_output = true;
                            match format {
                                ScreenCaptureFormat::Text => buffer.to_bytes("asc", &opt).unwrap_or_default(),
                                ScreenCaptureFormat::Ansi => buffer.to_bytes("ans", &opt).unwrap_or_default(),
                            }
                        } else {
                            Vec::new()
                        };

                        // Take the sender out of the Arc<Mutex> and send the response
                        if let Ok(mut tx_guard) = response_tx.lock() {
                            if let Some(tx) = tx_guard.take() {
                                let _ = tx.send(data);
                            }
                        }
                    }
                    McpCommand::SetTerminal { terminal_type, rows, columns } => {
                        // Update terminal settings
                        // Parse terminal type manually since FromStr is not implemented
                        let emulation = match terminal_type.to_lowercase().as_str() {
                            "ansi" | "ansi-bbs" => TerminalEmulation::Ansi,
                            "petscii" | "c64" | "commodore" => TerminalEmulation::PETscii,
                            "viewdata" => TerminalEmulation::ViewData,
                            "mode7" => TerminalEmulation::Mode7,
                            "atascii" | "atari" => TerminalEmulation::ATAscii,
                            "atarist" | "atari-st" => TerminalEmulation::AtariST,
                            _ => {
                                log::warn!("Unknown terminal type: {}, defaulting to ANSI", terminal_type);
                                TerminalEmulation::Ansi
                            }
                        };

                        unsafe {
                            TERM_EMULATION = emulation;
                        }
                        self.unicode_converter = get_unicode_converter(&emulation);
                        // Note: SetTerminalType command may not exist, we should update terminal_type in ConnectionConfig instead
                        // For now, just log it
                        log::info!("Terminal type changed to: {:?}", emulation);

                        if let (Some(rows), Some(cols)) = (rows, columns) {
                            // Resize terminal buffer
                            if let Ok(mut state) = self.terminal_window.terminal.edit_state.lock() {
                                let new_size = icy_engine::Size::new(*cols as i32, *rows as i32);
                                // Create a new buffer with the desired size
                                let mut new_buffer = icy_engine::Buffer::new(new_size);
                                // Copy content from old buffer if needed
                                let old_buffer = state.get_buffer();
                                for y in 0..new_size.height.min(old_buffer.get_size().height) {
                                    for x in 0..new_size.width.min(old_buffer.get_size().width) {
                                        let ch = old_buffer.get_char(icy_engine::Position::new(x, y));
                                        new_buffer.layers[0].set_char(icy_engine::Position::new(x, y), ch);
                                    }
                                }
                                *state.get_buffer_mut() = new_buffer;
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
                    McpCommand::SaveSession(file_path) => {
                        // Save the current terminal buffer to a file
                        if let Ok(mut state) = self.terminal_window.terminal.edit_state.lock() {
                            let buffer = state.get_buffer_mut();
                            if let Ok(bytes) = buffer.to_bytes("icy", &icy_engine::SaveOptions::default()) {
                                if let Err(e) = std::fs::write(file_path, bytes) {
                                    log::error!("Failed to save session: {}", e);
                                }
                            }
                        }
                    }
                    McpCommand::LoadSession(file_path) => {
                        // Load a session from a file
                        if let Ok(_bytes) = std::fs::read(file_path) {
                            if let Ok(mut state) = self.terminal_window.terminal.edit_state.lock() {
                                let path = PathBuf::from(file_path);
                                if let Ok(buffer) = icy_engine::Buffer::load_buffer(&path, true, None) {
                                    *state.get_buffer_mut() = buffer;
                                }
                            }
                        }
                    }
                    McpCommand::GetState(response_tx) => {
                        // Gather current terminal state
                        let state = if let Ok(edit_state) = self.terminal_window.terminal.edit_state.lock() {
                            let buffer = edit_state.get_display_buffer();
                            let cursor = edit_state.get_caret().get_position();
                            mcp::types::TerminalState {
                                cursor_position: (cursor.x as usize, cursor.y as usize),
                                screen_size: (buffer.get_size().width as usize, buffer.get_size().height as usize),
                                current_buffer: String::new(),
                                is_connected: self.is_connected,
                                current_bbs: self.current_address.as_ref().map(|addr| addr.system_name.clone()),
                            }
                        } else {
                            mcp::types::TerminalState {
                                cursor_position: (0, 0),
                                screen_size: (80, 25),
                                current_buffer: String::new(),
                                is_connected: false,
                                current_bbs: None,
                            }
                        };

                        // Take the sender out of the Arc<Mutex> and send the response
                        if let Ok(mut tx_guard) = response_tx.lock() {
                            if let Some(tx) = tx_guard.take() {
                                let _ = tx.send(state);
                            }
                        }
                    }

                    McpCommand::ListAddresses(response_tx) => {
                        // Get addresses from the address book
                        let addresses = if let Ok(book) = self.dialing_directory.addresses.lock() {
                            book.addresses.clone()
                        } else {
                            Vec::new()
                        };

                        // Take the sender out of the Arc<Mutex> and send the response
                        if let Ok(mut tx_guard) = response_tx.lock() {
                            if let Some(tx) = tx_guard.take() {
                                let _ = tx.send(addresses);
                            }
                        }
                    }
                }
                Task::none()
            }
        }
    }

    fn initiate_file_transfer(&mut self, protocol: icy_net::protocol::TransferProtocolType, is_download: bool) {
        if is_download {
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
            TerminalEvent::DataReceived(data) => {
                // Handle capture
                if self.terminal_window.is_capturing {
                    self.captured_data.extend_from_slice(&data);
                }
                Task::none()
            }
            TerminalEvent::Reconnect => {
                return self.update(Message::Reconnect);
            }
            TerminalEvent::Connect(address) => {
                if let Ok(e) = crate::ConnectionInformation::parse(&address) {
                    return self.update(Message::Connect(e.into()));
                }
                Task::none()
            } // 20forbeers.com:1337
            TerminalEvent::BufferUpdated => {
                let mut events = Vec::new();
                if let Some(rx) = &mut self.terminal_rx {
                    while let Ok(event) = rx.try_recv() {
                        events.push(event);
                    }
                }

                for event in events {
                    let _ = self.handle_terminal_event(event);
                }

                let mut mcp_commands = Vec::new();
                if let Some(rx) = &mut self.mcp_rx {
                    while let Ok(cmd) = rx.try_recv() {
                        mcp_commands.push(cmd);
                    }
                }

                for cmd in mcp_commands {
                    let _ = self.update(Message::McpCommand(Arc::new(cmd)));
                }

                Task::none()
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
            TerminalEvent::Error(error) => {
                log::error!("Terminal error: {}", error);
                // TODO: Show error dialog
                Task::none()
            }
            TerminalEvent::PlayMusic(music) => {
                let r = self.sound_thread.lock().unwrap().play_music(music);
                if let Err(r) = r {
                    log::error!("TerminalEvent::PlayMusic: {r}");
                }
                Task::none()
            }
            TerminalEvent::Beep => {
                let r = self.sound_thread.lock().unwrap().beep();
                if let Err(r) = r {
                    log::error!("TerminalEvent::Beep: {r}");
                }
                Task::none()
            }
            TerminalEvent::OpenLineSound => {
                let dial_tone = self.settings_dialog.original_options.lock().unwrap().dial_tone;
                let r = self.sound_thread.lock().unwrap().start_line_sound(dial_tone);
                if let Err(r) = r {
                    log::error!("TerminalEvent::OpenLineSound: {r}");
                }
                Task::none()
            }

            TerminalEvent::OpenDialSound(tone_dial, phone_number) => {
                let dial_tone = self.settings_dialog.original_options.lock().unwrap().dial_tone;
                let r = self.sound_thread.lock().unwrap().start_dial_sound(tone_dial, dial_tone, &phone_number);
                if let Err(r) = r {
                    log::error!("TerminalEvent::OpenDialSound: {r}");
                }
                Task::none()
            }

            TerminalEvent::StopSound => {
                let r = self.sound_thread.lock().unwrap().stop_line_sound();
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
        }
    }

    pub fn theme(&self) -> Theme {
        if self.get_mode() == MainWindowMode::ShowSettings {
            self.settings_dialog.temp_options.lock().unwrap().monitor_settings.get_theme()
        } else {
            self.settings_dialog.original_options.lock().unwrap().monitor_settings.get_theme()
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let terminal_view = {
            let settings = if self.get_mode() == MainWindowMode::ShowSettings {
                &self.settings_dialog.temp_options.lock().unwrap()
            } else {
                &self.settings_dialog.original_options.lock().unwrap()
            };
            self.terminal_window.view(settings)
        };

        match &self.state.mode {
            MainWindowMode::ShowTerminal => terminal_view,
            MainWindowMode::ShowDialingDirectory => self.dialing_directory.view(&self.settings_dialog.original_options.lock().unwrap()),
            MainWindowMode::ShowSettings => self.settings_dialog.view(terminal_view),
            MainWindowMode::SelectProtocol(download) => crate::ui::dialogs::protocol_selector::view_selector(*download, terminal_view),
            MainWindowMode::FileTransfer(download) => self.file_transfer_dialog.view(*download, terminal_view),
            MainWindowMode::ShowCaptureDialog => self.capture_dialog.view(terminal_view),
            MainWindowMode::ShowExportDialog => self.export_dialog.view(terminal_view),
            MainWindowMode::ShowIEMSI => self.iemsi_dialog.view(terminal_view),
            MainWindowMode::ShowFindDialog => find_dialog::find_dialog_overlay(&self.find_dialog, terminal_view),
            MainWindowMode::ShowBaudEmulationDialog => self.baud_emulation_dialog.view(terminal_view),
            MainWindowMode::ShowHelpDialog => self.help_dialog.view(terminal_view),
            MainWindowMode::ShowAboutDialog => self.about_dialog.view(),
            MainWindowMode::ShowErrorDialog(title, secondary_msg, error_message, _) => error_dialog::view(terminal_view, title, secondary_msg, error_message),
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
            icy_net::telnet::TerminalEmulation::PETscii => iced_engine_gui::key_map::C64_KEY_MAP,
            icy_net::telnet::TerminalEmulation::ViewData | icy_net::telnet::TerminalEmulation::Mode7 => iced_engine_gui::key_map::VIDEOTERM_KEY_MAP,
            icy_net::telnet::TerminalEmulation::AtariST | icy_net::telnet::TerminalEmulation::ATAscii => iced_engine_gui::key_map::ATASCII_KEY_MAP,
            _ => iced_engine_gui::key_map::ANSI_KEY_MAP,
        };

        // Use the lookup_key function from the key_map module
        iced_engine_gui::key_map::lookup_key(key, physical, modifiers, key_map)
    }

    fn clear_selection(&mut self) {
        if let Ok(mut edit_state) = self.terminal_window.terminal.edit_state.lock() {
            let _ = edit_state.clear_selection();
            self.shift_pressed_during_selection = false;
        }
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
                                    "c" => return Some(Message::ClearScreen),
                                    _ => {}
                                },
                                _ => {}
                            }
                        } else if modifiers.command() {
                            if let keyboard::Key::Character(s) = &key {
                                if s.to_lowercase() == "c" {
                                    return Some(Message::Copy);
                                }
                                if s.to_lowercase() == "v" {
                                    return Some(Message::Paste);
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
                        if let Some(bytes) = Self::map_key_event_to_bytes(unsafe { TERM_EMULATION }, &key, &physical_key, *modifiers) {
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

    fn parse_key_string(&self, key_str: &str) -> Option<Vec<u8>> {
        let key = match key_str.to_lowercase().as_str() {
            "enter" => keyboard::Key::Named(keyboard::key::Named::Enter),
            "escape" | "esc" => keyboard::Key::Named(keyboard::key::Named::Escape),
            "tab" => keyboard::Key::Named(keyboard::key::Named::Tab),
            "backspace" => keyboard::Key::Named(keyboard::key::Named::Backspace),
            "delete" | "del" => keyboard::Key::Named(keyboard::key::Named::Delete),
            "home" => keyboard::Key::Named(keyboard::key::Named::Home),
            "end" => keyboard::Key::Named(keyboard::key::Named::End),
            "pageup" | "pgup" => keyboard::Key::Named(keyboard::key::Named::PageUp),
            "pagedown" | "pgdn" => keyboard::Key::Named(keyboard::key::Named::PageDown),
            "up" | "arrowup" => keyboard::Key::Named(keyboard::key::Named::ArrowUp),
            "down" | "arrowdown" => keyboard::Key::Named(keyboard::key::Named::ArrowDown),
            "left" | "arrowleft" => keyboard::Key::Named(keyboard::key::Named::ArrowLeft),
            "right" | "arrowright" => keyboard::Key::Named(keyboard::key::Named::ArrowRight),
            "f1" => keyboard::Key::Named(keyboard::key::Named::F1),
            "f2" => keyboard::Key::Named(keyboard::key::Named::F2),
            "f3" => keyboard::Key::Named(keyboard::key::Named::F3),
            "f4" => keyboard::Key::Named(keyboard::key::Named::F4),
            "f5" => keyboard::Key::Named(keyboard::key::Named::F5),
            "f6" => keyboard::Key::Named(keyboard::key::Named::F6),
            "f7" => keyboard::Key::Named(keyboard::key::Named::F7),
            "f8" => keyboard::Key::Named(keyboard::key::Named::F8),
            "f9" => keyboard::Key::Named(keyboard::key::Named::F9),
            "f10" => keyboard::Key::Named(keyboard::key::Named::F10),
            "f11" => keyboard::Key::Named(keyboard::key::Named::F11),
            "f12" => keyboard::Key::Named(keyboard::key::Named::F12),
            _ => return None,
        };

        // Check for modifiers in the key string (e.g., "Ctrl+C", "Alt+F4")
        let modifiers = if key_str.contains("ctrl+") || key_str.contains("control+") {
            keyboard::Modifiers::CTRL
        } else if key_str.contains("alt+") {
            keyboard::Modifiers::ALT
        } else if key_str.contains("shift+") {
            keyboard::Modifiers::SHIFT
        } else {
            keyboard::Modifiers::empty()
        };

        let physical = keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified);

        Self::map_key_event_to_bytes(unsafe { TERM_EMULATION }, &key, &physical, modifiers)
    }
}
