use eframe::egui::Ui;
use icy_engine::AnsiSaveOptionsV2;

use super::ascii;

pub fn create_settings_page(ui: &mut Ui, options: &mut AnsiSaveOptionsV2) {
    ascii::create_settings_page(ui, options);
}
