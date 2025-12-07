use eframe::egui::{self, Ui};
use i18n_embed_fl::fl;
use icy_engine::{SaveOptions, ScreenPreperation};

pub fn create_settings_page(ui: &mut Ui, options: &mut SaveOptions) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(fl!(crate::LANGUAGE_LOADER, "export-video-preparation-label"));

            let label = match options.screen_preparation {
                ScreenPreperation::None => {
                    fl!(crate::LANGUAGE_LOADER, "export-video-preparation-None")
                }
                ScreenPreperation::ClearScreen => {
                    fl!(crate::LANGUAGE_LOADER, "export-video-preparation-Clear")
                }
                ScreenPreperation::Home => {
                    fl!(crate::LANGUAGE_LOADER, "export-video-preparation-Home")
                }
            };

            egui::ComboBox::from_id_salt("screen_prep_combo")
                .selected_text(label)
                .width(150.)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut options.screen_preparation,
                        ScreenPreperation::None,
                        fl!(crate::LANGUAGE_LOADER, "export-video-preparation-None"),
                    );
                    ui.selectable_value(
                        &mut options.screen_preparation,
                        ScreenPreperation::ClearScreen,
                        fl!(crate::LANGUAGE_LOADER, "export-video-preparation-Clear"),
                    );
                    ui.selectable_value(
                        &mut options.screen_preparation,
                        ScreenPreperation::Home,
                        fl!(crate::LANGUAGE_LOADER, "export-video-preparation-Home"),
                    );
                });
        });
        ui.add(egui::Checkbox::new(
            &mut options.save_sauce,
            fl!(crate::LANGUAGE_LOADER, "export-save-sauce-label"),
        ));
    });
}
