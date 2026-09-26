//! The tagline list: pick the tagline of the message being written, or manage the collection.

use eframe::egui::{self, Key};
use i18n_embed_fl::fl;
use icy_engine_gui::egui::appearance;
use icy_mail::{
    taglines::{Taglines, TAGLINE_LENGTH},
    LANGUAGE_LOADER,
};

use super::{
    app::{MailApp, Modal},
    widgets::{self, Icon},
};

const ROW_HEIGHT: f32 = 28.0;
const FILTER_ID: &str = "tagline-filter";
const EDIT_ID: &str = "tagline-edit";

pub struct TaglineDialog {
    /// Opened from the composer to choose the message's tagline.
    pick: bool,
    filter: String,
    /// Index into the tagline list.
    selected: Option<usize>,
    /// Tagline being written: `None` adds a new one, otherwise the index being changed.
    edit: Option<(Option<usize>, String)>,
    focus: Option<&'static str>,
    reveal: bool,
    error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    Use,
    Random,
    NoTagline,
    Close,
    New,
    Edit,
    Delete,
    Save,
    CancelEdit,
}

impl MailApp {
    pub fn tagline_file(&self) -> icy_mail::Res<Taglines> {
        match &self.storage {
            Some(directory) => Taglines::open_in(directory),
            None => Taglines::open(),
        }
    }

    /// Opens the tagline list, re-read so changes from other windows show up.
    pub fn open_taglines(&mut self, pick: bool) {
        match self.tagline_file() {
            Ok(taglines) => self.taglines = Some(taglines),
            Err(error) => {
                self.error = Some(fl!(LANGUAGE_LOADER, "taglines-read-error", error = error.to_string()));
                return;
            }
        }
        let lines = self.taglines.as_ref().map(|taglines| taglines.lines.as_slice()).unwrap_or_default();
        let current = self.composer.as_ref().filter(|_| pick).map(|composer| composer.draft.tagline.as_str());
        let selected = current
            .and_then(|current| lines.iter().position(|line| line == current))
            .or((!lines.is_empty()).then_some(0));
        self.tagline_dialog = Some(TaglineDialog {
            pick,
            filter: String::new(),
            selected,
            edit: None,
            focus: Some(FILTER_ID),
            reveal: true,
            error: None,
        });
        self.modal = Some(Modal::Taglines);
    }

