use eframe::egui::Ui;
use icy_engine::AnsiSaveOptionsV2;

pub fn create_settings_page(ui: &mut Ui, _options: &mut AnsiSaveOptionsV2) {
    ui.label("Note: Blink is animated");
}
