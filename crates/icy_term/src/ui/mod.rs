#![allow(unsafe_code, clippy::wildcard_imports)]

use chrono::Utc;
use egui::Vec2;
use egui_bind::BindTarget;
use i18n_embed_fl::fl;
use icy_engine::{BufferParser, Caret, Position};
use icy_engine_gui::BufferView;
use icy_net::protocol::TransferProtocolType;
use icy_net::telnet::TerminalEmulation;
use std::mem;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use eframe::egui::Key;

use crate::features::AutoLogin;
use crate::ui::connect::OpenConnectionData;
use crate::{get_parser, get_unicode_converter, Options};

pub mod app;
pub mod com_thread;
pub mod connect;

pub mod terminal_window;

pub mod util;
pub use util::*;

use self::connect::SendData;
use self::terminal_thread::TerminalThread;

pub mod dialogs;

pub mod terminal_thread;

#[macro_export]
macro_rules! check_error {
    ($main_window: expr, $res: expr, $terminate_connection: expr) => {{
        /*   if let Err(err) = $res {
            log::error!("{err}");
            $main_window.output_string(format!("\n\r{err}\n\r").as_str());

            if $terminate_connection {
                if let Some(con) = $main_window.buffer_update_thread.lock().connection.lock().as_mut() {
                    con.disconnect().unwrap_or_default();
                }
            }
        }*/
    }};
}

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

#[derive(Default)]
pub struct MainWindowState {
    pub mode: MainWindowMode,
    pub options: Options,

    pub settings_dialog: dialogs::settings_dialog::DialogState,

    // don't store files in unit test mode
    #[cfg(test)]
    pub options_written: bool,
}

impl MainWindowState {
    #[cfg(test)]
    pub fn store_options(&mut self) {
        self.options_written = true;
    }

    #[cfg(not(test))]
    pub fn store_options(&mut self) {
        if let Err(err) = self.options.store_options() {
            log::error!("{err}");
        }
    }
}

pub struct MainWindow {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,

    pub state: MainWindowState,

    screen_mode: ScreenMode,
    is_fullscreen_mode: bool,
    drag_start: Option<Vec2>,
    last_pos: Position,
    shift_pressed_during_selection: bool,
    use_rip: bool,

    buffer_update_thread: Arc<egui::mutex::Mutex<TerminalThread>>,
    update_thread_handle: Option<JoinHandle<()>>,
    pub tx: mpsc::Sender<SendData>,
    pub rx: mpsc::Receiver<SendData>,

    pub initial_upload_directory: Option<PathBuf>,
    // protocols
    // pub current_file_transfer: Option<FileTransferThread>,
    pub dialing_directory_dialog: dialogs::dialing_directory_dialog::DialogState,
    pub export_dialog: dialogs::export_dialog::DialogState,
    pub upload_dialog: dialogs::upload_dialog::DialogState,

    pub show_find_dialog: bool,
    pub find_dialog: dialogs::find_dialog::DialogState,
    title: String,
    buffer_parser: Box<dyn BufferParser>,
    #[cfg(target_arch = "wasm32")]
    poll_thread: com_thread::ConnectionThreadData,
}

impl MainWindow {
    pub fn get_options(&self) -> &Options {
        &self.state.options
    }

    pub fn get_mode(&self) -> MainWindowMode {
        self.state.mode.clone()
    }

    pub fn set_mode(&mut self, ctx: &egui::Context, mode: MainWindowMode) {
        if self.state.mode == mode {
            return;
        }
        self.state.mode = mode;
        ctx.request_repaint()
    }

    pub fn println(&mut self, str: &str) {
        for ch in str.chars() {
            if ch as u32 > 255 {
                continue;
            }
            self.print_char(ch as u8);
        }
    }

    pub fn output_char(&mut self, ch: char) {
        let translated_char = self.buffer_view.lock().get_unicode_converter().convert_from_unicode(ch, 0);
        if self.buffer_update_thread.lock().is_connected {
            self.send_vec(vec![translated_char as u8]);
        } else {
            self.print_char(translated_char as u8);
        }
    }

    pub fn output_string(&mut self, str: &str) {
        if self.buffer_update_thread.lock().is_connected {
            let mut v = Vec::new();
            for ch in str.chars() {
                let translated_char = self.buffer_view.lock().get_unicode_converter().convert_from_unicode(ch, 0);
                v.push(translated_char as u8);
            }
            self.send_vec(v);
        } else {
            for ch in str.chars() {
                let translated_char = self.buffer_view.lock().get_unicode_converter().convert_from_unicode(ch, 0);
                self.print_char(translated_char as u8);
            }
        }
    }

