//! Save screen and capture destinations, replacing the bare native file chooser.

use std::path::{Path, PathBuf};

use eframe::egui;

use super::appearance;

/// Formats the terminal screen can be written to, in the order the picker shows them.
pub const FORMATS: [(&str, &str); 7] = [
    ("ANSI", "ans"),
    ("ASCII", "asc"),
    ("Avatar", "avt"),
    ("PCBoard", "pcb"),
    ("BIN", "bin"),
    ("XBin", "xb"),
    ("IcyDraw", "icy"),
];

fn default_directory(configured: &str) -> String {
    if !configured.trim().is_empty() {
        return configured.to_owned();
    }
    std::env::current_dir()
        .ok()
        .map_or_else(|| ".".to_owned(), |path| path.to_string_lossy().into_owned())
}

fn destination_rows(ui: &mut egui::Ui, directory: &mut String, file: &mut String) {
    appearance::form_row(ui, &tr!("settings-paths-download-dir"), |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("...").on_hover_text(tr!("egui-browse")).clicked() && !ui.ctx().will_discard() {
                if let Some(path) = rfd::FileDialog::new().set_directory(&*directory).pick_folder() {
                    *directory = path.to_string_lossy().into_owned();
                }
            }
            ui.add(appearance::text_edit(directory).desired_width(f32::INFINITY));
        });
    });
    appearance::form_row(ui, &tr!("egui-file"), |ui| {
        ui.add(appearance::text_edit(file).desired_width(f32::INFINITY));
    });
}

/// Replaces the extension so the name always matches the chosen format.
fn with_extension(file: &str, extension: &str) -> String {
    let stem = Path::new(file).file_stem().and_then(|stem| stem.to_str()).unwrap_or("screen");
    format!("{stem}.{extension}")
}

pub struct SaveScreen {
    format: usize,
    directory: String,
    file: String,
    confirm: bool,
}

impl SaveScreen {
    pub fn new(directory: &str) -> Self {
        Self {
            format: 0,
            directory: default_directory(directory),
            file: format!("screen.{}", FORMATS[0].1),
            confirm: false,
        }
    }

    fn target(&self) -> PathBuf {
        Path::new(&self.directory).join(&self.file)
    }

    /// Returns the destination and its extension once the user confirms.
    pub fn show(&mut self, context: &egui::Context, open: &mut bool) -> Option<(PathBuf, &'static str)> {
        let mut accepted = None;
        let response = appearance::Dialog::new("save-screen")
            .size(appearance::DialogSize::Medium)
            .confirm_on_enter(true)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, "", |ui| {
                        let (name, extension) = FORMATS[self.format];
                        appearance::combo_row(ui, &tr!("egui-format"), format!("{name} (.{extension})"), |ui| {
                            for (index, (name, extension)) in FORMATS.iter().enumerate() {
                                if ui.selectable_label(self.format == index, format!("{name} (.{extension})")).clicked() {
                                    self.format = index;
                                    self.file = with_extension(&self.file, extension);
                                }
                            }
                        });
                        destination_rows(ui, &mut self.directory, &mut self.file);
                    });
                    if self.confirm {
                        ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-overwrite-question"));
                    }
                });
                let enabled = !self.file.trim().is_empty();
                let save = if self.confirm {
                    appearance::DialogButton::destructive(tr!("egui-overwrite"), true)
                } else {
                    appearance::DialogButton::primary(tr!("egui-save"), true)
                };
                dialog.buttons([appearance::DialogButton::cancel(tr!("egui-cancel"), false), save.enabled(enabled)]);
            });
        match response.action {
            Some(true) if self.target().exists() && !self.confirm => self.confirm = true,
            Some(true) => accepted = Some((self.target(), FORMATS[self.format].1)),
            _ => {}
        }
        if response.action == Some(false) || response.dismissed || accepted.is_some() {
            *open = false;
        }
        accepted
    }
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
                        destination_rows(ui, &mut self.directory, &mut self.file);
                    });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choosing_a_format_rewrites_the_extension() {
        assert_eq!(with_extension("screen.ans", "asc"), "screen.asc");
        assert_eq!(with_extension("my.capture.ans", "xb"), "my.capture.xb");
        assert_eq!(with_extension("", "ans"), "screen.ans");
    }

    #[test]
    fn saving_asks_before_overwriting_an_existing_file() {
        let directory = std::env::temp_dir();
        let name = format!("icy-export-{}-{}.ans", std::process::id(), fastrand::u64(..));
        let path = directory.join(&name);
        std::fs::write(&path, b"old").unwrap();
        let mut dialog = SaveScreen::new(&directory.to_string_lossy());
        dialog.file = name;
        let context = egui::Context::default();
        let mut open = true;
        let click = |context: &egui::Context, dialog: &mut SaveScreen, open: &mut bool, label: &str| {
            let mut result = None;
            for _ in 0..2 {
                let output = context.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                        ..Default::default()
                    },
                    |context| {
                        result = dialog.show(context, open);
                    },
                );
                let position = output.shapes.iter().rev().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
                    _ => None,
                });
                if let Some(position) = position {
                    let _ = context.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                            events: vec![
                                egui::Event::PointerMoved(position),
                                egui::Event::PointerButton {
                                    pos: position,
                                    button: egui::PointerButton::Primary,
                                    pressed: true,
                                    modifiers: egui::Modifiers::NONE,
                                },
                            ],
                            ..Default::default()
                        },
                        |context| {
                            result = dialog.show(context, open);
                        },
                    );
                    let _ = context.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                            events: vec![egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed: false,
                                modifiers: egui::Modifiers::NONE,
                            }],
                            ..Default::default()
                        },
                        |context| {
                            result = dialog.show(context, open);
                        },
                    );
                }
                if result.is_some() {
                    break;
                }
            }
            result
        };
        assert!(
            click(&context, &mut dialog, &mut open, &tr!("egui-save")).is_none(),
            "an existing file must be confirmed first"
        );
        assert!(dialog.confirm);
        let accepted = click(&context, &mut dialog, &mut open, &tr!("egui-overwrite"));
        assert_eq!(accepted.map(|(path, _)| path), Some(path.clone()));
        std::fs::remove_file(path).unwrap();
    }
}
