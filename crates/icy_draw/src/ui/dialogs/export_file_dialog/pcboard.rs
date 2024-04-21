use eframe::egui::Ui;
use i18n_embed_fl::fl;
use icy_engine::SaveOptions;

use super::avatar;

pub fn create_settings_page(ui: &mut Ui, options: &mut SaveOptions) {
    avatar::create_settings_page(ui, options);
    ui.horizontal(|ui| {
        ui.add(egui::Checkbox::new(
            &mut options.modern_terminal_output,
            fl!(crate::LANGUAGE_LOADER, "export-utf8-output-label"),
        ));
    });
}
