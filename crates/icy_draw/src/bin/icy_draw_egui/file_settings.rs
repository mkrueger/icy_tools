//! ANSI document formats: the templates of the New dialog, the File Settings dialog (canvas,
//! font height, format and display flags), the SAUCE dialog and the font slots of XBin Extended.

use super::{Dialog, DrawApp};
use eframe::egui;
use icy_draw::fl;
use icy_engine::{BitFont, FontMode, IceMode, Size, TextBuffer, TextPane};
use icy_engine_edit::FormatMode;
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};

/// ANSI document kinds offered by the New dialog, as in the original editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnsiTemplate {
    #[default]
    Modern,
    Dos,
    Ice,
    XBin,
    XBinExtended,
}

impl AnsiTemplate {
    pub const ALL: [AnsiTemplate; 5] = [
        AnsiTemplate::Modern,
        AnsiTemplate::Dos,
        AnsiTemplate::Ice,
        AnsiTemplate::XBin,
        AnsiTemplate::XBinExtended,
    ];

    pub fn title(self) -> String {
        match self {
            AnsiTemplate::Modern => fl!("new-file-template-ansi-title"),
            AnsiTemplate::Dos => fl!("new-file-template-cp437-title"),
            AnsiTemplate::Ice => fl!("new-file-template-ice-title"),
            AnsiTemplate::XBin => fl!("new-file-template-xb-title"),
            AnsiTemplate::XBinExtended => fl!("new-file-template-xb-ext-title"),
        }
    }

    pub fn description(self) -> String {
        match self {
            AnsiTemplate::Modern => fl!("new-file-template-ansi-description"),
            AnsiTemplate::Dos => fl!("new-file-template-cp437-description"),
            AnsiTemplate::Ice => fl!("new-file-template-ice-description"),
            AnsiTemplate::XBin => fl!("new-file-template-xb-description"),
            AnsiTemplate::XBinExtended => fl!("new-file-template-xb-ext-description"),
        }
    }

    pub fn apply(self, buffer: &mut TextBuffer) {
        let (ice_mode, font_mode) = match self {
            AnsiTemplate::Modern => (IceMode::Unlimited, FontMode::Unlimited),
            AnsiTemplate::Dos => (IceMode::Blink, FontMode::Sauce),
            AnsiTemplate::Ice => (IceMode::Ice, FontMode::Sauce),
            AnsiTemplate::XBin => (IceMode::Ice, FontMode::Single),
            AnsiTemplate::XBinExtended => (IceMode::Ice, FontMode::FixedSize),
        };
        buffer.ice_mode = ice_mode;
        buffer.font_mode = font_mode;
        if self == AnsiTemplate::XBinExtended && !buffer.has_font(1) {
            buffer.set_font(1, BitFont::default());
        }
    }
}

/// Short description of a format, shown next to the format selection.
pub fn format_description(format: FormatMode) -> String {
    match format {
        FormatMode::LegacyDos => fl!("file-settings-format-legacy-dos"),
        FormatMode::XBin => fl!("file-settings-format-xbin"),
        FormatMode::XBinExtended => fl!("file-settings-format-xbin-extended"),
        FormatMode::Unrestricted => fl!("file-settings-format-unrestricted"),
    }
}

/// Edited values of the File Settings dialog.
#[derive(Clone)]
pub struct FileSettingsDraft {
    size: [i32; 2],
    font_height: i32,
    format: FormatMode,
    ice_colors: bool,
    legacy_aspect: bool,
    letter_spacing: bool,
}

