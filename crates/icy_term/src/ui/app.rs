#![allow(unsafe_code, clippy::wildcard_imports)]

use std::{path::PathBuf, sync::Arc, time::Duration};

use directories::UserDirs;
use eframe::egui::{self};
use egui::{mutex::Mutex, FontId};
use icy_engine::Position;
use icy_net::{
    protocol::TransferState,
    telnet::{TermCaps, TerminalEmulation},
    ConnectionType,
};
use web_time::Instant;

use crate::{
    features::AutoFileTransfer,
    get_unicode_converter,
    ui::{connect::OpenConnectionData, dialogs, terminal_thread::TerminalThread, BufferView, MainWindowState, ScreenMode},
    util::SoundThread,
    AddressBook, Options,
};

use super::{MainWindow, MainWindowMode};

impl MainWindow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        use egui::FontFamily::Proportional;
        use egui::TextStyle::{Body, Button, Heading, Monospace, Small};
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let gl = cc.gl.as_ref().expect("You need to run eframe with the glow backend");
        let options = match Options::load_options() {
            Ok(options) => options,
            Err(e) => {
                log::error!("Error reading dialing_directory: {e}");
                Options::default()
            }
        };

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

        // try to detect dark vs light mode from the host system; default to dark
        let is_dark = if let Some(dark_mode) = &options.is_dark_mode {
            *dark_mode
        } else {
            dark_light::detect() != dark_light::Mode::Light
        };
        let ctx: &egui::Context = &cc.egui_ctx;
        ctx.set_visuals(if is_dark { egui::Visuals::dark() } else { egui::Visuals::light() });

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
            buffer_update_thread,
            update_thread_handle: Some(update_thread_handle),
            tx,
            rx,
            show_find_dialog: false,
            find_dialog: dialogs::find_dialog::DialogState::default(),
            shift_pressed_during_selection: false,
            use_rip: false,
            buffer_parser,
        };

        #[cfg(not(target_arch = "wasm32"))]
        parse_command_line(&ctx, &mut view);

        let mut style: egui::Style = (*ctx.style()).clone();
        style.spacing.window_margin = egui::Margin::same(8.0);

        //        style.spacing.button_padding = Vec2::new(4., 2.);
        style.text_styles = [
            (Heading, FontId::new(24.0, Proportional)),
            (Body, FontId::new(18.0, Proportional)),
            (Monospace, FontId::new(18.0, egui::FontFamily::Monospace)),
            (Button, FontId::new(18.0, Proportional)),
            (Small, FontId::new(14.0, Proportional)),
        ]
        .into();
        ctx.set_style(style);

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
        match self.get_mode() {
            MainWindowMode::ShowTerminal => {
                self.handle_terminal_key_binds(ctx);
                self.update_terminal_window(ctx, frame, false);
                ctx.request_repaint_after(Duration::from_millis(150));
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
                let state = self.buffer_update_thread.lock().current_transfer.clone();
                // auto close uploads.
                if !download && state.is_finished {
                    self.set_mode(MainWindowMode::ShowTerminal);
                }
                match dialogs::up_download_dialog::FileTransferDialog::new().show_dialog(ctx, frame, &state, download) {
                    dialogs::up_download_dialog::FileTransferDialogAction::Run => {}
                    dialogs::up_download_dialog::FileTransferDialogAction::Close |
                    dialogs::up_download_dialog::FileTransferDialogAction::CancelTransfer => {
                        if state.is_finished {
                            self.set_mode(MainWindowMode::ShowTerminal);
                        } else {
                            self.send_data(super::connect::SendData::CancelTransfer);
                            self.set_mode(MainWindowMode::ShowTerminal);
                            ctx.request_repaint_after(Duration::from_millis(150));
                        }
                    }
                }
            }
            MainWindowMode::ShowCaptureDialog => {
                self.update_terminal_window(ctx, frame, false);
                if !self.buffer_update_thread.lock().capture_dialog.show_caputure_dialog(ctx) {
                    self.set_mode(MainWindowMode::ShowTerminal);
                }
                ctx.request_repaint_after(Duration::from_millis(150));
            }
            MainWindowMode::ShowExportDialog => {
                self.update_terminal_window(ctx, frame, false);
                self.show_export_dialog(ctx);
                ctx.request_repaint_after(Duration::from_millis(150));
            }
            MainWindowMode::ShowUploadDialog => {
                self.update_terminal_window(ctx, frame, false);
                self.show_upload_dialog(ctx);
                ctx.request_repaint_after(Duration::from_millis(150));
            }
            MainWindowMode::ShowIEMSI => {
                self.update_terminal_window(ctx, frame, false);
                dialogs::show_iemsi::show_iemsi(self, ctx);
                ctx.request_repaint_after(Duration::from_millis(150));
            } // MainWindowMode::AskDeleteEntry => todo!(),

            MainWindowMode::ShowDisconnectedMessage(time, system) => {
                self.update_terminal_window(ctx, frame, false);
                dialogs::show_disconnected_message::show_disconnected(self, ctx, time, system);
                ctx.request_repaint_after(Duration::from_millis(150));
            }
        }
    }

    /*  fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.buffer_view.lock().destroy(gl);
        }
    }*/
}
