//! The address book: pick the recipient of the message being written, or manage the contacts.

use eframe::egui::{self, Key};
use i18n_embed_fl::fl;
use icy_engine_gui::egui::appearance;
use icy_mail::{
    address_book::{AddressBook, Contact, FIELD_LENGTH},
    LANGUAGE_LOADER,
};

use super::{
    app::{Folder, MailApp, Modal, NoticeKind},
    widgets::{self, Icon},
};

const ROW_HEIGHT: f32 = 30.0;
const FILTER_ID: &str = "address-filter";
const NAME_ID: &str = "address-name";

pub struct AddressDialog {
    /// Opened from the composer to choose the recipient.
    pick: bool,
    filter: String,
    /// Index into the contact list.
    selected: Option<usize>,
    /// Contact being written: `None` adds a new one, otherwise the index being changed.
    edit: Option<(Option<usize>, Contact)>,
    focus: Option<&'static str>,
    reveal: bool,
    error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    Use,
    Write,
    Close,
    New,
    Edit,
    Delete,
    Save,
    CancelEdit,
}

impl MailApp {
    pub fn address_file(&self) -> icy_mail::Res<AddressBook> {
        match &self.storage {
            Some(directory) => AddressBook::open_in(directory),
            None => AddressBook::open(),
        }
    }

    /// Opens the address book, re-read so changes from other windows show up.
    pub fn open_address_book(&mut self, pick: bool) {
        match self.address_file() {
            Ok(book) => self.address_book = Some(book),
            Err(error) => {
                self.error = Some(fl!(LANGUAGE_LOADER, "address-read-error", error = error.to_string()));
                return;
            }
        }
        let Some(book) = &self.address_book else {
            return;
        };
        let current = self.composer.as_ref().filter(|_| pick).map(|composer| composer.draft.to.clone());
        let selected = current.and_then(|current| book.find(&current)).or((!book.contacts.is_empty()).then_some(0));
        self.address_dialog = Some(AddressDialog {
            pick,
            filter: String::new(),
            selected,
            edit: None,
            focus: Some(FILTER_ID),
            reveal: true,
            error: None,
        });
        self.modal = Some(Modal::AddressBook);
    }

    /// Adds the author of the selected message to the address book.
    pub fn add_sender(&mut self, context: &egui::Context) {
        if !self.message_selected() || self.folder == Folder::Drafts {
            return;
        }
        let Some(name) = self.selected_info().map(|info| info.from.trim().to_string()) else {
            return;
        };
        let mut book = match self.address_file() {
            Ok(book) => book,
            Err(error) => {
                self.error = Some(fl!(LANGUAGE_LOADER, "address-read-error", error = error.to_string()));
                return;
            }
        };
        if book.find(&name).is_some() {
            self.notify(
                context,
                NoticeKind::Info,
                fl!(LANGUAGE_LOADER, "address-sender-already-present", name = name.as_str()),
            );
        } else if book.add(Contact::new(&name, "")).is_some() {
            match book.save() {
                Ok(()) => self.notify(context, NoticeKind::Success, fl!(LANGUAGE_LOADER, "address-sender-added", name = name.as_str())),
                Err(error) => self.error = Some(fl!(LANGUAGE_LOADER, "address-save-error-multiline", error = error.to_string())),
            }
        }
        self.address_book = Some(book);
    }