    pub fn print_char(&mut self, c: u8) {
        let buffer_view = &mut self.buffer_view.lock();
        buffer_view.get_edit_state_mut().set_is_buffer_dirty();
        let mut caret = Caret::default();
        mem::swap(&mut caret, buffer_view.get_caret_mut());
        let _ = self.buffer_parser.print_char(buffer_view.get_buffer_mut(), 0, &mut caret, c as char);
        mem::swap(&mut caret, buffer_view.get_caret_mut());
    }

    #[cfg(target_arch = "wasm32")]

    fn start_file_transfer(&mut self, protocol_type: crate::protocol::TransferType, download: bool, files_opt: Option<Vec<FileDescriptor>>) {
        // TODO
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn upload(&mut self, ctx: &egui::Context, protocol_type: TransferProtocolType, files: Vec<PathBuf>) {
        self.set_mode(ctx, MainWindowMode::FileTransfer(false));
        self.send_data(SendData::Upload(protocol_type, files));

        //        check_error!(self, r, false);
    }

    fn download(&mut self, ctx: &egui::Context, protocol_type: TransferProtocolType) {
        self.set_mode(ctx, MainWindowMode::FileTransfer(true));
        self.send_data(SendData::Download(protocol_type));

        //let r = self.tx.send(SendData::Download(protocol_type));
        check_error!(self, r, false);
    }

    pub(crate) fn initiate_file_transfer(&mut self, ctx: &egui::Context, protocol_type: TransferProtocolType, download: bool) {
        self.set_mode(ctx, MainWindowMode::ShowTerminal);
        if download {
            self.download(ctx, protocol_type);
        } else {
            self.init_upload_dialog(ctx, protocol_type);
        }
    }

    pub fn set_screen_mode(&mut self, mode: ScreenMode) {
        self.screen_mode = mode;
        mode.set_mode(self);
    }

    pub fn call_bbs_uuid(&mut self, ctx: &egui::Context, uuid: Option<usize>) {
        if uuid.is_none() {
            self.call_bbs(ctx, 0);
            return;
        }

        let uuid = uuid.unwrap();
        for (i, adr) in self.dialing_directory_dialog.addresses.addresses.iter().enumerate() {
            if adr.id == uuid {
                self.call_bbs(ctx, i);
                return;
            }
        }
    }

    pub fn call_bbs(&mut self, ctx: &egui::Context, i: usize) {
        self.set_mode(ctx, MainWindowMode::ShowTerminal);
        let cloned_addr = self.dialing_directory_dialog.addresses.addresses[i].clone();

        {
            let address = &mut self.dialing_directory_dialog.addresses.addresses[i];
            let mut adr = address.address.clone();
            if !adr.contains(':') {
                adr.push_str(":23");
            }
            address.number_of_calls += 1;
            address.last_call = Some(Utc::now());

            let (user_name, password) = if address.override_iemsi_settings {
                (address.iemsi_user.clone(), address.iemsi_password.clone())
            } else {
                (address.user_name.clone(), address.password.clone())
            };

            self.buffer_update_thread.lock().auto_login = if user_name.is_empty() || password.is_empty() {
                None
            } else {
                Some(AutoLogin::new(&cloned_addr.auto_login, user_name, password))
            };

            if let Some(rip_cache) = address.get_rip_cache() {
                self.buffer_update_thread.lock().cache_directory = rip_cache;
            }

            self.use_rip = matches!(address.terminal_type, TerminalEmulation::Rip);
            self.buffer_update_thread.lock().terminal_type = Some((address.terminal_type, address.ansi_music));
            self.buffer_view.lock().clear_reference_image();
            self.buffer_view.lock().get_buffer_mut().layers[0].clear();
            self.buffer_view.lock().get_buffer_mut().stop_sixel_threads();
            self.dialing_directory_dialog.cur_addr = i;
            let converter = get_unicode_converter(&address.terminal_type);

            self.buffer_view.lock().set_unicode_converter(converter);
            self.buffer_view.lock().get_buffer_mut().terminal_state.set_baud_rate(address.baud_emulation);

            self.buffer_view.lock().redraw_font();
            self.buffer_view.lock().redraw_view();
            self.buffer_view.lock().clear();
        }
        self.set_screen_mode(cloned_addr.screen_mode);
        let _r = self.dialing_directory_dialog.addresses.store_phone_book();
        check_error!(self, r, false);

        self.println(&fl!(crate::LANGUAGE_LOADER, "connect-to", address = cloned_addr.address.clone()));

        let timeout = self.get_options().connect_timeout;
        let window_size = self.screen_mode.get_window_size();

        let data = OpenConnectionData::from(&cloned_addr, timeout, window_size, Some(self.get_options().modem.clone()));

        if let Some(_handle) = self.update_thread_handle.take() {
            self.send_data(SendData::Disconnect);
        }
        self.buffer_parser = get_parser(&data.term_caps.terminal, data.use_ansi_music, PathBuf::new());
        let (update_thread_handle, tx, rx) = crate::ui::terminal_thread::start_update_thread(ctx, data, self.buffer_update_thread.clone());
        self.update_thread_handle = Some(update_thread_handle);
        self.tx = tx;
        self.rx = rx;
        self.send_data(SendData::SetBaudRate(cloned_addr.baud_emulation.get_baud_rate()));
    }

    pub fn send_data(&mut self, data: SendData) {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let _res = self.tx.send(data).await;
        });
    }

    pub fn hangup(&mut self, ctx: &egui::Context) {
        self.send_data(SendData::Disconnect);
        self.update_thread_handle = None;
        self.buffer_update_thread.lock().sound_thread.lock().clear();
        self.set_mode(ctx, MainWindowMode::ShowDialingDirectory);
    }

    pub fn send_login(&mut self) {
        let user_name = self
            .dialing_directory_dialog
            .addresses
            .addresses
            .get(self.dialing_directory_dialog.cur_addr)
            .unwrap()
            .user_name
            .clone();
        let password = self
            .dialing_directory_dialog
            .addresses
            .addresses
            .get(self.dialing_directory_dialog.cur_addr)
            .unwrap()
            .password
            .clone();
        let mut cr: Vec<u8> = [self.buffer_view.lock().get_unicode_converter().convert_from_unicode('\r', 0) as u8].to_vec();
        for (k, v) in self.screen_mode.get_input_mode().cur_map() {
            if *k == Key::Enter as u32 {
                cr = v.to_vec();
                break;
            }
        }

        self.output_string(&user_name);
        self.send_vec(cr.clone());
        sleep(std::time::Duration::from_millis(350));
        self.output_string(&password);
        self.send_vec(cr);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn update_title(&mut self, ctx: &egui::Context) {
        let title = if let MainWindowMode::ShowDialingDirectory = self.get_mode() {
            crate::DEFAULT_TITLE.to_string()
        } else {
            let show_disconnect = false;
            let d = Instant::now().duration_since(self.buffer_update_thread.lock().connection_time);
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

            let title = if self.buffer_update_thread.lock().is_connected {
                fl!(
                    crate::LANGUAGE_LOADER,
                    "title-connected",
                    version = crate::VERSION.to_string(),
                    time = connection_time.clone(),
                    name = system_name.clone()
                )
            } else {
                fl!(crate::LANGUAGE_LOADER, "title-offline", version = crate::VERSION.to_string())
            };
            if show_disconnect {
                self.set_mode(ctx, MainWindowMode::ShowDisconnectedMessage(system_name.clone(), connection_time.clone()));
                self.output_string("\nNO CARRIER\n");
            }
            title
        };

        if self.title != title {
            self.title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }

    fn handle_terminal_key_binds(&mut self, ctx: &egui::Context) {
        if self.get_options().bind.clear_screen.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.buffer_view.lock().clear_buffer_screen();
        }
        if self.get_options().bind.dialing_directory.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(ctx, MainWindowMode::ShowDialingDirectory);
        }
        if self.get_options().bind.hangup.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.hangup(ctx);
        }
        if self.get_options().bind.send_login_pw.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.send_login();
        }
        if self.get_options().bind.show_settings.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(ctx, MainWindowMode::ShowSettings);
        }
        if self.get_options().bind.show_capture.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(ctx, MainWindowMode::ShowCaptureDialog);
        }
        if self.get_options().bind.quit.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            #[cfg(not(target_arch = "wasm32"))]
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.get_options().bind.full_screen.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.is_fullscreen_mode = !self.is_fullscreen_mode;
            #[cfg(not(target_arch = "wasm32"))]
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen_mode));
        }
        if self.get_options().bind.upload.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(ctx, MainWindowMode::SelectProtocol(false));
        }
        if self.get_options().bind.download.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(ctx, MainWindowMode::SelectProtocol(true));
        }

        if self.get_options().bind.show_find.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.show_find_dialog = true;
            let lock = &mut self.buffer_view.lock();
            let (buffer, _, parser) = lock.get_edit_state_mut().get_buffer_and_caret_mut();
            self.find_dialog.search_pattern(buffer, (*parser).as_ref());
            self.find_dialog.update_pattern(lock);
        }
    }

    fn send_vec(&mut self, to_vec: Vec<u8>) {
        if !self.buffer_update_thread.lock().is_connected {
            return;
        }
        self.send_data(SendData::Data(to_vec));

        /*
        if let Err(err) = self.tx.send(SendData::Data(to_vec)) {
            self.buffer_update_thread.lock().is_connected = false;
            log::error!("{err}");
            self.output_string(&format!("\n{err}"));
        }*/
    }
}

pub fn button_tint(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().widgets.active.fg_stroke.color
}
