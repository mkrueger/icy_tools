use std::{path::PathBuf, time::Instant};

use i18n_embed_fl::fl;
use iced::{Element, Task, Theme};
use icy_engine::{BufferParser, Position};

use crate::{ScreenMode, ui::MainWindowState};

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum MainWindowMode {
    ShowTerminal,
    #[default]
    ShowDialingDirectory,
    ///Shows settings - parameter: show dialing_directory
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

pub enum Message {}

#[derive(Default)]
pub struct MainWindow {
    //    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub state: MainWindowState,

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
        Task::none()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dracula.clone()
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.state.mode {
            MainWindowMode::ShowTerminal => todo!(),
            MainWindowMode::ShowDialingDirectory => todo!(),
            MainWindowMode::ShowSettings => todo!(),
            MainWindowMode::SelectProtocol(_) => todo!(),
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
        // Only subscribe to keyboard events when in memory editor mode
        iced::Subscription::none()
    }

    pub fn get_mode(&self) -> MainWindowMode {
        self.state.mode.clone()
    }
}