impl DrawApp {
    pub(super) fn open_file_settings(&mut self) {
        self.document.finish();
        let draft = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            FileSettingsDraft {
                size: [buffer.width(), buffer.height()],
                font_height: buffer.font_dimensions().height,
                format: state.get_format_mode(),
                ice_colors: buffer.ice_mode.has_high_bg_colors(),
                legacy_aspect: buffer.use_aspect_ratio(),
                letter_spacing: buffer.use_letter_spacing(),
            }
        });
        self.dialog = Some(Dialog::FileSettings(Box::new(draft)));
    }

    /// Shows the File Settings dialog; returns false once it is closed.
    pub(super) fn file_settings_dialog(&mut self, context: &egui::Context, draft: &mut FileSettingsDraft) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Apply,
        }
        let response = appearance::Dialog::new("file-settings")
            .title(fl!("file-settings-dialog-title"))
            .size(DialogSize::Medium)
            .max_height(720.0)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, "", |ui| {
                        appearance::form_row(ui, &fl!("file-settings-canvas-size"), |ui| {
                            ui.add(egui::DragValue::new(&mut draft.size[0]).range(1..=1000));
                            ui.label("×");
                            ui.add(egui::DragValue::new(&mut draft.size[1]).range(1..=10000));
                        });
                        appearance::form_row(ui, &fl!("file-settings-font-height"), |ui| {
                            ui.add(egui::DragValue::new(&mut draft.font_height).range(1..=32).suffix(" px"));
                        });
                        appearance::combo_row(ui, &fl!("file-settings-format"), draft.format.to_string(), |ui| {
                            for format in FormatMode::ALL {
                                ui.selectable_value(&mut draft.format, format, format.to_string())
                                    .on_hover_text(format_description(format));
                            }
                        });
                        appearance::form_row(ui, "", |ui| {
                            ui.weak(format_description(draft.format));
                        });
                    });
                    appearance::group(ui, &fl!("sauce-display"), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 20.0;
                            ui.checkbox(&mut draft.ice_colors, fl!("status-ice-colors"));
                            ui.checkbox(&mut draft.letter_spacing, fl!("file-settings-9px-font"));
                            ui.checkbox(&mut draft.legacy_aspect, fl!("file-settings-legacy-ar"));
                        });
                    });
                });
                dialog.buttons([
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(labels::ok(), Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Cancel) => false,
            Some(Action::Apply) => {
                self.apply_file_settings(draft);
                false
            }
            None => !response.dismissed,
        }
    }

    fn apply_file_settings(&mut self, draft: &FileSettingsDraft) {
        let draft = draft.clone();
        self.edit(|state| {
            let _undo = state.begin_atomic_undo(fl!("file-settings-dialog-title"));
            let size = Size::new(draft.size[0].max(1), draft.size[1].max(1));
            if state.get_buffer().size() != size {
                state.resize_buffer(false, size)?;
            }
            let font_size = state.get_buffer().font_dimensions();
            if font_size.height != draft.font_height {
                state.set_font_dimensions(Size::new(font_size.width, draft.font_height))?;
            }
            state.set_format_mode(draft.format);
            if draft.format == FormatMode::XBinExtended && !state.get_buffer().has_font(1) {
                state.set_font_in_slot(1, BitFont::default())?;
            }
            if state.get_buffer().ice_mode.has_high_bg_colors() != draft.ice_colors {
                state.set_ice_mode(if draft.ice_colors { IceMode::Ice } else { IceMode::Blink })?;
            }
            if state.get_buffer().use_aspect_ratio() != draft.legacy_aspect {
                state.set_use_aspect_ratio(draft.legacy_aspect)?;
            }
            if state.get_buffer().use_letter_spacing() != draft.letter_spacing {
                state.set_use_letter_spacing(draft.letter_spacing)?;
            }
            Ok(())
        });
    }

    /// Names of both font slots and the caret's slot when the document is XBin Extended.
    pub(super) fn font_slots(&self) -> Option<([String; 2], usize)> {
        self.document.with_state(|state| {
            (state.get_format_mode() == FormatMode::XBinExtended).then(|| {
                let buffer = state.get_buffer();
                let name = |slot| buffer.font(slot).map_or_else(|| fl!("status-unknown-font"), |font| font.name().to_string());
                ([name(0), name(1)], usize::from(state.get_caret().font_page().min(1)))
            })
        })
    }

    /// Makes `slot` the font of newly typed and painted characters; a double click also picks its font.
    pub(super) fn select_font_slot(&mut self, slot: usize, choose_font: bool) {
        self.edit(|state| state.switch_to_font_page(slot.min(1) as u8));
        if choose_font {
            self.open_font_selector();
        }
    }
}

/// SAUCE record limits: characters per title/author/group, characters per comment line and comment lines.
const SAUCE_TITLE: usize = 35;
const SAUCE_NAME: usize = 20;
const SAUCE_COMMENT_WIDTH: usize = 64;
const SAUCE_COMMENT_LINES: usize = 255;

/// Edited values of the SAUCE dialog: the record's texts and the display flags it stores.
#[derive(Clone)]
pub struct SauceDraft {
    title: String,
    author: String,
    group: String,
    comments: String,
    ice_colors: bool,
    letter_spacing: bool,
    legacy_aspect: bool,
    size: Size,
    font: String,
}

/// Comment lines as stored in the record: at most 255 lines of 64 characters.
fn sauce_comment_lines(comments: &str) -> Vec<String> {
    let mut lines: Vec<String> = comments
        .lines()
        .take(SAUCE_COMMENT_LINES)
        .map(|line| line.chars().take(SAUCE_COMMENT_WIDTH).collect())
        .collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
}

