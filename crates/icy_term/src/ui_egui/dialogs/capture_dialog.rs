use std::io::Write;

use eframe::egui::{self, RichText};
use egui::TextEdit;
use egui::{Frame, Layout};
use egui_file::FileDialog;
use i18n_embed_fl::fl;
use std::path::Path;

#[derive(Default)]
pub struct DialogState {
    open_file_dialog: Option<FileDialog>,
    pub capture_session: bool,
    pub capture_filename: String,

    /// debug spew prevention
    pub show_capture_error: bool,
}

pub enum Message {
    StartCapture,
    StopCapture,
    OpenFolder,
    CloseDialog,
    ChangeCaptureFileName(String),
}

impl DialogState {
    pub(crate) fn append_data(&mut self, data: &[u8]) {
        if self.capture_session {
            if let Ok(mut data_file) = std::fs::OpenOptions::new().create(true).append(true).open(&self.capture_filename) {
                if let Err(err) = data_file.write_all(data) {
                    if !self.show_capture_error {
                        self.show_capture_error = true;
                        log::error!("{err}");
                    }
                }
            }
        }
    }

    pub fn show_caputure_dialog(&mut self, ctx: &egui::Context) -> bool {
        let mut result = None;
        let mut open = true;
        let mut close_dialog = false;
        if ctx.input(|i: &egui::InputState| i.key_down(egui::Key::Escape)) {
            open = false;
        }
        let window_frame = Frame::window(&ctx.style());
        egui::Window::new(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-title"))
            .open(&mut open)
            .collapsible(true)
            .frame(window_frame)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-label")));

                ui.horizontal(|ui| {
                    let mut file = self.capture_filename.clone();
                    let r = ui.add(TextEdit::singleline(&mut file).desired_width(370.));
                    if r.changed() {
                        result = Some(Message::ChangeCaptureFileName(file));
                    }
                    if ui.button("…").clicked() {
                        result = Some(Message::OpenFolder);
                    }
                });
                ui.add_space(8.);
                ui.separator();
                ui.add_space(4.0);

                ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                    if self.capture_session {
                        if ui.button(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture")).clicked() {
                            result = Some(Message::StopCapture);
                            close_dialog = true;
                        }
                    } else if ui.button(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-button")).clicked() {
                        result = Some(Message::StartCapture);
                        close_dialog = true;
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(path) = Path::new(&self.capture_filename).parent() {
                        if ui.button(fl!(crate::LANGUAGE_LOADER, "capture-dialog-open-folder-button")).clicked() {
                            if let Some(s) = path.to_str() {
                                if let Err(err) = open::that(s) {
                                    log::error!("Failed to open folder: {}", err);
                                }
                            }
                        }
                    }

                    if ui.button(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")).clicked() {
                        close_dialog = true;
                    }
                });
            });

        if let Some(dialog) = &mut self.open_file_dialog {
            if dialog.show(ctx).selected() {
                if let Some(path) = dialog.path() {
                    if let Some(s) = path.to_str() {
                        result = Some(Message::ChangeCaptureFileName(s.to_string()));
                    }
                }
            }
        }

        if result.is_none() && (!open || close_dialog) {
            result = Some(Message::CloseDialog);
        }

        self.update_state(result) | open
    }

    fn update_state(&mut self, msg_opt: Option<Message>) -> bool {
        match msg_opt {
            Some(Message::OpenFolder) => {
                let initial_path = if self.capture_filename.is_empty() {
                    None
                } else {
                    Path::new(&self.capture_filename).parent().map(std::path::Path::to_path_buf)
                };
                let mut dialog: FileDialog = FileDialog::save_file(initial_path);
                dialog.open();
                self.open_file_dialog = Some(dialog);
            }
            Some(Message::StopCapture) => {
                self.capture_session = false;
                return false;
            }
            Some(Message::StartCapture) => {
                self.capture_session = true;
                return false;
            }
            Some(Message::CloseDialog) => {
                return false;
            }
            Some(Message::ChangeCaptureFileName(file)) => {
                self.capture_filename = file;
                self.show_capture_error = false;
                //  state.store_options();
            }
            _ => {}
        }
        true
    }
}
/*
#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use crate::ui::{dialogs::capture_dialog::update_state, MainWindowState};

    #[test]
    fn test_start_capture() {
        let mut state: MainWindowState = MainWindowState::default();
        assert!(!state.capture_dialog.capture_session);
        update_state(&mut state, Some(super::Message::StartCapture));
        assert!(state.capture_dialog.capture_session);
        assert!(!state.options_written);
    }

    #[test]
    fn test_stop_capture() {
        let mut state: MainWindowState = MainWindowState::default();
        state.capture_dialog.capture_session = true;
        update_state(&mut state, Some(super::Message::StopCapture));
        assert!(!state.capture_dialog.capture_session);
        assert!(!state.options_written);
    }

    #[test]
    fn test_close_dialog() {
        let mut state: MainWindowState = MainWindowState::default();
        state.mode = super::MainWindowMode::ShowCaptureDialog;
        update_state(&mut state, Some(super::Message::CloseDialog));
        assert!(matches!(state.mode, super::MainWindowMode::ShowTerminal));
        assert!(!state.options_written);
    }

    #[test]
    fn test_change_filename() {
        let mut state: MainWindowState = MainWindowState::default();
        update_state(&mut state, Some(super::Message::ChangeCaptureFileName("foo.baz".to_string())));
        assert_eq!("foo.baz".to_string(), state.options.capture_filename);
        assert!(state.options_written);
    }
}
*/
