use eframe::egui::{self, RichText};
use egui::{TextEdit, Vec2};
use i18n_embed_fl::fl;

use crate::ui::{MainWindow, MainWindowMode};

pub fn show_iemsi(window: &mut MainWindow, ctx: &egui::Context) {
    use egui::{Frame, Layout};

    let mut open = true;
    let mut close_dialog = false;
    if ctx.input(|i| i.key_down(egui::Key::Escape)) {
        open = false;
    }
    let window_frame = Frame::window(&ctx.style());
    let iemsi = window
        .buffer_update_thread
        .lock()
        .auto_login
        .as_ref()
        .unwrap()
        .iemsi
        .isi
        .as_ref()
        .unwrap()
        .clone();

    egui::Window::new("")
        .open(&mut open)
        .title_bar(false)
        .frame(window_frame)
        .fixed_size(Vec2::new(400., 300.))
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-heading")));
            ui.separator();

            egui::Grid::new("some_unique_id").num_columns(2).min_row_height(24.).show(ui, |ui| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-name")));
                });
                // passing the argument with .as_str() is necessary to make the TextEdit non editable
                ui.add(TextEdit::singleline(&mut iemsi.name.as_str()));
                ui.end_row();

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-location")));
                });
                ui.add(TextEdit::singleline(&mut iemsi.location.as_str()));
                ui.end_row();

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-operator")));
                });
                ui.add(TextEdit::singleline(&mut iemsi.operator.as_str()));
                ui.end_row();

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-notice")));
                });
                ui.add(TextEdit::singleline(&mut iemsi.notice.as_str()));
                ui.end_row();

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-capabilities")));
                });
                ui.add(TextEdit::singleline(&mut iemsi.capabilities.as_str()));
                ui.end_row();

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-id")));
                });
                ui.add(TextEdit::singleline(&mut iemsi.id.as_str()));
                ui.end_row();
            });

            ui.add_space(8.);
            ui.separator();
            ui.add_space(4.0);

            ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "dialing_directory-ok-button")).clicked() {
                    close_dialog = true;
                }
            });
        });

    if !open || close_dialog {
        window.set_mode(MainWindowMode::ShowTerminal);
    }
}
