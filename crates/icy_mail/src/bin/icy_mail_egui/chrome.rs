use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use i18n_embed_fl::fl;
use icy_engine_gui::ScalingMode;
use icy_mail::LANGUAGE_LOADER;
use icy_mail::{
    options::Theme,
    reader::{SearchFields, ViewMode},
};

use super::{
    app::{Folder, MailApp, Modal, NoticeKind},
    settings,
    widgets::{self, Icon},
};

/// Toolbar width below which the actions lose their captions.
const CAPTION_WIDTH: f32 = 900.0;
/// Toolbar width below which the search field moves to its own row.
const INLINE_SEARCH_WIDTH: f32 = 600.0;

pub fn shortcut(context: &egui::Context, modifiers: Modifiers, key: Key) -> String {
    context.format_shortcut(&KeyboardShortcut::new(modifiers, key))
}

/// Menus and their submenus share one width, so shortcuts and submenu arrows line up.
fn menu_width(ui: &mut egui::Ui) {
    ui.set_min_width(250.0);
}

/// Menu entry with a right-aligned shortcut; closes the menu when clicked.
fn item(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    let button = egui::Button::new(label).shortcut_text(egui::RichText::new(shortcut).weak());
    let clicked = ui.add_enabled(enabled, button).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

impl MailApp {
    pub fn toolbar(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let width = ui.available_width();
        let captions = width >= CAPTION_WIDTH;
        let inline_search = width >= INLINE_SEARCH_WIDTH;
        let open = self.reader.package.is_some();
        let drafts = self.draft_count();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if self
                .icons
                .tool(
                    ui,
                    Icon::Open,
                    captions.then_some(fl!(LANGUAGE_LOADER, "toolbar-open").as_str()),
                    &fl!(LANGUAGE_LOADER, "toolbar-open-tooltip"),
                    !self.loader.picking,
                    false,
                )
                .clicked()
            {
                self.loader.pick(&context);
            }
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            if self
                .icons
                .tool(
                    ui,
                    Icon::Compose,
                    captions.then_some(fl!(LANGUAGE_LOADER, "toolbar-new").as_str()),
                    &fl!(LANGUAGE_LOADER, "toolbar-new-tooltip"),
                    open,
                    false,
                )
                .clicked()
            {
                self.new_draft(&context);
            }
            let export = if drafts > 0 {
                fl!(LANGUAGE_LOADER, "toolbar-export-count", count = drafts)
            } else {
                fl!(LANGUAGE_LOADER, "toolbar-export")
            };
            if self
                .icons
                .tool(
                    ui,
                    Icon::Export,
                    captions.then_some(export.as_str()),
                    &fl!(LANGUAGE_LOADER, "toolbar-export-tooltip"),
                    drafts > 0,
                    false,
                )
                .clicked()
            {
                self.export(&context);
            }
            if open && inline_search {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                self.next_unread_button(ui);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let menu = self.icons.button(ui, Icon::Menu, &fl!(LANGUAGE_LOADER, "toolbar-menu"), true);
                egui::Popup::menu(&menu).show(|ui| self.menu(ui));
                ui.add_space(4.0);
                for (mode, icon, tooltip) in [
                    (ViewMode::Threads, Icon::Threads, fl!(LANGUAGE_LOADER, "toolbar-threads-tooltip")),
                    (ViewMode::List, Icon::List, fl!(LANGUAGE_LOADER, "toolbar-list-tooltip")),
                ] {
                    if self.icons.toggle(ui, icon, &tooltip, self.reader.view_mode == mode).clicked() {
                        self.set_mode(mode);
                    }
                }
                if inline_search {
                    ui.add_space(6.0);
                    self.search(ui, (ui.available_width() - 12.0).clamp(160.0, 300.0));
                }
            });
        });
        if !inline_search {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if open {
                    self.next_unread_button(ui);
                    ui.add_space(4.0);
                }
                self.search(ui, ui.available_width());
            });
        }
    }

    /// The primary reading action; N and Space do the same from the keyboard.
    fn next_unread_button(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let label = fl!(LANGUAGE_LOADER, "toolbar-next-unread");
        if ui
            .add_enabled(self.counts.unread > 0, icy_engine_gui::egui::appearance::primary_button(&label))
            .on_hover_text(fl!(LANGUAGE_LOADER, "toolbar-next-unread-tooltip"))
            .clicked()
        {
            self.next_unread(&context);
        }
    }

    fn search(&mut self, ui: &mut egui::Ui, width: f32) {
        let id = egui::Id::new("mail-search");
        let weak = ui.visuals().weak_text_color();
        let enabled = self.reader.package.is_some();
        ui.add_enabled_ui(enabled, |ui| {
            widgets::field(ui, width, id, |ui| {
                // The magnifier chooses what the search looks at; it takes the accent while limited.
                let fields = self.reader.search_fields;
                let tint = if fields.all() { weak } else { widgets::accent(ui) };
                let image = self.icons.image(ui.ctx(), Icon::Search, 16.0).tint(tint);
                let scope = ui
                    .add(egui::Button::image(image).frame(false))
                    .on_hover_text(fl!(LANGUAGE_LOADER, "search-fields-tooltip"));
                egui::Popup::menu(&scope)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.set_min_width(180.0);
                        ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "search-fields-title")).weak());
                        let mut fields = self.reader.search_fields;
                        let only = |fields: SearchFields, field: bool| {
                            field && [fields.from, fields.to, fields.subject, fields.text].iter().filter(|on| **on).count() == 1
                        };
                        let before = fields;
                        for (value, label) in [
                            (&mut fields.from, fl!(LANGUAGE_LOADER, "search-field-from")),
                            (&mut fields.to, fl!(LANGUAGE_LOADER, "search-field-to")),
                            (&mut fields.subject, fl!(LANGUAGE_LOADER, "search-field-subject")),
                            (&mut fields.text, fl!(LANGUAGE_LOADER, "search-field-text")),
                        ] {
                            // The last field searched stays on, so the search never finds nothing by design.
                            ui.add_enabled_ui(!only(before, *value), |ui| ui.checkbox(value, label));
                        }
                        ui.separator();
                        if ui
                            .add_enabled(!fields.all(), egui::Button::new(fl!(LANGUAGE_LOADER, "search-fields-all")))
                            .clicked()
                        {
                            fields = SearchFields::default();
                        }
                        if fields != before && fields.any() {
                            self.set_search_fields(fields);
                        }
                    });
                if self.loader.searching {
                    ui.spinner().on_hover_text(fl!(LANGUAGE_LOADER, "search-bodies-running"));
                }
                let clear = !self.reader.filter.is_empty();
                let edit_width = ui.available_width() - if clear { 24.0 } else { 0.0 };
                let response = ui.add_sized(
                    [edit_width.max(20.0), 26.0],
                    egui::TextEdit::singleline(&mut self.reader.filter)
                        .id(id)
                        .frame(false)
                        .margin(egui::vec2(4.0, 4.0))
                        .hint_text(search_hint(self.reader.search_fields)),
                );
                if response.has_focus() {
                    ui.memory_mut(|memory| {
                        memory.set_focus_lock_filter(
                            response.id,
                            egui::EventFilter {
                                escape: true,
                                ..Default::default()
                            },
                        );
                    });
                }
                if response.changed() {
                    if !self.folder.holds_messages() {
                        self.select_folder(Folder::All);
                    }
                    self.filter_changed();
                }
                if clear {
                    let image = self.icons.image(ui.ctx(), Icon::Close, 14.0);
                    if ui
                        .add(egui::Button::image(image).frame(false).image_tint_follows_text_color(true))
                        .on_hover_text(fl!(LANGUAGE_LOADER, "toolbar-search-clear"))
                        .clicked()
                    {
                        self.reader.filter.clear();
                        self.filter_changed();
                        response.request_focus();
                    }
                }
                response
            });
        });
    }

    /// The main menu: File, Message, View, Tools and Help, like Icy Term's.
    fn menu(&mut self, ui: &mut egui::Ui) {
        menu_width(ui);
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-file"), |ui| self.file_menu(ui));
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-message"), |ui| self.message_menu(ui));
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-view"), |ui| self.view_menu(ui));
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-tools"), |ui| self.tools_menu(ui));
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-help"), |ui| self.help_menu(ui));
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        menu_width(ui);
        let context = ui.ctx().clone();
        let command = |key| shortcut(&context, Modifiers::COMMAND, key);
        let command_shift = |key| shortcut(&context, Modifiers::COMMAND | Modifiers::SHIFT, key);
        let open = self.reader.package.is_some();
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-new-window"), &command_shift(Key::N), true) {
            self.new_window = true;
        }
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-open-packet"), &command(Key::O), !self.loader.picking) {
            self.loader.pick(&context);
        }
        let recent = self.recent.as_ref().map(|recent| recent.packets.clone()).unwrap_or_default();
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            ui.menu_button(fl!(LANGUAGE_LOADER, "menu-open-recent"), |ui| {
                menu_width(ui);
                for path in &recent {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if ui.button(name).on_hover_text(path.display().to_string()).clicked() {
                        self.open(path.clone(), &context);
                        ui.close();
                    }
                }
            });
        });
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-reload-packet"), "F5", open && self.loading.is_none()) {
            self.reload(&context);
        }
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-packet-information"), "", open) {
            self.modal = Some(Modal::PacketInfo);
        }
        ui.separator();
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-export-replies"), &command_shift(Key::E), self.draft_count() > 0) {
            self.export(&context);
        }
        ui.separator();
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-settings"), &command(Key::Comma), true) {
            self.open_settings(&context);
        }
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-close-window"), &command(Key::W), true) {
            self.close(&context);
        }
    }

    fn message_menu(&mut self, ui: &mut egui::Ui) {
        menu_width(ui);
        let context = ui.ctx().clone();
        let command = |key| shortcut(&context, Modifiers::COMMAND, key);
        let open = self.reader.package.is_some();
        let selected = self.message_selected();
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-new-message"), &command(Key::N), open) {
            self.new_draft(&context);
        }
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-reply"), &command(Key::R), selected) {
            self.reply(&context, false);
        }
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-forward"), &command(Key::L), selected) {
            self.reply(&context, true);
        }
        ui.separator();
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-next-unread"), "N", open) {
            self.next_unread(&context);
        }
        let unread = self.reader.selected_message.is_some_and(|index| !self.reader.is_read(index));
        let label = if unread {
            fl!(LANGUAGE_LOADER, "menu-mark-read")
        } else {
            fl!(LANGUAGE_LOADER, "menu-mark-unread")
        };
        if item(ui, &label, "M", selected) {
            self.toggle_read(&context);
        }
        let starred = self.reader.selected_message.is_some_and(|index| self.reader.is_starred(index));
        let label = if starred {
            fl!(LANGUAGE_LOADER, "menu-unstar")
        } else {
            fl!(LANGUAGE_LOADER, "menu-star")
        };
        if item(ui, &label, "S", selected) {
            self.toggle_star(&context);
        }
        if item(
            ui,
            &fl!(LANGUAGE_LOADER, "menu-mark-folder-read"),
            &shortcut(&context, Modifiers::SHIFT, Key::C),
            open && self.folder.holds_messages(),
        ) {
            self.mark_folder_read(&context);
        }
        if item(
            ui,
            &fl!(LANGUAGE_LOADER, "menu-mark-thread-read"),
            &shortcut(&context, Modifiers::SHIFT, Key::M),
            self.thread_selected(),
        ) {
            self.mark_thread_read(&context);
        }
        ui.separator();
        let tagline = selected && self.message_tagline().is_some();
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-save-tagline"), "T", tagline) {
            self.save_tagline(&context);
        }
        if item(
            ui,
            &fl!(LANGUAGE_LOADER, "menu-add-author"),
            &shortcut(&context, Modifiers::SHIFT, Key::A),
            selected,
        ) {
            self.add_sender(&context);
        }
        ui.separator();
        let saving = selected && !self.loader.save_picking;
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-save-message"), "", saving) {
            self.save_message(&context, false);
        }
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-save-message-utf8"), "", saving) {
            self.save_message(&context, true);
        }
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        menu_width(ui);
        let context = ui.ctx().clone();
        let switch = shortcut(&context, Modifiers::COMMAND, Key::T);
        for (mode, label) in [
            (ViewMode::List, fl!(LANGUAGE_LOADER, "menu-view-list")),
            (ViewMode::Threads, fl!(LANGUAGE_LOADER, "menu-view-threads")),
        ] {
            let button = egui::Button::selectable(self.reader.view_mode == mode, label).shortcut_text(egui::RichText::new(&switch).weak());
            if ui.add(button).clicked() {
                self.set_mode(mode);
                ui.close();
            }
        }
        let unread_only = self.reader.unread_only;
        if ui
            .add(egui::Button::selectable(unread_only, fl!(LANGUAGE_LOADER, "menu-unread-only")))
            .clicked()
        {
            self.set_unread_only(!unread_only);
            ui.close();
        }
        let threads = self.reader.package.is_some() && self.reader.view_mode == ViewMode::Threads;
        if item(
            ui,
            &fl!(LANGUAGE_LOADER, "menu-collapse-all-threads"),
            &shortcut(&context, Modifiers::SHIFT, Key::ArrowLeft),
            threads,
        ) {
            self.set_all_threads_collapsed(true);
        }
        if item(
            ui,
            &fl!(LANGUAGE_LOADER, "menu-expand-all-threads"),
            &shortcut(&context, Modifiers::SHIFT, Key::ArrowRight),
            threads,
        ) {
            self.set_all_threads_collapsed(false);
        }
        ui.separator();
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-reading-pane"), |ui| {
            menu_width(ui);
            for pane in settings::READING_PANES {
                if ui
                    .add(egui::Button::selectable(self.reading_pane == pane, settings::reading_pane_name(pane)))
                    .clicked()
                {
                    self.reading_pane = pane;
                    ui.close();
                }
            }
        });
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-message-zoom"), |ui| {
            menu_width(ui);
            zoom_choices(ui, &mut self.settings.scaling_mode);
        });
        ui.menu_button(fl!(LANGUAGE_LOADER, "menu-appearance"), |ui| {
            menu_width(ui);
            let current = context.options(|options| options.theme_preference);
            for theme in [Theme::System, Theme::Light, Theme::Dark] {
                let preference = settings::theme_preference(theme);
                if ui.add(egui::Button::selectable(current == preference, settings::theme_name(theme))).clicked() {
                    context.set_theme(preference);
                    ui.close();
                }
            }
        });
    }

    fn tools_menu(&mut self, ui: &mut egui::Ui) {
        menu_width(ui);
        let context = ui.ctx().clone();
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-address-book"), "A", true) {
            self.open_address_book(false);
        }
        if item(
            ui,
            &fl!(LANGUAGE_LOADER, "menu-taglines"),
            &shortcut(&context, Modifiers::COMMAND | Modifiers::SHIFT, Key::T),
            true,
        ) {
            self.open_taglines(false);
        }
    }

    fn help_menu(&mut self, ui: &mut egui::Ui) {
        menu_width(ui);
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-keyboard-shortcuts"), "F1", true) {
            self.modal = Some(Modal::Shortcuts);
        }
        if item(ui, &fl!(LANGUAGE_LOADER, "menu-about"), "", true) {
            self.open_about();
        }
    }

    pub fn status_bar(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        if self.notice.is_none() {
            if let Some(composer) = &mut self.composer {
                let help = ui.horizontal_centered(|ui| composer.editor.status_bar(ui)).inner;
                if help {
                    self.modal = Some(Modal::Shortcuts);
                }
                return;
            }
        }
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if let Some(notice) = &self.notice {
                let (icon, color) = match notice.kind {
                    NoticeKind::Info => (Icon::Info, ui.visuals().text_color()),
                    NoticeKind::Success => (Icon::Check, egui::Color32::from_rgb(76, 175, 80)),
                    NoticeKind::Warning => (Icon::Warning, widgets::warning(ui)),
                };
                let text = notice.text.clone();
                ui.add(self.icons.image(&context, icon, 16.0).tint(color));
                ui.add(egui::Label::new(text).truncate());
            } else if let Some(package) = &self.reader.package {
                let total = package.message_count();
                let unread = self.counts.unread;
                let text = fl!(
                    LANGUAGE_LOADER,
                    "status-reading-progress",
                    read = total.saturating_sub(unread),
                    total = total,
                    unread = unread
                );
                ui.label(text).on_hover_text(fl!(LANGUAGE_LOADER, "status-reading-progress-tooltip"));
            } else if self.loading.is_none() {
                ui.weak(fl!(LANGUAGE_LOADER, "status-no-packet"));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(latest) = &self.latest_version {
                    let label = fl!(LANGUAGE_LOADER, "status-update-available", version = latest.to_string());
                    let link = egui::RichText::new(label).size(12.0).color(ui.visuals().hyperlink_color);
                    if ui
                        .add(egui::Label::new(link).sense(egui::Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!(
                            "https://github.com/mkrueger/icy_tools/releases/tag/IcyMail{latest}"
                        )));
                    }
                }
                if self.reader.package.is_none() {
                    return;
                }
                let zoom = settings::zoom_name(self.settings.scaling_mode);
                widgets::status_menu(ui, egui::RichText::new(zoom).size(12.0), &fl!(LANGUAGE_LOADER, "status-message-zoom"), |ui| {
                    zoom_choices(ui, &mut self.settings.scaling_mode);
                });
                let drafts = self.draft_count();
                if drafts > 0 && ui.available_width() > 260.0 {
                    ui.separator();
                    let label = fl!(LANGUAGE_LOADER, "status-outbox-drafts", count = drafts);
                    let link = egui::RichText::new(label).size(12.0).color(widgets::accent(ui));
                    if ui
                        .add(egui::Label::new(link).sense(egui::Sense::click()))
                        .on_hover_text(fl!(LANGUAGE_LOADER, "status-show-outbox"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.select_folder(Folder::Drafts);
                    }
                }
                if ui.available_width() > 360.0 {
                    ui.separator();
                    ui.add(egui::Label::new(egui::RichText::new(self.bbs_name()).size(12.0).weak()).truncate())
                        .on_hover_text(self.path.as_ref().map_or(String::new(), |path| path.display().to_string()));
                }
            });
        });
    }
}

/// "Search messages", or which fields a limited search looks at.
fn search_hint(fields: SearchFields) -> String {
    if fields.all() {
        return fl!(LANGUAGE_LOADER, "toolbar-search-hint");
    }
    let names: Vec<String> = [
        (fields.subject, fl!(LANGUAGE_LOADER, "search-field-subject")),
        (fields.from, fl!(LANGUAGE_LOADER, "search-field-from")),
        (fields.to, fl!(LANGUAGE_LOADER, "search-field-to")),
        (fields.text, fl!(LANGUAGE_LOADER, "search-field-text")),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    fl!(LANGUAGE_LOADER, "toolbar-search-hint-fields", fields = names.join(", "))
}

fn zoom_choices(ui: &mut egui::Ui, mode: &mut ScalingMode) {
    for zoom in settings::ZOOMS {
        if ui.add(egui::Button::selectable(*mode == zoom, settings::zoom_name(zoom))).clicked() {
            *mode = zoom;
            ui.close();
        }
    }
}