    /// Shows the tagline list; returns whether it closed.
    pub fn taglines_dialog(&mut self, context: &egui::Context) -> bool {
        let (Some(dialog), Some(taglines)) = (&mut self.tagline_dialog, &mut self.taglines) else {
            self.tagline_dialog = None;
            return true;
        };
        let filter = dialog.filter.trim().to_lowercase();
        let visible: Vec<usize> = taglines
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| filter.is_empty() || line.to_lowercase().contains(&filter))
            .map(|(index, _)| index)
            .collect();
        if dialog.selected.is_none_or(|selected| !visible.contains(&selected)) {
            dialog.selected = visible.first().copied();
        }
        let mut action = None;
        let focused = context.memory(|memory| memory.focused());
        if dialog.edit.is_some() {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                action = Some(Action::CancelEdit);
            }
        } else if !egui::Popup::is_any_open(context) {
            if widgets::list_keys(context, &visible, &mut dialog.selected) {
                dialog.reveal = true;
            }
            if dialog.selected.is_some() && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Enter)) {
                action = Some(if dialog.pick { Action::Use } else { Action::Edit });
            }
            if focused != Some(egui::Id::new(FILTER_ID))
                && dialog.selected.is_some()
                && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Delete))
            {
                action = Some(Action::Delete);
            }
        }
        let count = taglines.lines.len();
        let subtitle = match count {
            0 => fl!(LANGUAGE_LOADER, "taglines-count-none"),
            1 => fl!(LANGUAGE_LOADER, "taglines-count-one"),
            count => fl!(LANGUAGE_LOADER, "taglines-count-many", count = count),
        };
        let icons = &mut self.icons;
        let response = appearance::Dialog::new("mail-taglines")
            .title(if dialog.pick {
                fl!(LANGUAGE_LOADER, "taglines-title-choose")
            } else {
                fl!(LANGUAGE_LOADER, "taglines-title-manage")
            })
            .subtitle(subtitle)
            .size(appearance::DialogSize::Large)
            .fixed_height(520.0)
            .show(context, |frame| {
                let ui = frame.ui();
                let focus = dialog.focus.take();
                if let Some((index, text)) = &mut dialog.edit {
                    ui.label(
                        egui::RichText::new(if index.is_some() {
                            fl!(LANGUAGE_LOADER, "taglines-edit-heading")
                        } else {
                            fl!(LANGUAGE_LOADER, "taglines-new-heading")
                        })
                        .weak(),
                    );
                    ui.horizontal(|ui| {
                        let buttons = 150.0;
                        let edit = appearance::text_edit(text)
                            .id(egui::Id::new(EDIT_ID))
                            .hint_text(fl!(LANGUAGE_LOADER, "taglines-edit-hint"))
                            .char_limit(TAGLINE_LENGTH)
                            .desired_width((ui.available_width() - buttons).max(120.0));
                        let response = ui.add(edit);
                        if focus == Some(EDIT_ID) {
                            response.request_focus();
                        }
                        if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                            action = Some(Action::Save);
                        }
                        if ui
                            .add_enabled(!text.trim().is_empty(), appearance::primary_button(fl!(LANGUAGE_LOADER, "taglines-save")))
                            .clicked()
                        {
                            action = Some(Action::Save);
                        }
                        if ui.button(fl!(LANGUAGE_LOADER, "taglines-cancel")).clicked() {
                            action = Some(Action::CancelEdit);
                        }
                    });
                    ui.label(
                        egui::RichText::new(fl!(LANGUAGE_LOADER, "taglines-shown-as", count = text.chars().count(), limit = TAGLINE_LENGTH))
                            .size(11.5)
                            .weak(),
                    );
                } else {
                    ui.horizontal(|ui| {
                        let edit = appearance::text_edit(&mut dialog.filter)
                            .id(egui::Id::new(FILTER_ID))
                            .hint_text(fl!(LANGUAGE_LOADER, "taglines-filter-hint"))
                            .desired_width((ui.available_width() - 130.0).max(120.0));
                        let response = ui.add(edit);
                        if focus == Some(FILTER_ID) {
                            response.request_focus();
                        }
                        widgets::keep_list_focus(ui, &response);
                        if response.changed() {
                            dialog.reveal = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(fl!(LANGUAGE_LOADER, "taglines-new-button"))
                                .on_hover_text(fl!(LANGUAGE_LOADER, "taglines-new-tooltip"))
                                .clicked()
                            {
                                action = Some(Action::New);
                            }
                        });
                    });
                }
                if let Some(error) = &dialog.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.add_space(6.0);
                let reveal = std::mem::take(&mut dialog.reveal);
                frame.content(|ui| {
                    if visible.is_empty() {
                        ui.add_space(24.0);
                        ui.vertical_centered(|ui| {
                            if count == 0 {
                                ui.label(appearance::bold(ui, fl!(LANGUAGE_LOADER, "taglines-empty-title")));
                                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "taglines-empty-help")).weak());
                            } else {
                                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "taglines-no-filter-match", filter = dialog.filter.trim())).weak());
                            }
                        });
                        return;
                    }
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let font = egui::FontId::proportional(14.0);
                    for &index in &visible {
                        let selected = dialog.selected == Some(index);
                        let (rect, response) = widgets::list_row(ui, selected, ROW_HEIGHT);
                        let color = widgets::list_text(ui, selected);
                        let actions = if selected { 64.0 } else { 0.0 };
                        let text_rect = egui::Rect::from_min_max(rect.min + egui::vec2(10.0, 0.0), rect.max - egui::vec2(10.0 + actions, 0.0));
                        widgets::paint_line(ui, text_rect, &format!("... {}", taglines.lines[index]), font.clone(), color, &filter);
                        if selected {
                            let buttons = egui::Rect::from_min_max(egui::pos2(rect.right() - actions, rect.top()), rect.max);
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(buttons)
                                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    if icons.button(ui, Icon::Delete, &fl!(LANGUAGE_LOADER, "taglines-delete-tooltip"), true).clicked() {
                                        action = Some(Action::Delete);
                                    }
                                    if icons.button(ui, Icon::Compose, &fl!(LANGUAGE_LOADER, "taglines-edit-tooltip"), true).clicked() {
                                        action = Some(Action::Edit);
                                    }
                                },
                            );
                            if reveal {
                                response.scroll_to_me(None);
                            }
                        }
                        if response.clicked() {
                            dialog.selected = Some(index);
                        }
                        if response.double_clicked() {
                            dialog.selected = Some(index);
                            action = Some(if dialog.pick { Action::Use } else { Action::Edit });
                        }
                    }
                });
                if dialog.pick {
                    frame.buttons([
                        appearance::DialogButton::secondary(fl!(LANGUAGE_LOADER, "taglines-random"), Action::Random)
                            .leading()
                            .enabled(count > 0)
                            .tooltip(fl!(LANGUAGE_LOADER, "taglines-random-tooltip")),
                        appearance::DialogButton::secondary(fl!(LANGUAGE_LOADER, "taglines-no-tagline"), Action::NoTagline).leading(),
                        appearance::DialogButton::cancel(fl!(LANGUAGE_LOADER, "taglines-cancel"), Action::Close),
                        appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "taglines-use"), Action::Use).enabled(dialog.selected.is_some()),
                    ]);
                } else {
                    frame.buttons([appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "taglines-close"), Action::Close).cancels()]);
                }
            });
        let action = action.or(response.action).or(response.dismissed.then_some(Action::Close));
        let selected = dialog.selected;
        let chosen = match action {
            Some(Action::Close) => return self.close_taglines(),
            Some(Action::Use) => selected.and_then(|index| taglines.lines.get(index).cloned()),
            Some(Action::Random) => taglines.random().map(str::to_string),
            Some(Action::NoTagline) => Some(String::new()),
            Some(Action::New) => {
                dialog.edit = Some((None, String::new()));
                dialog.focus = Some(EDIT_ID);
                return false;
            }
            Some(Action::Edit) => {
                if let Some(index) = selected {
                    dialog.edit = Some((Some(index), taglines.lines[index].clone()));
                    dialog.focus = Some(EDIT_ID);
                }
                return false;
            }
            Some(Action::CancelEdit) => {
                dialog.edit = None;
                dialog.error = None;
                dialog.focus = Some(FILTER_ID);
                return false;
            }
            Some(Action::Delete) => {
                if let Some(index) = selected {
                    let position = visible.iter().position(|&entry| entry == index).unwrap_or_default();
                    taglines.remove(index);
                    // The entries after the removed one move up by one.
                    dialog.selected = visible
                        .get(position + 1)
                        .map(|&next| next - 1)
                        .or_else(|| position.checked_sub(1).map(|previous| visible[previous]));
                    dialog.error = taglines
                        .save()
                        .err()
                        .map(|error| fl!(LANGUAGE_LOADER, "taglines-save-error", error = error.to_string()));
                }
                return false;
            }
            Some(Action::Save) => {
                let Some((index, text)) = dialog.edit.clone() else {
                    return false;
                };
                let stored = match index {
                    Some(index) => taglines.set(index, &text).then_some(index),
                    None => taglines.add(&text),
                };
                if let Some(index) = stored {
                    dialog.edit = None;
                    dialog.selected = Some(index);
                    dialog.reveal = true;
                    dialog.focus = Some(FILTER_ID);
                    if !taglines.lines[index].to_lowercase().contains(&filter) {
                        dialog.filter.clear();
                    }
                    dialog.error = taglines
                        .save()
                        .err()
                        .map(|error| fl!(LANGUAGE_LOADER, "taglines-save-error", error = error.to_string()));
                }
                return false;
            }
            None => return false,
        };
        if let (Some(tagline), Some(composer)) = (chosen, &mut self.composer) {
            composer.draft.tagline = tagline;
        }
        self.close_taglines()
    }

    fn close_taglines(&mut self) -> bool {
        self.tagline_dialog = None;
        if let Some(composer) = &mut self.composer {
            composer.editor.request_focus();
        }
        true
    }
}