    /// Shows the address book; returns whether it closed.
    pub fn address_dialog(&mut self, context: &egui::Context) -> bool {
        let (Some(dialog), Some(book)) = (&mut self.address_dialog, &mut self.address_book) else {
            self.address_dialog = None;
            return true;
        };
        let filter = dialog.filter.trim().to_string();
        let visible: Vec<usize> = book
            .contacts
            .iter()
            .enumerate()
            .filter(|(_, contact)| contact.matches(&filter))
            .map(|(index, _)| index)
            .collect();
        if dialog.selected.is_none_or(|selected| !visible.contains(&selected)) {
            dialog.selected = visible.first().copied();
        }
        let can_write = self.reader.package.is_some() && self.composer.is_none();
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
                action = Some(if dialog.pick {
                    Action::Use
                } else if can_write {
                    Action::Write
                } else {
                    Action::Edit
                });
            }
            if focused != Some(egui::Id::new(FILTER_ID))
                && dialog.selected.is_some()
                && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Delete))
            {
                action = Some(Action::Delete);
            }
        }
        let count = book.contacts.len();
        let subtitle = match count {
            0 => fl!(LANGUAGE_LOADER, "address-count-none"),
            1 => fl!(LANGUAGE_LOADER, "address-count-one"),
            count => fl!(LANGUAGE_LOADER, "address-count-many", count = count),
        };
        let icons = &mut self.icons;
        let response = appearance::Dialog::new("mail-address-book")
            .title(if dialog.pick {
                fl!(LANGUAGE_LOADER, "address-title-choose")
            } else {
                fl!(LANGUAGE_LOADER, "address-title-manage")
            })
            .subtitle(subtitle)
            .size(appearance::DialogSize::Large)
            .fixed_height(520.0)
            .show(context, |frame| {
                let ui = frame.ui();
                let focus = dialog.focus.take();
                if let Some((index, contact)) = &mut dialog.edit {
                    ui.label(
                        egui::RichText::new(if index.is_some() {
                            fl!(LANGUAGE_LOADER, "address-edit-heading")
                        } else {
                            fl!(LANGUAGE_LOADER, "address-new-heading")
                        })
                        .weak(),
                    );
                    let mut enter = false;
                    egui::Grid::new("address-edit").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        let width = (ui.available_width() - 80.0).max(160.0);
                        ui.label(fl!(LANGUAGE_LOADER, "address-name-label"));
                        let name = ui.add(
                            appearance::text_edit(&mut contact.name)
                                .id(egui::Id::new(NAME_ID))
                                .hint_text(fl!(LANGUAGE_LOADER, "address-name-hint"))
                                .char_limit(FIELD_LENGTH)
                                .desired_width(width),
                        );
                        if focus == Some(NAME_ID) {
                            name.request_focus();
                        }
                        ui.end_row();
                        ui.label(fl!(LANGUAGE_LOADER, "address-address-label"));
                        let address = ui.add(
                            appearance::text_edit(&mut contact.address)
                                .hint_text(fl!(LANGUAGE_LOADER, "address-address-hint"))
                                .char_limit(FIELD_LENGTH)
                                .desired_width(width),
                        );
                        ui.end_row();
                        enter = (name.lost_focus() || address.lost_focus()) && ui.input(|input| input.key_pressed(Key::Enter));
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !contact.name.trim().is_empty(),
                                appearance::primary_button(fl!(LANGUAGE_LOADER, "address-save")),
                            )
                            .clicked()
                            || enter
                        {
                            action = Some(Action::Save);
                        }
                        if ui.button(fl!(LANGUAGE_LOADER, "address-cancel")).clicked() {
                            action = Some(Action::CancelEdit);
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        let edit = appearance::text_edit(&mut dialog.filter)
                            .id(egui::Id::new(FILTER_ID))
                            .hint_text(fl!(LANGUAGE_LOADER, "address-filter-hint"))
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
                                .button(fl!(LANGUAGE_LOADER, "address-new-button"))
                                .on_hover_text(fl!(LANGUAGE_LOADER, "address-new-tooltip"))
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
                                ui.label(appearance::bold(ui, fl!(LANGUAGE_LOADER, "address-empty-title")));
                                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "address-empty-help")).weak());
                            } else {
                                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "address-no-filter-match", filter = filter.as_str())).weak());
                            }
                        });
                        return;
                    }
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for &index in &visible {
                        let contact = &book.contacts[index];
                        let selected = dialog.selected == Some(index);
                        let (rect, response) = widgets::list_row(ui, selected, ROW_HEIGHT);
                        let color = widgets::list_text(ui, selected);
                        let weak = if selected {
                            color.gamma_multiply(0.8)
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        let actions = if selected { 64.0 } else { 0.0 };
                        let inner = egui::Rect::from_min_max(rect.min + egui::vec2(10.0, 0.0), rect.max - egui::vec2(10.0 + actions, 0.0));
                        let split = inner.left() + inner.width() * 0.5;
                        let name = egui::Rect::from_min_max(inner.min, egui::pos2(split - 8.0, inner.bottom()));
                        let address = egui::Rect::from_min_max(egui::pos2(split, inner.top()), inner.max);
                        let bold = egui::FontId::new(14.0, appearance::bold_family(ui));
                        widgets::paint_line(ui, name, &contact.name, bold, color, &filter);
                        widgets::paint_line(ui, address, &contact.address, egui::FontId::proportional(13.0), weak, &filter);
                        if selected {
                            let buttons = egui::Rect::from_min_max(egui::pos2(rect.right() - actions, rect.top()), rect.max);
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(buttons)
                                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    if icons.button(ui, Icon::Delete, &fl!(LANGUAGE_LOADER, "address-delete-tooltip"), true).clicked() {
                                        action = Some(Action::Delete);
                                    }
                                    if icons.button(ui, Icon::Compose, &fl!(LANGUAGE_LOADER, "address-edit-tooltip"), true).clicked() {
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
                            action = Some(if dialog.pick {
                                Action::Use
                            } else if can_write {
                                Action::Write
                            } else {
                                Action::Edit
                            });
                        }
                    }
                });
                let chosen = dialog.selected.is_some();
                if dialog.pick {
                    frame.buttons([
                        appearance::DialogButton::cancel(fl!(LANGUAGE_LOADER, "address-cancel"), Action::Close),
                        appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "address-use-recipient"), Action::Use).enabled(chosen),
                    ]);
                } else {
                    frame.buttons([
                        appearance::DialogButton::secondary(fl!(LANGUAGE_LOADER, "address-write-message"), Action::Write)
                            .leading()
                            .enabled(chosen && can_write)
                            .tooltip(fl!(LANGUAGE_LOADER, "address-write-message-tooltip")),
                        appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "address-close"), Action::Close).cancels(),
                    ]);
                }
            });
        let action = action.or(response.action).or(response.dismissed.then_some(Action::Close));
        let selected = dialog.selected;
        let name = selected.and_then(|index| book.contacts.get(index)).map(|contact| contact.name.clone());
        match action {
            Some(Action::Close) => {}
            Some(Action::Use) => {
                if let (Some(name), Some(composer)) = (name, &mut self.composer) {
                    composer.draft.to = name;
                }
            }
            Some(Action::Write) => {
                self.address_dialog = None;
                self.modal = None;
                if let Some(name) = name {
                    self.new_draft(context);
                    if let Some(composer) = &mut self.composer {
                        composer.draft.to.clone_from(&name);
                        composer.original.to = name;
                    }
                }
                return true;
            }
            Some(Action::New) => {
                dialog.edit = Some((None, Contact::default()));
                dialog.focus = Some(NAME_ID);
                return false;
            }
            Some(Action::Edit) => {
                if let Some(index) = selected {
                    dialog.edit = Some((Some(index), book.contacts[index].clone()));
                    dialog.focus = Some(NAME_ID);
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
                    book.remove(index);
                    dialog.selected = visible
                        .get(position + 1)
                        .map(|&next| next - 1)
                        .or_else(|| position.checked_sub(1).map(|previous| visible[previous]));
                    dialog.error = book
                        .save()
                        .err()
                        .map(|error| fl!(LANGUAGE_LOADER, "address-save-error", error = error.to_string()));
                }
                return false;
            }
            Some(Action::Save) => {
                let Some((index, contact)) = dialog.edit.clone() else {
                    return false;
                };
                let stored = match index {
                    Some(index) => book.update(index, contact),
                    None if book.find(&contact.name).is_some() => None,
                    None => book.add(contact),
                };
                match stored {
                    Some(index) => {
                        dialog.edit = None;
                        dialog.selected = Some(index);
                        dialog.reveal = true;
                        dialog.focus = Some(FILTER_ID);
                        if !book.contacts[index].matches(&filter) {
                            dialog.filter.clear();
                        }
                        dialog.error = book
                            .save()
                            .err()
                            .map(|error| fl!(LANGUAGE_LOADER, "address-save-error", error = error.to_string()));
                    }
                    None => dialog.error = Some(fl!(LANGUAGE_LOADER, "address-duplicate-name")),
                }
                return false;
            }
            None => return false,
        }
        self.address_dialog = None;
        if let Some(composer) = &mut self.composer {
            composer.editor.request_focus();
        }
        true
    }
}
