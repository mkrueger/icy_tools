pub fn fields(ui: &mut eframe::egui::Ui, settings: &mut icy_engine_gui::MonitorSettings) {
    icy_engine_gui::egui::monitor::fields(ui, settings, |key| icy_term::LANGUAGE_LOADER.get(key));
}
