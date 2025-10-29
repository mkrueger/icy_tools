use std::{
    path::PathBuf,
    ptr::addr_eq,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::{
    ui::{
        dialogs::find_dialog,
        export_dialog,
        up_download_dialog::{self, FileTransferDialogState},
    },
    util::SoundThread,
};
use i18n_embed_fl::fl;
use iced::{Element, Event, Task, Theme, keyboard};
use icy_engine::{Position, editor::EditState};
use icy_net::{ConnectionType, protocol::TransferState};
use tokio::sync::mpsc;

use crate::{
    Address, AddressBook, Options, ScreenMode,
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
    SelectProtocol(bool),
    FileTransfer(bool),
    ShowCaptureDialog,
    ShowExportDialog,
    ShowIEMSI,
    ShowFindDialog,
}

#[derive(Debug, Clone)]
pub enum Message {
    DialingDirectory(crate::ui::dialogs::dialing_directory_dialog::DialingDirectoryMsg),
    SettingsDialog(crate::ui::dialogs::settings_dialog::SettingsMsg),
    CaptureDialog(crate::ui::dialogs::capture_dialog::CaptureMsg),
    ShowIemsi(crate::ui::dialogs::show_iemsi::IemsiMsg),
    FindDialog(find_dialog::FindDialogMsg),
    ExportDialog(export_dialog::ExportMsg),
    TransferDialog(up_download_dialog::TransferMsg),
    CancelFileTransfer,
    UpdateTransferState(TransferState),
    ShowExportDialog,
    Connect(Address),
    CloseDialog,
    Disconnect,
    ShowDialingDirectory,
    ShowSettings,
    ShowCaptureDialog,
    ShowFindDialog,
    Upload,
    Download,
    SendLogin,
    InitiateFileTransfer {
        protocol: icy_net::protocol::TransferProtocolType,
        is_download: bool,
    },
    OpenReleaseLink,
    StartCapture(String),
    StopCapture,
    ShowIemsiDialog,
    // Terminal thread events
    TerminalEvent(TerminalEvent),
    SendData(Vec<u8>),
    None,
    StopSound,
}

pub struct MainWindow {
    pub state: MainWindowState,
    pub dialing_directory: dialing_directory_dialog::DialingDirectoryState,
    pub settings_dialog: settings_dialog::SettingsDialogState,
    pub capture_dialog: capture_dialog::CaptureDialogState,
    pub terminal_window: terminal_window::TerminalWindow,
    pub iemsi_dialog: show_iemsi::ShowIemsiDialog,
    pub find_dialog: find_dialog::DialogState,
    pub export_dialog: export_dialog::ExportDialogState,
    pub file_transfer_dialog: up_download_dialog::FileTransferDialogState,

    // sound thread
    pub sound_thread: Arc<Mutex<SoundThread>>,

    // Terminal thread communication
    terminal_tx: mpsc::UnboundedSender<TerminalCommand>,
    terminal_rx: Option<mpsc::UnboundedReceiver<TerminalEvent>>,

    // Connection state
    is_connected: bool,
    connection_time: Option<Instant>,
    current_address: Option<Address>,

    // Capture state
    capture_file: Option<PathBuf>,
    captured_data: Vec<u8>,

    screen_mode: ScreenMode,
    is_fullscreen_mode: bool,
    last_pos: Position,
    shift_pressed_during_selection: bool,
    use_rip: bool,

    pub initial_upload_directory: Option<PathBuf>,
    pub show_find_dialog: bool,
    show_disconnect: bool,
    title: String,
}

impl MainWindow {
    pub fn new() -> Self {
        let options = match Options::load_options() {
            Ok(options) => options,
            Err(e) => {
                log::error!("Error reading dialing_directory: {e}");
                Options::default()
            }
        };

        let addresses = AddressBook::load_phone_book().unwrap();

        let default_capture_path = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("capture.ans");

        let default_export_path = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("export.icy");

        // Create shared edit state for terminal
        let sound_thread = Arc::new(Mutex::new(SoundThread::new()));
        let terminal_window = terminal_window::TerminalWindow::new(sound_thread.clone());
        let edit_state = terminal_window.scene.edit_state.clone();

        // Create terminal thread
        let (terminal_tx, terminal_rx) = create_terminal_thread(edit_state.clone(), icy_net::telnet::TerminalEmulation::Ansi);

        Self {
            state: MainWindowState {
                mode: MainWindowMode::ShowTerminal,
                #[cfg(test)]
                options_written: false,
            },
            dialing_directory: dialing_directory_dialog::DialingDirectoryState::new(addresses),
            settings_dialog: settings_dialog::SettingsDialogState::new(options),
            capture_dialog: capture_dialog::CaptureDialogState::new(default_capture_path.to_string_lossy().to_string()),
            terminal_window,
            iemsi_dialog: show_iemsi::ShowIemsiDialog::new(None),
            find_dialog: find_dialog::DialogState::new(),
            export_dialog: export_dialog::ExportDialogState::new(default_export_path.to_string_lossy().to_string()),
            file_transfer_dialog: FileTransferDialogState::new(),

            terminal_tx,
            terminal_rx: Some(terminal_rx),

            is_connected: false,
            connection_time: None,
            current_address: None,

            capture_file: None,
            captured_data: Vec::new(),

            screen_mode: ScreenMode::Default,
            is_fullscreen_mode: false,
            last_pos: Position::default(),
            shift_pressed_during_selection: false,
            use_rip: false,
            initial_upload_directory: None,
            show_find_dialog: false,
            show_disconnect: false,
            title: crate::DEFAULT_TITLE.to_string(),
            sound_thread,
        }
    }

    pub fn title(&self) -> String {
        if self.is_connected {
            if let Some(connection_time) = self.connection_time {
                let d = Instant::now().duration_since(connection_time);
                let sec = d.as_secs();
                let minutes = sec / 60;
                let hours = minutes / 60;
                let connection_time_str = format!("{:02}:{:02}:{:02}", hours, minutes % 60, sec % 60);

                let system_name = self
                    .current_address
                    .as_ref()
                    .map(|addr| {
                        if addr.system_name.is_empty() {
                            addr.address.clone()
                        } else {
                            addr.system_name.clone()
                        }
                    })
                    .unwrap_or_else(|| "Unknown".to_string());

                fl!(
                    crate::LANGUAGE_LOADER,
                    "title-connected",
                    version = crate::VERSION.to_string(),
                    time = connection_time_str,
                    name = system_name
                )
            } else {
                crate::DEFAULT_TITLE.to_string()
            }
        } else {
            fl!(crate::LANGUAGE_LOADER, "title-offline", version = crate::VERSION.to_string())
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Process any pending terminal events
        if let Some(rx) = &mut self.terminal_rx {
            while let Ok(event) = rx.try_recv() {
                // Clone the event to avoid borrow issues
                let task = self.handle_terminal_event(event);
                //   if !task.is_none() {
                return task;
                //}
            }
        }

        match message {
            Message::DialingDirectory(msg) => self.dialing_directory.update(msg),

            Message::Connect(address) => {
                // Send connect command to terminal thread
                let config = ConnectionConfig {
                    connection_type: icy_net::ConnectionType::from(address.protocol.clone()),
                    address: address.address.clone(),
                    terminal_type: address.terminal_type,
                    window_size: (80, 25),
                    timeout: web_time::Duration::from_secs(30),
                    user_name: opt_non_empty(&address.user_name),
                    password: opt_non_empty(&address.password),
                    proxy_command: None, // fill from settings if needed
                    modem: if matches!(address.protocol, ConnectionType::Modem) {
                        Some(ModemConfig {
                            device: "/dev/ttyUSB0".into(),
                            baud_rate: 57600,
                            char_size: 8,
                            parity: icy_net::serial::Parity::None,
                            stop_bits: icy_net::serial::StopBits::One,
                            flow_control: icy_net::serial::FlowControl::XonXoff,
                            init_string: vec!["ATZ".into(), "ATE1".into()],
                            dial_string: "ATDT".to_string(),
                        })
                    } else {
                        None
                    },
                    music_option: address.ansi_music,
                    screen_mode: address.get_screen_mode(),
                    auto_login: self.settings_dialog.original_options.iemsi.autologin,
                    login_exp: address.auto_login.clone(),
                };

                let screen_mode = address.get_screen_mode();
                screen_mode.apply_to_edit_state(&mut self.terminal_window.scene.edit_state.lock().unwrap());

                let _ = self.terminal_tx.send(TerminalCommand::Connect(config));
                self.terminal_window.connect(Some(address.clone()));

                self.current_address = Some(address);
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }

            Message::Disconnect => {
                let _ = self.terminal_tx.send(TerminalCommand::Disconnect);
                self.terminal_window.disconnect();
                Task::none()
            }

            Message::SendData(data) => {
                println!("Sending data to terminal thread: {:?}", data);
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
                // Get IEMSI info from terminal if available
                let iemsi_info = Some(icy_net::iemsi::EmsiISI {
                    name: "Example BBS".to_string(),
                    location: "Nowhere".to_string(),
                    operator: "Operator".to_string(),
                    notice: "Welcome to Example BBS".to_string(),
                    capabilities: "Capabilities".to_string(),
                    id: "123456".to_string(),
                    localtime: "time".to_string(),
                    wait: "wait".to_string(),
                });
                self.iemsi_dialog = show_iemsi::ShowIemsiDialog::new(iemsi_info);
                self.state.mode = MainWindowMode::ShowIEMSI;
                Task::none()
            }

            Message::SettingsDialog(msg) => {
                if let Some(close_msg) = self.settings_dialog.update(msg) {
                    let c = self.update(close_msg);
                    return c;
                }
                Task::none()
            }

            Message::CloseDialog => {
                self.state.mode = MainWindowMode::ShowTerminal;
                Task::none()
            }

            Message::ShowDialingDirectory => {
                self.state.mode = MainWindowMode::ShowDialingDirectory;
                Task::none()
            }

            Message::Upload => {
                if self.is_connected {
                    self.state.mode = MainWindowMode::SelectProtocol(false);
                }
                Task::none()
            }

            Message::Download => {
                if self.is_connected {
                    self.state.mode = MainWindowMode::SelectProtocol(true);
                }
                Task::none()
            }

            Message::SendLogin => {
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
                                let username_data = address.user_name.as_bytes().to_vec();
                                let mut username_with_cr = username_data;
                                username_with_cr.push(b'\r');
                                let _ = self.terminal_tx.send(TerminalCommand::SendData(username_with_cr));

                                // Send password after a small delay
                                // Note: In a real implementation, you might want to add a proper delay mechanism
                                // For now, we'll send it immediately and rely on the terminal's buffering
                                std::thread::sleep(std::time::Duration::from_millis(500));

                                let password_data = address.password.as_bytes().to_vec();
                                let mut password_with_cr = password_data;
                                password_with_cr.push(b'\r');
                                let _ = self.terminal_tx.send(TerminalCommand::SendData(password_with_cr));
                            } else if !address.user_name.is_empty() {
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
                        Message::CloseDialog => {
                            self.state.mode = MainWindowMode::ShowTerminal;
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
                if is_download {
                    let _ = self.terminal_tx.send(TerminalCommand::StartDownload(protocol, None));
                } else {
                    let files = rfd::FileDialog::new()
                        .set_title("Select Files to Upload")
                        .set_directory(self.initial_upload_directory.as_ref().and_then(|p| p.to_str()).unwrap_or("."))
                        .pick_files();

                    if let Some(files) = files {
                        let _ = self.terminal_tx.send(TerminalCommand::StartUpload(protocol, files));
                    }
                }

                self.state.mode = MainWindowMode::FileTransfer(is_download);
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
                self.state.mode = MainWindowMode::ShowSettings;
                Task::none()
            }

            Message::ShowCaptureDialog => {
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
                self.state.mode = MainWindowMode::ShowFindDialog;
                return self.find_dialog.focus_search_input();
            }

            Message::FindDialog(msg) => {
                self.terminal_window.scene.cache.clear();
                if let Some(close_msg) = self.find_dialog.update(msg, self.terminal_window.scene.edit_state.clone()) {
                    return self.update(close_msg);
                }

                Task::none()
            }

            Message::ShowExportDialog => {
                self.state.mode = MainWindowMode::ShowExportDialog;
                Task::none()
            }
            Message::ExportDialog(msg) => {
                if let Some(response) = self.export_dialog.update(msg, self.terminal_window.scene.edit_state.clone()) {
                    match response {
                        Message::CloseDialog => {
                            self.state.mode = MainWindowMode::ShowTerminal;
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
        }
    }

    fn handle_terminal_event(&mut self, event: TerminalEvent) -> Task<Message> {
        match event {
            TerminalEvent::Connected => {
                self.is_connected = true;
                self.terminal_window.is_connected = true;
                self.connection_time = Some(Instant::now());
                self.show_disconnect = false;
                Task::none()
            }
            TerminalEvent::Disconnected(_error) => {
                self.is_connected = false;
                self.terminal_window.is_connected = false;
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
            TerminalEvent::BufferUpdated => {
                self.terminal_window.scene.cache.clear();
                Task::none()
            }
            TerminalEvent::TransferStarted(_state) => {
                self.state.mode = MainWindowMode::FileTransfer(true);
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
            TerminalEvent::AutoTransferTriggered(_, _, _) => {
                self.state.mode = MainWindowMode::FileTransfer(true);
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
            self.settings_dialog.temp_options.monitor_settings.get_theme()
        } else {
            self.settings_dialog.original_options.monitor_settings.get_theme()
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let settings = if self.get_mode() == MainWindowMode::ShowSettings {
            &self.settings_dialog.temp_options
        } else {
            &self.settings_dialog.original_options
        };

        match &self.state.mode {
            MainWindowMode::ShowTerminal => self.terminal_window.view(settings),
            MainWindowMode::ShowDialingDirectory => self.dialing_directory.view(&self.settings_dialog.original_options),
            MainWindowMode::ShowSettings => self.settings_dialog.view(self.terminal_window.view(settings)),
            MainWindowMode::SelectProtocol(download) => crate::ui::dialogs::protocol_selector::view_selector(*download, self.terminal_window.view(settings)),
            MainWindowMode::FileTransfer(download) => self.file_transfer_dialog.view(*download, self.terminal_window.view(settings)),
            MainWindowMode::ShowCaptureDialog => self.capture_dialog.view(self.terminal_window.view(settings)),
            MainWindowMode::ShowExportDialog => self.export_dialog.view(self.terminal_window.view(settings)),
            MainWindowMode::ShowIEMSI => self.iemsi_dialog.view(self.terminal_window.view(settings)),
            MainWindowMode::ShowFindDialog => find_dialog::find_dialog_overlay(&self.find_dialog, self.terminal_window.view(settings)),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        let keyboard_sub = if matches!(self.state.mode, MainWindowMode::ShowDialingDirectory) {
            iced::event::listen_with(|event, _status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::NavigateUp))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::NavigateDown))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::ConnectSelected))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        Some(Message::DialingDirectory(dialing_directory_dialog::DialingDirectoryMsg::Cancel))
                    }
                    _ => None,
                },
                _ => None,
            })
        } else if matches!(self.state.mode, MainWindowMode::SelectProtocol(_)) {
            iced::event::listen_with(|event, _status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CloseDialog),
                    _ => None,
                },
                _ => None,
            })
        } else if matches!(self.state.mode, MainWindowMode::ShowSettings) {
            iced::event::listen_with(|event, _status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::SettingsDialog(settings_dialog::SettingsMsg::Cancel)),
                    _ => None,
                },
                _ => None,
            })
        } else if matches!(self.state.mode, MainWindowMode::ShowCaptureDialog) {
            iced::event::listen_with(|event, _status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CaptureDialog(capture_dialog::CaptureMsg::Cancel)),
                    _ => None,
                },
                _ => None,
            })
        } else if matches!(self.state.mode, MainWindowMode::ShowIEMSI) {
            iced::event::listen_with(|event, _status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::ShowIemsi(show_iemsi::IemsiMsg::Close)),
                    _ => None,
                },
                _ => None,
            })
        } else if matches!(self.state.mode, MainWindowMode::ShowTerminal) {
            iced::event::listen_with(|event, _status, _| match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => {
                    if modifiers.alt() {
                        if let keyboard::Key::Character(s) = &key {
                            if s.to_lowercase() == "f" {
                                return Some(Message::ShowFindDialog);
                            }
                            if s.to_lowercase() == "e" {
                                return Some(Message::ShowExportDialog);
                            }
                        }
                    }

                    // Try to map the key with modifiers using the key map
                    if let Some(bytes) = Self::map_key_event_to_bytes(&key, modifiers) {
                        Some(Message::SendData(bytes))
                    } else if let Some(text) = text {
                        // If no special mapping, send the text as-is
                        Some(Message::SendData(text.as_bytes().to_vec()))
                    } else {
                        None
                    }
                }
                _ => None,
            })
        } else if matches!(self.state.mode, MainWindowMode::ShowFindDialog) {
            // Handle find dialog keyboard shortcuts
            iced::event::listen_with(|event, _status, _| match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::FindDialog(find_dialog::FindDialogMsg::CloseDialog)),
                    keyboard::Key::Named(keyboard::key::Named::PageUp) => Some(Message::FindDialog(find_dialog::FindDialogMsg::FindPrev)),
                    keyboard::Key::Named(keyboard::key::Named::PageDown) => Some(Message::FindDialog(find_dialog::FindDialogMsg::FindNext)),
                    keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::FindDialog(find_dialog::FindDialogMsg::FindNext)),
                    _ => None,
                },
                _ => None,
            })
        } else if matches!(self.state.mode, MainWindowMode::ShowExportDialog) {
            // Handle find dialog keyboard shortcuts
            iced::event::listen_with(|event, _status, _| match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers: _, .. }) => match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::ExportDialog(export_dialog::ExportMsg::Cancel)),
                    _ => None,
                },
                _ => None,
            })
        } else {
            iced::Subscription::none()
        };

        // Add a subscription for terminal events (polling)
        let terminal_sub = iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::TerminalEvent(TerminalEvent::BufferUpdated));

        iced::Subscription::batch([keyboard_sub, terminal_sub])
    }

    pub fn get_mode(&self) -> MainWindowMode {
        self.state.mode.clone()
    }

    fn map_key_event_to_bytes(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Vec<u8>> {
        // Get the appropriate key map based on terminal type
        // For now, we'll use ANSI as default, but this should be configurable
        let key_map = iced_engine_gui::key_map::ANSI_KEY_MAP;

        // Use the lookup_key function from the key_map module
        iced_engine_gui::key_map::lookup_key(key, modifiers, key_map)
    }
}

fn opt_non_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s.to_string()) }
}
