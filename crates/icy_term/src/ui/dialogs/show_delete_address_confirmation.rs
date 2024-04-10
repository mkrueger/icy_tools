use eframe::egui::{self};
use egui_modal::Modal;
use i18n_embed_fl::fl;

use crate::ui::{MainWindow, MainWindowMode};

pub fn show_dialog(window: &mut MainWindow, ctx: &egui::Context, uuid: usize) {
    if ctx.input(|i| i.key_down(egui::Key::Escape)) {
        window.set_mode(MainWindowMode::ShowDialingDirectory);
    }
    let modal = Modal::new(ctx, "my_modal");
    modal.show(|ui| {
        modal.title(ui, fl!(crate::LANGUAGE_LOADER, "delete-bbs-title"));
        modal.frame(ui, |ui: &mut egui::Ui| {
            modal.body(
                ui,
                fl!(
                    crate::LANGUAGE_LOADER,
                    "delete-bbs-question",
                    system = window.dialing_directory_dialog.get_address_mut(Some(uuid)).system_name.clone()
                ),
            );
        });
        modal.buttons(ui, |ui| {
            if modal.button(ui, fl!(crate::LANGUAGE_LOADER, "delete-bbs-delete-button")).clicked() {
                window.dialing_directory_dialog.delete_bbs(uuid);
                window.set_mode(MainWindowMode::ShowDialingDirectory);
            }

            if modal.button(ui, fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")).clicked() {
                window.set_mode(MainWindowMode::ShowDialingDirectory);
            }
        });
    });

    modal.open();
}