/// Label, edit field and "used / limit" counter of one SAUCE text row.
fn sauce_text_row(ui: &mut egui::Ui, label: &str, value: &mut String, limit: usize) {
    const LABEL_WIDTH: f32 = 72.0;
    const COUNTER_WIDTH: f32 = 44.0;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(LABEL_WIDTH, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(LABEL_WIDTH);
            ui.label(label);
        });
        let used = value.chars().count();
        let width = ui.available_width() - COUNTER_WIDTH - ui.spacing().item_spacing.x;
        ui.add(appearance::text_edit(value).char_limit(limit).desired_width(width));
        let color = if used >= limit {
            ui.visuals().warn_fg_color
        } else {
            ui.visuals().weak_text_color()
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{used}/{limit}")).monospace().size(11.0).color(color));
        });
    });
}

impl DrawApp {
    pub(super) fn open_sauce(&mut self) {
        self.document.finish();
        let draft = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let sauce = state.get_sauce_meta();
            SauceDraft {
                title: sauce.title.to_string(),
                author: sauce.author.to_string(),
                group: sauce.group.to_string(),
                comments: sauce.comments.iter().map(|line| line.to_string()).collect::<Vec<_>>().join("\n"),
                ice_colors: buffer.ice_mode.has_high_bg_colors(),
                letter_spacing: buffer.use_letter_spacing(),
                legacy_aspect: buffer.use_aspect_ratio(),
                size: buffer.size(),
                font: buffer.font(0).map(|font| font.name().to_string()).unwrap_or_default(),
            }
        });
        self.dialog = Some(Dialog::Sauce(Box::new(draft)));
    }

    /// Shows the SAUCE dialog; returns false once it is closed.
    pub(super) fn sauce_dialog(&mut self, context: &egui::Context, draft: &mut SauceDraft) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Apply,
        }
        let response = appearance::Dialog::new("sauce")
            .title(fl!("sauce-information"))
            .subtitle(fl!("sauce-subtitle"))
            .size(DialogSize::Width(560.0))
            .max_height(720.0)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, &fl!("sauce-record"), |ui| {
                        sauce_text_row(ui, &fl!("file-settings-title"), &mut draft.title, SAUCE_TITLE);
                        sauce_text_row(ui, &fl!("file-settings-author"), &mut draft.author, SAUCE_NAME);
                        sauce_text_row(ui, &fl!("file-settings-group"), &mut draft.group, SAUCE_NAME);
                    });
                    appearance::group(ui, &fl!("sauce-comments"), |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.comments)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(4),
                        );
                        let lines = draft.comments.lines().count();
                        let too_long = draft.comments.lines().position(|line| line.chars().count() > SAUCE_COMMENT_WIDTH);
                        ui.horizontal(|ui| {
                            match too_long {
                                Some(line) => ui.colored_label(
                                    ui.visuals().warn_fg_color,
                                    fl!("sauce-comment-too-long", line = (line + 1), limit = SAUCE_COMMENT_WIDTH),
                                ),
                                None => ui.weak(fl!("file-settings-comments-info")),
                            };
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(format!("{lines}/{SAUCE_COMMENT_LINES}"))
                                        .monospace()
                                        .size(11.0)
                                        .color(ui.visuals().weak_text_color()),
                                );
                            });
                        });
                    });
                    appearance::group(ui, &fl!("sauce-display"), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 20.0;
                            ui.checkbox(&mut draft.ice_colors, fl!("status-ice-colors"));
                            ui.checkbox(&mut draft.letter_spacing, fl!("file-settings-9px-font"));
                            ui.checkbox(&mut draft.legacy_aspect, fl!("file-settings-legacy-ar"));
                        });
                        let mut facts = format!("{} {} × {}", fl!("file-settings-canvas-size"), draft.size.width, draft.size.height);
                        if !draft.font.is_empty() {
                            facts.push_str(&format!("  ·  {} {}", fl!("glyph-font-label"), draft.font));
                        }
                        ui.weak(facts);
                    });
                });
                dialog.buttons([
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(labels::ok(), Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Cancel) => false,
            Some(Action::Apply) => {
                self.apply_sauce(draft);
                false
            }
            None => !response.dismissed,
        }
    }

    fn apply_sauce(&mut self, draft: &SauceDraft) {
        let draft = draft.clone();
        self.edit(|state| {
            let _undo = state.begin_atomic_undo(fl!("sauce-information"));
            state.update_sauce_data(icy_engine_edit::SauceMetaData {
                title: draft.title.as_str().into(),
                author: draft.author.as_str().into(),
                group: draft.group.as_str().into(),
                comments: sauce_comment_lines(&draft.comments).iter().map(|line| line.as_str().into()).collect(),
            })?;
            if state.get_buffer().ice_mode.has_high_bg_colors() != draft.ice_colors {
                state.set_ice_mode(if draft.ice_colors { IceMode::Ice } else { IceMode::Blink })?;
            }
            if state.get_buffer().use_letter_spacing() != draft.letter_spacing {
                state.set_use_letter_spacing(draft.letter_spacing)?;
            }
            if state.get_buffer().use_aspect_ratio() != draft.legacy_aspect {
                state.set_use_aspect_ratio(draft.legacy_aspect)?;
            }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::NewKind;
    use super::*;

    #[test]
    fn templates_create_the_matching_formats() {
        let mut app = DrawApp::new();
        for (template, format, ice) in [
            (AnsiTemplate::Modern, FormatMode::Unrestricted, IceMode::Unlimited),
            (AnsiTemplate::Dos, FormatMode::LegacyDos, IceMode::Blink),
            (AnsiTemplate::Ice, FormatMode::LegacyDos, IceMode::Ice),
            (AnsiTemplate::XBin, FormatMode::XBin, IceMode::Ice),
            (AnsiTemplate::XBinExtended, FormatMode::XBinExtended, IceMode::Ice),
        ] {
            app.new_template = template;
            app.create(NewKind::Ansi, Size::new(40, 10));
            let (actual, actual_ice, second_font) = app
                .document
                .with_state(|state| (state.get_format_mode(), state.get_buffer().ice_mode, state.get_buffer().has_font(1)));
            assert_eq!((actual, actual_ice), (format, ice), "{template:?}");
            assert_eq!(second_font, template == AnsiTemplate::XBinExtended, "{template:?}");
        }
    }

    #[test]
    fn file_settings_apply_as_one_undo_step() {
        let mut app = DrawApp::new();
        app.open_file_settings();
        let Some(Dialog::FileSettings(mut draft)) = app.dialog.take() else {
            panic!("file settings dialog not opened");
        };
        draft.size = [60, 20];
        draft.format = FormatMode::XBinExtended;
        draft.ice_colors = true;
        draft.letter_spacing = true;
        app.apply_file_settings(&draft);
        app.document.with_state(|state| {
            assert_eq!(state.get_buffer().size(), Size::new(60, 20));
            assert_eq!(state.get_format_mode(), FormatMode::XBinExtended);
            assert!(state.get_buffer().has_font(1));
            assert!(state.get_buffer().ice_mode.has_high_bg_colors());
            assert!(state.get_buffer().use_letter_spacing());
        });
        assert_eq!(app.font_slots().map(|(_, slot)| slot), Some(0));
        app.select_font_slot(1, false);
        assert_eq!(app.font_slots().map(|(_, slot)| slot), Some(1));
        app.document.type_text("A").unwrap();
        assert_eq!(
            app.document
                .with_state(|state| state.get_buffer().char_at(icy_engine::Position::new(0, 0)).attribute.font_page()),
            1,
            "typed characters use the selected font slot"
        );
        app.document.undo().unwrap();
        // Switching the font slot is an undo step of its own.
        app.document.undo().unwrap();
        assert_eq!(app.font_slots().map(|(_, slot)| slot), Some(0));
        app.document.undo().unwrap();
        assert_eq!(app.document.with_state(|state| state.get_buffer().size()), Size::new(80, 25));
    }

    #[test]
    fn sauce_dialog_stores_texts_flags_and_trimmed_comments() {
        let mut app = DrawApp::new();
        app.open_sauce();
        let Some(Dialog::Sauce(mut draft)) = app.dialog.take() else {
            panic!("SAUCE dialog not opened");
        };
        draft.title = "Art".into();
        draft.comments = format!("{}\nsecond\n\n", "x".repeat(80));
        draft.letter_spacing = true;
        draft.legacy_aspect = true;
        app.apply_sauce(&draft);
        app.document.with_state(|state| {
            let sauce = state.get_sauce_meta();
            assert_eq!(sauce.title.to_string(), "Art");
            let comments: Vec<_> = sauce.comments.iter().map(|line| line.to_string()).collect();
            assert_eq!(comments, ["x".repeat(64), "second".to_owned()]);
            assert!(state.get_buffer().use_letter_spacing() && state.get_buffer().use_aspect_ratio());
        });
    }
}
