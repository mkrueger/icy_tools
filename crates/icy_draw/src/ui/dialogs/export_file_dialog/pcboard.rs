use eframe::egui::Ui;
use icy_engine::SaveOptions;

use super::avatar;

pub fn create_settings_page(ui: &mut Ui, options: &mut SaveOptions) {
    avatar::create_settings_page(ui, options);
}
