use eframe::egui;
use egui_modal::Modal;
use i18n_embed_fl::fl;

#[derive(Default)]
pub struct AboutDialog {
    pub create: bool,
}

impl crate::ModalDialog for AboutDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "help_dialog");

        modal.show(|ui| {
            ui.set_height(280.0);
            ui.set_width(600.0);

            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "about-dialog-title"));

            modal.frame(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(fl!(crate::LANGUAGE_LOADER, "about-dialog-heading"));
                });

                ui.label(fl!(crate::LANGUAGE_LOADER, "about-dialog-description"));
                ui.add_space(12.0); // ui.separator();
                ui.label(fl!(crate::LANGUAGE_LOADER, "about-dialog-created_by", authors = env!("CARGO_PKG_AUTHORS")));

                ui.add_space(8.0); // ui.separator();
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-ok")).clicked() {
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.create
    }
}
