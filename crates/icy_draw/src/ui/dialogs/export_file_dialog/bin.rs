use eframe::egui::Ui;
use icy_engine::SaveOptions;

use super::ascii;

pub fn create_settings_page(ui: &mut Ui, options: &mut SaveOptions) {
    ascii::create_settings_page(ui, options);
}
