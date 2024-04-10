#![allow(unsafe_code, clippy::wildcard_imports)]

use chrono::Utc;
use egui::Vec2;
use egui_bind::BindTarget;
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, Caret, Position};
use icy_engine_gui::BufferView;
use std::mem;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Instant;

use eframe::egui::Key;

use crate::features::AutoLogin;
use crate::ui::connect::DataConnection;
use crate::Options;
use crate::{protocol::FileDescriptor, TerminalResult};

pub mod app;
pub mod connect;

pub mod terminal_window;

pub mod util;
pub use util::*;

use self::buffer_update_thread::BufferUpdateThread;
use self::connect::Connection;
use self::file_transfer_thread::FileTransferThread;
pub mod dialogs;

pub mod com_thread;
pub mod file_transfer_thread;

pub mod buffer_update_thread;

#[macro_export]
macro_rules! check_error {
    ($main_window: expr, $res: expr, $terminate_connection: expr) => {{
        if let Err(err) = $res {
            log::error!("{err}");
            $main_window.output_string(format!("\n\r{err}\n\r").as_str());

            if $terminate_connection {
                if let Some(con) = $main_window.buffer_update_thread.lock().connection.lock().as_mut() {
                    con.disconnect().unwrap_or_default();
                }
            }
        }
    }};
}

