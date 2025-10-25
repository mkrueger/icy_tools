#![allow(unsafe_code, clippy::wildcard_imports)]

use std::{path::PathBuf, sync::Arc, time::Duration};

use directories::UserDirs;
use eframe::egui::{self};
use egui::{Rect, mutex::Mutex};
use icy_engine::Position;
use icy_net::{
    ConnectionType,
    protocol::TransferState,
    telnet::{TermCaps, TerminalEmulation},
};
use web_time::Instant;

use crate::{
    AddressBook, Options,
    features::AutoFileTransfer,
    get_unicode_converter,
    ui::{BufferView, MainWindowState, ScreenMode, connect::OpenConnectionData, dialogs, terminal_thread::TerminalThread},
    util::SoundThread,
};

use super::{MainWindow, MainWindowMode};

impl MainWindow {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let gl = cc.gl.as_ref().expect("You need to run eframe with the glow backend");
        let mut view = BufferView::new(gl);
        view.interactive = true;
        view.get_edit_state_mut().set_unicode_converter(get_unicode_converter(&TerminalEmulation::Ansi));

        let addresses: AddressBook = match crate::addresses::start_read_book() {
            Ok(addresses) => addresses,
            Err(e) => {
                log::error!("Error reading dialing_directory: {e}");
                AddressBook::default()
            }
        };

        //  #[cfg(not(target_arch = "wasm32"))]
        // let is_fullscreen_mode = cc.integration_info.window_info.fullscreen;
        //  #[cfg(target_arch = "wasm32")]
        let is_fullscreen_mode = false;

        let ctx: &egui::Context = &cc.egui_ctx;
        ctx.set_theme(options.get_theme());

        let mut initial_upload_directory = None;

        if let Some(dirs) = UserDirs::new() {
            initial_upload_directory = Some(dirs.home_dir().to_path_buf());
        }
        let buffer_update_view = Arc::new(eframe::epaint::mutex::Mutex::new(view));

        let buffer_update_thread = Arc::new(Mutex::new(TerminalThread {
            buffer_view: buffer_update_view.clone(),
            capture_dialog: dialogs::capture_dialog::DialogState::default(),
            auto_file_transfer: AutoFileTransfer::default(),
            auto_transfer: None,
            auto_login: None,
            sound_thread: Arc::new(eframe::epaint::mutex::Mutex::new(SoundThread::new())),
            terminal_type: None,
            mouse_field: Vec::new(),
            cache_directory: PathBuf::new(),
            is_connected: false,
            connection_time: Instant::now(),
            current_transfer: TransferState::new(String::new()),
        }));

        let data = OpenConnectionData {
            address: "".to_string(),
            user_name: "".to_string(),
            password: "".to_string(),
            connection_type: ConnectionType::Telnet,
            timeout: Duration::from_secs(1000),
            use_ansi_music: icy_engine::ansi::MusicOption::Off,
            baud_emulation: icy_engine::ansi::BaudEmulation::Off,
            proxy_command: None,
            term_caps: TermCaps {
                window_size: (0, 0),
                terminal: TerminalEmulation::Ascii,
            },
            modem: None,
            screen_mode: ScreenMode::default(),
        };

        let (update_thread_handle, tx, rx) = crate::ui::terminal_thread::start_update_thread(&cc.egui_ctx, data, buffer_update_thread.clone());
        let mut parser = icy_engine::ansi::Parser::default();
        parser.bs_is_ctrl_char = true;
        let buffer_parser = Box::new(parser);
        let mut view = MainWindow {
            buffer_view: buffer_update_view.clone(),
            //address_list: HoverList::new(),
            state: MainWindowState { options, ..Default::default() },
            initial_upload_directory,
            screen_mode: ScreenMode::default(),
            #[cfg(target_arch = "wasm32")]
            poll_thread,
            is_fullscreen_mode,
            export_dialog: dialogs::export_dialog::DialogState::default(),
            upload_dialog: dialogs::upload_dialog::DialogState::default(),
            dialing_directory_dialog: dialogs::dialing_directory_dialog::DialogState::new(addresses),
            drag_start: None,
            last_pos: Position::default(),
            terminal_thread: buffer_update_thread,
            terminal_thread_handle: Some(update_thread_handle),
            tx,
            rx,
            show_find_dialog: false,
            find_dialog: dialogs::find_dialog::DialogState::default(),
            shift_pressed_during_selection: false,
            use_rip: false,
            buffer_parser,
            title: String::new(),
            show_disconnect: false,
        };

