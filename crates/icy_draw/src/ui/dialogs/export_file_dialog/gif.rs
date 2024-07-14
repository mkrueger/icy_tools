use eframe::egui::Ui;
use icy_engine::SaveOptions;

pub fn create_settings_page(ui: &mut Ui, _options: &mut SaveOptions) {
    ui.label("Note: Blink is animated");
}