#[derive(Clone, PartialEq, Eq, Default)]
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
    pub connection: Arc<eframe::epaint::mutex::Mutex<Option<Box<Connection>>>>,

    pub state: MainWindowState,

    screen_mode: ScreenMode,
    is_fullscreen_mode: bool,
    drag_start: Option<Vec2>,
    last_pos: Position,
    shift_pressed_during_selection: bool,
    is_disconnected: bool,
    use_rip: bool,

    buffer_update_thread: Arc<egui::mutex::Mutex<BufferUpdateThread>>,
    update_thread_handle: Option<JoinHandle<()>>,

    pub initial_upload_directory: Option<PathBuf>,
    // protocols
    pub current_file_transfer: Option<FileTransferThread>,

    pub dialing_directory_dialog: dialogs::dialing_directory_dialog::DialogState,
    pub export_dialog: dialogs::export_dialog::DialogState,
    pub upload_dialog: dialogs::upload_dialog::DialogState,

    pub show_find_dialog: bool,
    pub find_dialog: dialogs::find_dialog::DialogState,
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

    pub fn set_mode(&mut self, mode: MainWindowMode) {
        self.state.mode = mode;
    }

    pub fn println(&mut self, str: &str) {
        for ch in str.chars() {
            if ch as u32 > 255 {
                continue;
            }
            if let Err(err) = self.buffer_view.lock().print_char(ch) {
                log::error!("{err}");
            }
        }
    }

    pub fn output_char(&mut self, ch: char) {
        let translated_char = self.buffer_view.lock().get_unicode_converter().convert_from_unicode(ch, 0);
        let mut print = true;
        if let Some(con) = self.connection.lock().as_mut() {
            if con.is_connected() {
                let r = con.send(vec![translated_char as u8]);
                check_error!(self, r, false);
                print = false;
            }
        }

        if print {
            self.print_char(translated_char as u8);
        }
    }

    pub fn output_string(&self, str: &str) {
        let mut print = true;

        if let Some(con) = self.connection.lock().as_mut() {
            if con.is_connected() {
                let mut v = Vec::new();
                for ch in str.chars() {
                    let translated_char = self.buffer_view.lock().get_unicode_converter().convert_from_unicode(ch, 0);
                    v.push(translated_char as u8);
                }
                let r = con.send(v);
                check_error!(self, r, false);
                print = false;
            }
        }
        if print {
            for ch in str.chars() {
                let translated_char = self.buffer_view.lock().get_unicode_converter().convert_from_unicode(ch, 0);
                self.print_char(translated_char as u8);
            }
        }
    }

    pub fn print_char(&self, c: u8) {
        let buffer_view = &mut self.buffer_view.lock();
        buffer_view.get_edit_state_mut().set_is_buffer_dirty();
        let attribute = buffer_view.get_caret().get_attribute();
        let mut caret = Caret::default();
        mem::swap(&mut caret, buffer_view.get_caret_mut());
        buffer_view
            .get_buffer_mut()
            .print_char(0, &mut caret, AttributedChar::new(c as char, attribute));
        mem::swap(&mut caret, buffer_view.get_caret_mut());
    }

    #[cfg(target_arch = "wasm32")]

    fn start_file_transfer(&mut self, protocol_type: crate::protocol::TransferType, download: bool, files_opt: Option<Vec<FileDescriptor>>) {
        // TODO
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_file_transfer(&mut self, protocol_type: crate::protocol::TransferType, download: bool, files_opt: Option<Vec<FileDescriptor>>) {
        self.set_mode(MainWindowMode::FileTransfer(download));

        let r = crate::protocol::DiskStorageHandler::new();
        check_error!(self, r, false);
        if let Some(mut con) = self.connection.lock().take() {
            con.start_transfer();
            self.current_file_transfer = Some(FileTransferThread::new(con, protocol_type, download, files_opt));
        }
    }

    pub(crate) fn initiate_file_transfer(&mut self, protocol_type: crate::protocol::TransferType, download: bool) {
        self.set_mode(MainWindowMode::ShowTerminal);
        if let Some(con) = self.connection.lock().as_mut() {
            if con.is_disconnected() {
                return;
            }
        }

        if download {
            self.start_file_transfer(protocol_type, download, None);
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            self.init_upload_dialog(protocol_type);
        }
    }

    pub fn set_screen_mode(&mut self, mode: ScreenMode) {
        self.screen_mode = mode;
        mode.set_mode(self);
    }

    pub fn show_terminal(&mut self) {
        self.set_mode(MainWindowMode::ShowTerminal);
    }

    pub fn show_dialing_directory(&mut self) {
        self.set_mode(MainWindowMode::ShowDialingDirectory);
    }

    pub fn call_bbs_uuid(&mut self, uuid: Option<usize>) {
        if uuid.is_none() {
            self.call_bbs(0);
            return;
        }

        let uuid = uuid.unwrap();
        for (i, adr) in self.dialing_directory_dialog.addresses.addresses.iter().enumerate() {
            if adr.id == uuid {
                self.call_bbs(i);
                return;
            }
        }
    }

    pub fn call_bbs(&mut self, i: usize) {
        self.set_mode(MainWindowMode::ShowTerminal);
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

            self.use_rip = matches!(address.terminal_type, crate::Terminal::Rip);
            self.buffer_update_thread.lock().terminal_type = Some((address.terminal_type, address.ansi_music));
            self.buffer_update_thread.lock().auto_file_transfer.reset();
            self.buffer_view.lock().clear_reference_image();
            self.buffer_view.lock().get_buffer_mut().layers[0].clear();
            self.buffer_view.lock().get_buffer_mut().stop_sixel_threads();
            self.dialing_directory_dialog.cur_addr = i;
            let converter = address.terminal_type.get_unicode_converter();

            self.buffer_view.lock().set_unicode_converter(converter);
            self.buffer_view.lock().get_buffer_mut().terminal_state.set_baud_rate(address.baud_emulation);

            self.buffer_view.lock().redraw_font();
            self.buffer_view.lock().redraw_view();
            self.buffer_view.lock().clear();
        }
        self.set_screen_mode(cloned_addr.screen_mode);
        let r = self.dialing_directory_dialog.addresses.store_phone_book();
        check_error!(self, r, false);

        self.println(&fl!(crate::LANGUAGE_LOADER, "connect-to", address = cloned_addr.address.clone()));

        let timeout = self.get_options().connect_timeout;
        let window_size = self.screen_mode.get_window_size();
        if let Some(con) = self.connection.lock().as_mut() {
            let r = con.connect(&cloned_addr, timeout, window_size, Some(self.get_options().modem.clone()));
            check_error!(self, r, false);
            let r = con.set_baud_rate(cloned_addr.baud_emulation.get_baud_rate());
            check_error!(self, r, false);
        }
    }

    pub fn update_state(&mut self, ctx: &egui::Context) -> TerminalResult<()> {
        #[cfg(target_arch = "wasm32")]
        self.poll_thread.poll();
        if let Some(con) = self.connection.lock().as_mut() {
            con.update_state()?;
        }

        if self.update_thread_handle.as_ref().unwrap().is_finished() {
            if let Err(err) = &self.update_thread_handle.take().unwrap().join() {
                let msg = if let Some(msg) = err.downcast_ref::<&'static str>() {
                    (*msg).to_string()
                } else if let Some(msg) = err.downcast_ref::<String>() {
                    msg.clone()
                } else {
                    format!("?{err:?}")
                };
                log::error!("Error during update thread: {:?}", msg);
                self.update_thread_handle = Some(crate::ui::buffer_update_thread::run_update_thread(ctx, self.buffer_update_thread.clone()));
            }
        }

        let take = self.buffer_update_thread.lock().auto_transfer.take();
        if let Some((protocol_type, download)) = take {
            self.initiate_file_transfer(protocol_type, download);
        }
        Ok(())
    }

    pub fn hangup(&mut self) {
        if let Some(con) = self.connection.lock().as_mut() {
            check_error!(self, con.disconnect(), false);
        }
        self.buffer_update_thread.lock().sound_thread.lock().clear();
        self.set_mode(MainWindowMode::ShowDialingDirectory);
    }

    pub fn send_login(&mut self) {
        if let Some(con) = self.connection.lock().as_mut() {
            if con.is_disconnected() {
                return;
            }
        }
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
        if let Some(con) = self.connection.lock().as_mut() {
            let r = con.send(cr.clone());
            check_error!(self, r, false);
        }
        sleep(std::time::Duration::from_millis(350));
        self.output_string(&password);
        if let Some(con) = self.connection.lock().as_mut() {
            let r = con.send(cr);
            check_error!(self, r, false);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn update_title(&mut self, ctx: &egui::Context) {
        if let MainWindowMode::ShowDialingDirectory = self.get_mode() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(crate::DEFAULT_TITLE.to_string()));
        } else {
            if self.connection.lock().is_none() {
                return;
            }

            let mut show_disconnect = false;
            let mut connection_time = String::new();
            let mut system_name = String::new();
            if let Some(con) = self.connection.lock().as_mut() {
                let d = Instant::now().duration_since(con.get_connection_time());
                let sec = d.as_secs();
                let minutes = sec / 60;
                let hours = minutes / 60;
                let cur = &self.dialing_directory_dialog.addresses.addresses[self.dialing_directory_dialog.cur_addr];
                connection_time = format!("{:02}:{:02}:{:02}", hours, minutes % 60, sec % 60);
                system_name = if cur.system_name.is_empty() {
                    cur.address.clone()
                } else {
                    cur.system_name.clone()
                };

                let title = if con.is_connected() {
                    self.is_disconnected = false;
                    fl!(
                        crate::LANGUAGE_LOADER,
                        "title-connected",
                        version = crate::VERSION.to_string(),
                        time = connection_time.clone(),
                        name = system_name.clone()
                    )
                } else {
                    if self.is_disconnected {
                        return;
                    }
                    self.is_disconnected = true;
                    show_disconnect = true;
                    fl!(crate::LANGUAGE_LOADER, "title-offline", version = crate::VERSION.to_string())
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
            }
            if show_disconnect {
                self.set_mode(MainWindowMode::ShowDisconnectedMessage(system_name.clone(), connection_time.clone()));
                self.output_string("\nNO CARRIER\n");
            }
        }
    }

    fn handle_terminal_key_binds(&mut self, ctx: &egui::Context) {
        if self.get_options().bind.clear_screen.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.buffer_view.lock().clear_buffer_screen();
        }
        if self.get_options().bind.dialing_directory.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(MainWindowMode::ShowDialingDirectory);
        }
        if self.get_options().bind.hangup.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.hangup();
        }
        if self.get_options().bind.send_login_pw.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.send_login();
        }
        if self.get_options().bind.show_settings.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(MainWindowMode::ShowSettings);
        }
        if self.get_options().bind.show_capture.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(MainWindowMode::ShowCaptureDialog);
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
            self.set_mode(MainWindowMode::SelectProtocol(false));
        }
        if self.get_options().bind.download.pressed(ctx) {
            ctx.input_mut(|i| i.events.clear());
            self.set_mode(MainWindowMode::SelectProtocol(true));
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
}

pub fn button_tint(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().widgets.active.fg_stroke.color
}