        #[cfg(not(target_arch = "wasm32"))]
        parse_command_line(&ctx, &mut view);

        icy_engine_gui::set_icy_style(ctx);

        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
            o.zoom_factor = 1.0;
        });

        view
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_command_line(ctx: &egui::Context, view: &mut MainWindow) {
    let args: Vec<String> = std::env::args().collect();
    if let Some(arg) = args.get(1) {
        view.dialing_directory_dialog.addresses.addresses[0].address = arg.clone();
        view.call_bbs(ctx, 0);
    }
}

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        self.update_title(ctx);

        ctx.input(|i| {
            if let Some(or) = i.viewport().outer_rect {
                if let Some(ir) = i.viewport().inner_rect {
                    let rect = Rect {
                        min: or.min,
                        max: (ir.max - ir.min).to_pos2(),
                    };
                    if self.state.options.window_rect != Some(rect) {
                        self.state.options.window_rect = Some(rect);
                        self.state.store_options();
                    }
                }
            }
        });

        match self.get_mode() {
            MainWindowMode::ShowTerminal => {
                self.handle_terminal_key_binds(ctx);
                self.update_terminal_window(ctx, frame, false);
            }
            MainWindowMode::ShowDialingDirectory => {
                self.update_terminal_window(ctx, frame, true);
            }
            MainWindowMode::ShowSettings => {
                self.update_terminal_window(ctx, frame, false);
                self.state.show_settings(ctx, frame);
            }
            MainWindowMode::DeleteSelectedAddress(uuid) => {
                self.update_terminal_window(ctx, frame, true);
                super::dialogs::show_delete_address_confirmation::show_dialog(self, ctx, uuid);
            }

            MainWindowMode::SelectProtocol(download) => {
                self.update_terminal_window(ctx, frame, false);
                dialogs::protocol_selector::view_selector(self, ctx, frame, download);
            }

            MainWindowMode::FileTransfer(download) => {
                self.update_terminal_window(ctx, frame, false);
                let state = self.terminal_thread.lock().current_transfer.clone();
                // auto close uploads.
                if !download && state.is_finished {
                    self.set_mode(ctx, MainWindowMode::ShowTerminal);
                }
                match dialogs::up_download_dialog::FileTransferDialog::new().show_dialog(ctx, frame, &state, download) {
                    dialogs::up_download_dialog::FileTransferDialogAction::Run => {}
                    dialogs::up_download_dialog::FileTransferDialogAction::Close | dialogs::up_download_dialog::FileTransferDialogAction::CancelTransfer => {
                        if state.is_finished {
                            self.set_mode(ctx, MainWindowMode::ShowTerminal);
                        } else {
                            self.send_data(super::connect::SendData::CancelTransfer);
                            self.set_mode(ctx, MainWindowMode::ShowTerminal);
                        }
                    }
                }
            }
            MainWindowMode::ShowCaptureDialog => {
                self.update_terminal_window(ctx, frame, false);
                if !self.terminal_thread.lock().capture_dialog.show_caputure_dialog(ctx) {
                    self.set_mode(ctx, MainWindowMode::ShowTerminal);
                }
            }
            MainWindowMode::ShowExportDialog => {
                self.update_terminal_window(ctx, frame, false);
                self.show_export_dialog(ctx);
            }
            MainWindowMode::ShowUploadDialog => {
                self.update_terminal_window(ctx, frame, false);
                self.show_upload_dialog(ctx);
            }
            MainWindowMode::ShowIEMSI => {
                self.update_terminal_window(ctx, frame, false);
                dialogs::show_iemsi::show_iemsi(self, ctx);
            } // MainWindowMode::AskDeleteEntry => todo!(),

            MainWindowMode::ShowDisconnectedMessage(time, system) => {
                self.update_terminal_window(ctx, frame, false);
                dialogs::show_disconnected_message::show_disconnected(self, ctx, time, system);
            }
        }
    }

    /*  fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.buffer_view.lock().destroy(gl);
        }
    }*/
}
