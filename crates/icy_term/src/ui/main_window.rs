use std::{path::PathBuf, time::Instant};

use i18n_embed_fl::fl;
use iced::{Element, Task, Theme, keyboard};
use icy_engine::{BufferParser, Position};

use crate::{
    Address, AddressBook, Options, ScreenMode,
    ui::{MainWindowState, dialing_directory_dialog, settings_dialog, terminal_window},
};

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum MainWindowMode {
    ShowTerminal,
    #[default]
    ShowDialingDirectory,
    ShowSettings,
    SelectProtocol(bool),
    FileTransfer(bool),
    DeleteSelectedAddress(usize),
    ShowCaptureDialog,
    ShowExportDialog,
    ShowUploadDialog,
    ShowIEMSI,
    ShowDisconnectedMessage(String, String),
}

#[derive(Debug, Clone)]
pub enum Message {
    DialingDirectory(crate::ui::dialogs::dialing_directory_dialog::DialingDirectoryMsg),
    SettingsDialog(crate::ui::dialogs::settings_dialog::SettingsMsg),
    Connect(Address),
    CloseDialog,
    Disconnect,
    ShowDialingDirectory,
    ShowSettings,
    Upload,
    Download,
    InitiateFileTransfer {
        protocol: icy_net::protocol::TransferProtocolType,
        is_download: bool,
    },
    OpenReleaseLink,
}

pub struct MainWindow {
    //    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub state: MainWindowState,
    pub dialing_directory: dialing_directory_dialog::DialingDirectoryState,
    pub settings_dialog: settings_dialog::SettingsDialogState,
    pub terminal_window: terminal_window::TerminalWindow,

    screen_mode: ScreenMode,
    is_fullscreen_mode: bool,
    //drag_start: Option<Vec2>,
    last_pos: Position,
    shift_pressed_during_selection: bool,
    use_rip: bool,

    //    terminal_thread: Arc<egui::mutex::Mutex<TerminalThread>>,
    //     terminal_thread_handle: Option<JoinHandle<()>>,
    //    pub tx: mpsc::Sender<SendData>,
    //    pub rx: mpsc::Receiver<SendData>,
    pub initial_upload_directory: Option<PathBuf>,
    // protocols
    // pub current_file_transfer: Option<FileTransferThread>,
    //    pub dialing_directory_dialog: dialogs::dialing_directory_dialog::DialogState,
    //    pub export_dialog: dialogs::export_dialog::DialogState,
    //    pub upload_dialog: dialogs::upload_dialog::DialogState,
    //    pub find_dialog: dialogs::find_dialog::DialogState,
    pub show_find_dialog: bool,
    show_disconnect: bool,
    title: String,
    //    buffer_parser: Box<dyn BufferParser>,
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

        Self {
            state: MainWindowState {
                mode: MainWindowMode::ShowTerminal,
                #[cfg(test)]
                options_written: false,
            },
            dialing_directory: dialing_directory_dialog::DialingDirectoryState::new(addresses),
            settings_dialog: settings_dialog::SettingsDialogState::new(options),
            terminal_window: terminal_window::TerminalWindow::new(),
            screen_mode: ScreenMode::Default,
            is_fullscreen_mode: false,
            last_pos: Position::default(),
            shift_pressed_during_selection: false,
            use_rip: false,
            initial_upload_directory: None,
            show_find_dialog: false,
            show_disconnect: false,
            title: crate::DEFAULT_TITLE.to_string(),
        }
    }

    pub fn title(&self) -> String {
        //        if let MainWindowMode::ShowDialingDirectory = self.get_mode() {
        crate::DEFAULT_TITLE.to_string()
        /*        } else {
            let d = Instant::now().duration_since(self.terminal_thread.lock().connection_time);
            let sec = d.as_secs();
            let minutes = sec / 60;
            let hours = minutes / 60;
            let cur = &self.dialing_directory_dialog.addresses.addresses[self.dialing_directory_dialog.cur_addr];
            let connection_time = format!("{:02}:{:02}:{:02}", hours, minutes % 60, sec % 60);
            let system_name = if cur.system_name.is_empty() {
                cur.address.clone()
            } else {
                cur.system_name.clone()
            };

            let is_connected = self.terminal_thread.lock().is_connected;
            let title = if is_connected {
                self.show_disconnect = true;
                fl!(
                    crate::LANGUAGE_LOADER,
                    "title-connected",
                    version = crate::VERSION.to_string(),
                    time = connection_time.clone(),
                    name = system_name.clone()
                )
            } else {
                fl!(crate::LANGUAGE_LOADER, "title-offline", version = crate::VERSION.to_string())
            }
            title
        }*/
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DialingDirectory(msg) => self.dialing_directory.update(msg),
            Message::SettingsDialog(msg) => {
                if let Some(close_msg) = self.settings_dialog.update(msg) {
                    // Handle the close message
                    return self.update(close_msg);
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
                self.state.mode = MainWindowMode::SelectProtocol(false);
                Task::none()
            }
            Message::Download => {
                self.state.mode = MainWindowMode::SelectProtocol(true);
                Task::none()
            }

            Message::OpenReleaseLink => {
                // Open the GitHub releases page in the default browser
                if let Err(e) = webbrowser::open("https://github.com/mkrueger/icy_tools/releases") {
                    eprintln!("Failed to open release link: {}", e);
                }
                Task::none()
            }

            Message::ShowSettings => {
                self.state.mode = MainWindowMode::ShowSettings;
                Task::none()
            }

            _ => Task::none(),
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark.clone()
    }

    pub fn view(&self) -> Element<'_, Message> {
        println!("MainWindow::view mode={:?}", self.state.mode);
        match self.state.mode {
            MainWindowMode::ShowTerminal => self.terminal_window.view(),
            MainWindowMode::ShowDialingDirectory => self.dialing_directory.view(&self.settings_dialog.original_options),
            MainWindowMode::ShowSettings => self.settings_dialog.view(self.terminal_window.view()),
            MainWindowMode::SelectProtocol(download) => crate::ui::dialogs::protocol_selector::view_selector(download, self.terminal_window.view()),
            MainWindowMode::FileTransfer(_) => todo!(),
            MainWindowMode::DeleteSelectedAddress(_) => todo!(),
            MainWindowMode::ShowCaptureDialog => todo!(),
            MainWindowMode::ShowExportDialog => todo!(),
            MainWindowMode::ShowUploadDialog => todo!(),
            MainWindowMode::ShowIEMSI => todo!(),
            MainWindowMode::ShowDisconnectedMessage(_, _) => todo!(),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        // Subscribe to keyboard events when dialing directory is shown
        if matches!(self.state.mode, MainWindowMode::ShowDialingDirectory) {
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
        } else {
            iced::Subscription::none()
        }
    }

    pub fn get_mode(&self) -> MainWindowMode {
        self.state.mode.clone()
    }
}
