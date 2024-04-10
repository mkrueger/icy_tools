use eframe::egui::{self, Ui};
use egui::Checkbox;
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

            egui::ComboBox::from_id_source("screen_prep_combo")
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
        ui.checkbox(&mut options.compress, fl!(crate::LANGUAGE_LOADER, "export-compression-label"));

        ui.add_enabled(
            options.compress,
            Checkbox::new(&mut options.use_repeat_sequences, fl!(crate::LANGUAGE_LOADER, "export-use_repeat_sequences")),
        );

        ui.checkbox(&mut options.preserve_line_length, fl!(crate::LANGUAGE_LOADER, "export-save_full_line_length"));

        ui.horizontal(|ui| {
            ui.add(egui::Checkbox::new(
                &mut options.modern_terminal_output,
                fl!(crate::LANGUAGE_LOADER, "export-utf8-output-label"),
            ));
        });

        ui.horizontal(|ui| {
            let mut use_max_lines = options.output_line_length.is_some();
            ui.add(egui::Checkbox::new(
                &mut use_max_lines,
                fl!(crate::LANGUAGE_LOADER, "export-limit-output-line-length-label"),
            ));
            if use_max_lines != options.output_line_length.is_some() {
                if use_max_lines {
                    options.output_line_length = Some(80);
                } else {
                    options.output_line_length = None;
                }
            }
            if let Some(mut len) = options.output_line_length {
                ui.add(egui::Slider::new(&mut len, 32..=255).text(fl!(crate::LANGUAGE_LOADER, "export-maximum_line_length")));
                options.output_line_length = Some(len);
            }
        });

        ui.horizontal(|ui: &mut Ui| {
            ui.add(egui::Checkbox::new(
                &mut options.save_sauce,
                fl!(crate::LANGUAGE_LOADER, "export-save-sauce-label"),
            ));
        });
    });
}
