//! Capture destination, replacing the bare native file chooser. Saving the screen uses the
//! shared export dialog.

use std::path::{Path, PathBuf};

use eframe::egui;

use super::appearance;

fn default_directory(configured: &str) -> String {
    if !configured.trim().is_empty() {
        return configured.to_owned();
    }
    std::env::current_dir()
        .ok()
        .map_or_else(|| ".".to_owned(), |path| path.to_string_lossy().into_owned())
}

fn folder_row(ui: &mut egui::Ui, directory: &mut String) {
    appearance::form_row(ui, &tr!("egui-output-folder"), |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("...").on_hover_text(tr!("egui-browse")).clicked() && !ui.ctx().will_discard() {
                if let Some(path) = rfd::FileDialog::new().set_directory(&*directory).pick_folder() {
                    *directory = path.to_string_lossy().into_owned();
                }
            }
            ui.add(appearance::text_edit(directory).desired_width(f32::INFINITY));
        });
    });
}

fn file_row(ui: &mut egui::Ui, file: &mut String) {
    appearance::form_row(ui, &tr!("egui-file"), |ui| {
        ui.add(appearance::text_edit(file).desired_width(f32::INFINITY));
    });
}

/// Shows the full destination so the user sees exactly which file gets written.
fn target_preview(ui: &mut egui::Ui, target: &Path) {
    appearance::form_row(ui, "", |ui| {
        ui.add(
            egui::Label::new(egui::RichText::new(format!("\u{2192} {}", target.display())).weak().size(12.0))
                .wrap()
                .selectable(true),
        );
    });
}

pub struct Capture {
    directory: String,
    file: String,
    pub running: bool,
}

impl Capture {
    pub fn new(directory: &str, running: bool) -> Self {
        Self {
            directory: default_directory(directory),
            file: "capture.ans".to_owned(),
            running,
        }
    }

    /// Returns the destination when the capture should start.
    pub fn show(&mut self, context: &egui::Context, open: &mut bool) -> Option<PathBuf> {
        let mut started = None;
        let response = appearance::Dialog::new("capture").size(appearance::DialogSize::Medium).show(context, |dialog| {
            dialog.content(|ui| {
                if self.running {
                    ui.horizontal(|ui| {
                        appearance::status_badge(ui, &tr!("egui-recording"), ui.visuals().error_fg_color);
                    });
                    ui.add_space(8.0);
                }
                appearance::group(ui, "", |ui| {
                    ui.add_enabled_ui(!self.running, |ui| {
                        folder_row(ui, &mut self.directory);
                        file_row(ui, &mut self.file);
                    });
                    target_preview(ui, &Path::new(&self.directory).join(&self.file));
                });
            });
            let action = if self.running {
                appearance::DialogButton::primary(tr!("toolbar-stop-capture"), true)
            } else {
                appearance::DialogButton::primary(tr!("egui-capture-start"), true).enabled(!self.file.trim().is_empty())
            };
            dialog.buttons([appearance::DialogButton::cancel(tr!("egui-close"), false), action]);
        });
        if response.action == Some(true) && !self.running {
            started = Some(Path::new(&self.directory).join(&self.file));
        }
        if response.action.is_some() || response.dismissed {
            *open = false;
        }
        started
    }
}
