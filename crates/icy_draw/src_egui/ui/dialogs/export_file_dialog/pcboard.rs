use eframe::egui::Ui;
use i18n_embed_fl::fl;
use icy_engine::AnsiSaveOptionsV2;

use super::avatar;

pub fn create_settings_page(ui: &mut Ui, options: &mut AnsiSaveOptionsV2) {
    avatar::create_settings_page(ui, options);
    ui.horizontal(|ui| {
        ui.add(egui::Checkbox::new(
            &mut options.modern_terminal_output,
            fl!(crate::LANGUAGE_LOADER, "export-utf8-output-label"),
        ));
    });
}
