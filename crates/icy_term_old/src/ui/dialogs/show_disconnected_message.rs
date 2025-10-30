use eframe::egui::{self, RichText};
use egui::{Align2, Vec2};
use i18n_embed_fl::fl;

use crate::ui::{MainWindow, MainWindowMode};

pub fn show_disconnected(window: &mut MainWindow, ctx: &egui::Context, system: String, time: String) {
    use egui::{Frame, Layout};

    let mut open = true;
    let mut close_dialog = false;
    if ctx.input(|i| i.key_down(egui::Key::Escape)) {
        open = false;
    }
    let window_frame = Frame::window(&ctx.style());

    egui::Window::new("")
        .open(&mut open)
        .title_bar(false)
        .frame(window_frame)
        .fixed_size(Vec2::new(400., 300.))
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "show-disconnected-heading")));
            ui.separator();
            ui.label(RichText::new(fl!(
                crate::LANGUAGE_LOADER,
                "show-disconnected-message",
                system = system,
                time = time
            )));

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
        window.set_mode(ctx, MainWindowMode::ShowTerminal);
    }
}
