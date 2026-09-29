use eframe::egui::{self, Key, Modifiers};
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, Position, Selection, Size, TextScreen};
use icy_engine_gui::{
    egui::{appearance, screen::ScreenView},
    ScalingMode,
};
use icy_mail::{
    drafts::{Draft, DraftKind},
    editor,
    options::ReadingMode,
    reader::{NavigateDirection, Pane},
    LANGUAGE_LOADER,
};

use super::{
    app::{draft_title, Folder, MailApp, Modal},
    chrome,
    list::{self, display_date},
    modern_view::Document,
    widgets::{self, Icon},
};

/// Space between the message text and the edges of the reading pane.
const BODY_PADDING: egui::Vec2 = egui::vec2(12.0, 8.0);

#[derive(Default)]
pub struct BodyHighlights {
    key: Option<(u64, String, bool)>,
    original: Vec<(Position, AttributedChar)>,
}

impl BodyHighlights {
    pub fn update(&mut self, view: &mut ScreenView, query: &str, dark_mode: bool) -> Result<(), String> {
        let key = (view.shader_state.instance_id, query.trim().to_lowercase(), dark_mode);
        if self.key.as_ref() == Some(&key) {
            return Ok(());
        }
        let mut screen = view.terminal.screen.lock();
        let screen = screen.as_editable().ok_or("Message highlighting requires an editable text screen")?;
        // Only restore cells on the same rendered message, never on its replacement.
        if self.key.as_ref().is_some_and(|previous| previous.0 == key.0) {
            for &(position, ch) in &self.original {
                screen.set_char(position, ch);
            }
        }
        self.original.clear();
        if !key.1.is_empty() {
            let background = widgets::search_highlight_color(dark_mode);
            for row in 0..screen.height() {
                let line: String = (0..screen.width())
                    .map(|column| screen.buffer_type().convert_to_unicode(screen.char_at((column, row).into()).ch))
                    .collect();
                for found in icy_mail::text::find_ignore_case(&line, &key.1) {
                    let start = line[..found.start].chars().count() as i32;
                    let end = start + line[found].chars().count() as i32;
                    for column in start..end {
                        let position = Position::new(column, row);
                        let mut ch = screen.char_at(position);
                        self.original.push((position, ch));
                        ch.attribute.set_foreground_rgb(0, 0, 0);
                        ch.attribute.set_background_rgb(background.r(), background.g(), background.b());
                        ch.attribute.set_is_blinking(false);
                        ch.attribute.set_is_concealed(false);
                        screen.set_char(position, ch);
                    }
                }
            }
        }
        self.key = Some(key);
        Ok(())
    }
}

impl MailApp {
    pub fn content(&mut self, ui: &mut egui::Ui) {
        if self.folder == Folder::Drafts {
            self.draft_preview(ui);
            return;
        }
        if self.folder == Folder::Bulletins {
            self.file_preview(ui);
            return;
        }
        let context = ui.ctx().clone();
        let Some(info) = self.selected_info().cloned() else {
            let image = self.icons.image(&context, Icon::Mailbox, 44.0);
            widgets::empty(
                ui,
                image,
                &fl!(LANGUAGE_LOADER, "reader-no-message-selected"),
                &fl!(LANGUAGE_LOADER, "reader-pick-message-detail"),
            );
            return;
        };
        let fields = self.reader.search_fields;
        let (subject_needle, from_needle, to_needle) = (
            self.search_needle(fields.subject),
            self.search_needle(fields.from),
            self.search_needle(fields.to),
        );
        let compact = ui.available_height() < 220.0;
        let minimal = ui.available_height() < 160.0;
        let conference = self.folder_name(Folder::Conference(info.conference));
        let parent = if info.ref_number == 0 {
            None
        } else {
            self.reader.find(info.conference, info.ref_number)
        };
        let unread = !self.reader.is_read(info.index);
        let mut navigate = None;
        let mut open_parent = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 14,
                right: 8,
                top: if compact { 2 } else { 6 },
                bottom: if compact { 2 } else { 6 },
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let actions = 7.0 * 32.0 + 18.0;
                    let width = (ui.available_width() - actions).max(40.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(width);
                        let size = if compact { 14.0 } else { 16.0 };
                        let mut job = widgets::header_job(ui, &info.subject, "", egui::FontId::proportional(size), ui.visuals().strong_text_color(), true);
                        widgets::highlight(&mut job, 0, &subject_needle, ui);
                        ui.add(egui::Label::new(job).truncate()).on_hover_text(info.subject.as_str());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let position = self.reader.selected_position();
                        if self
                            .icons
                            .button(
                                ui,
                                Icon::Down,
                                &fl!(LANGUAGE_LOADER, "reader-next-message"),
                                position.is_some_and(|position| position + 1 < self.reader.messages.len()),
                            )
                            .clicked()
                        {
                            navigate = Some(NavigateDirection::Down);
                        }
                        if self
                            .icons
                            .button(
                                ui,
                                Icon::Up,
                                &fl!(LANGUAGE_LOADER, "reader-previous-message"),
                                position.is_some_and(|position| position > 0),
                            )
                            .clicked()
                        {
                            navigate = Some(NavigateDirection::Up);
                        }
                        ui.add_space(6.0);
                        let more = self.icons.button(ui, Icon::More, &fl!(LANGUAGE_LOADER, "reader-more-actions"), true);
                        egui::Popup::menu(&more).show(|ui| self.message_actions_menu(ui));
                        ui.add_space(6.0);
                        let starred = self.reader.is_starred(info.index);
                        let tooltip = if starred {
                            fl!(LANGUAGE_LOADER, "reader-unstar-short")
                        } else {
                            fl!(LANGUAGE_LOADER, "reader-star-short")
                        };
                        if self.icons.star_button(ui, starred, &tooltip).clicked() {
                            self.set_starred(&context, info.index, !starred);
                        }
                        let (icon, tooltip) = if unread {
                            (Icon::Read, fl!(LANGUAGE_LOADER, "reader-mark-as-read-short"))
                        } else {
                            (Icon::Unread, fl!(LANGUAGE_LOADER, "reader-mark-as-unread-short"))
                        };
                        if self.icons.button(ui, icon, &tooltip, true).clicked() {
                            self.toggle_read(&context);
                        }
                        if self
                            .icons
                            .button(ui, Icon::Forward, &fl!(LANGUAGE_LOADER, "reader-forward-short"), true)
                            .clicked()
                        {
                            self.reply(&context, true);
                        }
                        if self.icons.button(ui, Icon::Reply, &fl!(LANGUAGE_LOADER, "reader-reply-short"), true).clicked() {
                            self.reply(&context, false);
                        }
                    });
                });
                if minimal {
                    return;
                }
                if compact {
                    let color = ui.visuals().weak_text_color();
                    let font = egui::FontId::proportional(12.0);
                    let mut job = egui::text::LayoutJob::default();
                    widgets::append_header(&mut job, ui, &info.from, "", font.clone(), color, false);
                    widgets::highlight(&mut job, 0, &from_needle, ui);
                    job.append(
                        " \u{2192} ",
                        0.0,
                        egui::TextFormat {
                            font_id: font.clone(),
                            color,
                            ..Default::default()
                        },
                    );
                    let start = job.text.len();
                    widgets::append_header(&mut job, ui, &info.to, "", font.clone(), color, false);
                    widgets::highlight(&mut job, start, &to_needle, ui);
                    let suffix = format!("  \u{00b7}  {}  \u{00b7}  {conference}", info.date_str);
                    job.append(
                        &suffix,
                        0.0,
                        egui::TextFormat {
                            font_id: font,
                            color,
                            ..Default::default()
                        },
                    );
                    let details = format!("{} \u{2192} {}  \u{00b7}  {}  \u{00b7}  {conference}", info.from, info.to, info.date_str);
                    ui.add(egui::Label::new(job).truncate()).on_hover_text(details);
                    return;
                }
                // Sender, recipient and details share one line; narrow panes split them into two.
                let parent_visible = parent.is_some_and(|index| self.reader.contains(index));
                let people = |ui: &mut egui::Ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    widgets::avatar(ui, &info.from, 20.0);
                    ui.add_space(2.0);
                    let mut from = widgets::header_job(ui, &info.from, "", egui::FontId::proportional(13.0), ui.visuals().strong_text_color(), true);
                    widgets::highlight(&mut from, 0, &from_needle, ui);
                    ui.label(from);
                    ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "reader-to")).weak().size(12.5));
                    let mut to = widgets::header_job(ui, &info.to, "", egui::FontId::proportional(13.0), ui.visuals().text_color(), false);
                    widgets::highlight(&mut to, 0, &to_needle, ui);
                    ui.label(to);
                    if info.private {
                        ui.add_space(2.0);
                        appearance::status_badge(ui, &fl!(LANGUAGE_LOADER, "reader-private"), widgets::warning(ui));
                    }
                };
                let details = |ui: &mut egui::Ui| -> bool {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let weak = |text: String| egui::RichText::new(text).weak().size(12.0);
                    ui.label(weak(format!("{}  \u{00b7}  {conference}  \u{00b7}  #{}", info.date_str, info.number)));
                    if info.ref_number == 0 {
                        return false;
                    }
                    ui.label(weak("\u{00b7}".into()));
                    let text = fl!(LANGUAGE_LOADER, "reader-reply-to", number = info.ref_number);
                    if !parent_visible {
                        ui.label(weak(text));
                        return false;
                    }
                    let link = egui::RichText::new(text).size(12.0).color(widgets::accent(ui));
                    ui.add(egui::Label::new(link).sense(egui::Sense::click()))
                        .on_hover_text(fl!(LANGUAGE_LOADER, "reader-show-original-message"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                };
                ui.add_space(2.0);
                ui.spacing_mut().interact_size.y = 22.0;
                ui.spacing_mut().item_spacing.y = 2.0;
                if ui.available_width() >= 620.0 {
                    ui.horizontal(|ui| {
                        people(ui);
                        ui.add_space(10.0);
                        open_parent = details(ui);
                    });
                } else {
                    ui.horizontal(people);
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        open_parent = details(ui);
                    });
                }
            });
        let separator = ui.visuals().widgets.noninteractive.bg_stroke;
        let top = ui.cursor().top();
        ui.painter().hline(ui.max_rect().x_range(), top, separator);
        if let Some(direction) = navigate {
            self.reader.navigate(Pane::Messages, direction);
            self.reveal_message = true;
        }
        if open_parent {
            if let Some(index) = parent {
                self.reader.select_message(index);
                self.reveal_message = true;
            }
        }
        if self.body_loading || self.rendered != self.reader.selected_message {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }
        let modern = self.reading_mode == ReadingMode::Modern;
        let response = self.document_body(ui, Document::Message(info.index));
        response.context_menu(|ui| {
            // The modern view copies its own text selection with Ctrl+C.
            if !modern && ui.button(fl!(LANGUAGE_LOADER, "reader-copy")).clicked() {
                self.copy(ui.ctx());
                ui.close();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-copy-message")).clicked() {
                self.copy_message(ui.ctx());
                ui.close();
            }
            ui.separator();
            if ui.button(fl!(LANGUAGE_LOADER, "reader-reply")).clicked() {
                self.reply(ui.ctx(), false);
                ui.close();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-forward")).clicked() {
                self.reply(ui.ctx(), true);
                ui.close();
            }
        });
    }

    /// Less frequent message actions behind the header's "more" button.
    fn message_actions_menu(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let entry = |ui: &mut egui::Ui, image: egui::Image<'static>, label: String, shortcut: String, enabled: bool, tooltip: Option<String>| {
            let button = egui::Button::image_and_text(image, label)
                .image_tint_follows_text_color(true)
                .shortcut_text(egui::RichText::new(shortcut).weak());
            let mut response = ui.add_enabled(enabled, button);
            if let Some(tooltip) = tooltip {
                response = response.on_hover_text(tooltip.clone()).on_disabled_hover_text(tooltip);
            }
            let clicked = response.clicked();
            if clicked {
                ui.close();
            }
            clicked
        };
        let copy = self.icons.image(&context, Icon::Copy, 16.0);
        if entry(
            ui,
            copy,
            fl!(LANGUAGE_LOADER, "reader-menu-copy-message-text"),
            String::new(),
            !self.body_loading,
            None,
        ) {
            self.copy_message(&context);
        }
        let tagline = self.message_tagline();
        let tooltip = tagline.as_ref().map_or_else(
            || fl!(LANGUAGE_LOADER, "reader-no-tagline"),
            |tagline| fl!(LANGUAGE_LOADER, "reader-save-tagline", tagline = tagline.as_str()),
        );
        let tag = self.icons.image(&context, Icon::Tag, 16.0);
        let shortcut = chrome::shortcut(&context, Modifiers::NONE, Key::T);
        if entry(
            ui,
            tag,
            fl!(LANGUAGE_LOADER, "reader-menu-save-tagline"),
            shortcut,
            tagline.is_some(),
            Some(tooltip),
        ) {
            self.save_tagline(&context);
        }
        let add = self.icons.image(&context, Icon::PersonAdd, 16.0);
        let shortcut = chrome::shortcut(&context, Modifiers::SHIFT, Key::A);
        if entry(ui, add, fl!(LANGUAGE_LOADER, "reader-menu-add-author"), shortcut, true, None) {
            self.add_sender(&context);
        }
        ui.separator();
        let saving = !self.loader.save_picking;
        for (utf8, label) in [
            (false, fl!(LANGUAGE_LOADER, "menu-save-message")),
            (true, fl!(LANGUAGE_LOADER, "menu-save-message-utf8")),
        ] {
            let image = self.icons.image(&context, Icon::Export, 16.0);
            if entry(ui, image, label, String::new(), saving, None) {
                self.save_message(&context, utf8);
            }
        }
    }

    /// The message terminal with focus handling and mouse selection.
    pub(super) fn terminal_body(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let query = if self.folder.holds_messages() && self.reader.search_fields.text {
            &self.reader.filter
        } else {
            ""
        };
        if let Err(error) = self.body_highlights.update(&mut self.screen, query, ui.visuals().dark_mode) {
            self.error = Some(error.to_string());
        }
        if let Some(mode) = icy_engine_gui::egui::zoom::mouse_wheel(
            ui,
            ui.is_enabled(),
            self.screen.zoom,
            self.settings.use_integer_scaling,
            ScalingMode::MIN_ZOOM..=ScalingMode::MAX_ZOOM,
        ) {
            self.settings.scaling_mode = mode;
            self.options_save_after = Some(ui.input(|input| input.time) + 0.25);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
        // Pad the text away from the pane edges; the margin keeps the terminal's background color.
        let area = ui.available_rect_before_wrap();
        let (red, green, blue) = self.screen.terminal.screen.lock().palette().rgb(0);
        ui.painter().rect_filled(area, 0.0, egui::Color32::from_rgb(red, green, blue));
        *self.screen.terminal.background_color.write() = [red, green, blue, 255].map(|channel| f32::from(channel) / 255.0);
        let inner = egui::Rect::from_min_max(
            area.min + egui::vec2(BODY_PADDING.x, BODY_PADDING.y),
            egui::pos2((area.max.x - BODY_PADDING.x).max(area.min.x + 1.0), area.max.y),
        );
        let response = ui
            .scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.screen.show(ui, &self.settings))
            .inner;
        ui.advance_cursor_after_rect(area);
        self.content_rect = response.rect;
        if response.clicked() || response.drag_started() {
            self.set_focus(Pane::Content, ui.ctx());
        }
        if ui.ctx().will_discard() {
            return response;
        }
        if !ui.is_enabled() {
            self.selection_anchor = None;
            self.last_reader_click = None;
            return response;
        }
        if !response.hovered() && ui.input(|input| input.pointer.primary_pressed()) {
            self.last_reader_click = None;
        }
        if response.hovered() && ui.input(|input| input.pointer.primary_pressed()) {
            self.selection_anchor = ui.input(|input| input.pointer.press_origin()).and_then(|pos| self.cell(pos)).map(|anchor| {
                let mut selection = if ui.input(|input| input.modifiers.shift) {
                    self.screen.terminal.screen.lock().selection().unwrap_or_else(|| Selection::new(anchor))
                } else {
                    Selection::new(anchor)
                };
                selection.shape = if ui.input(|input| input.modifiers.alt) {
                    icy_engine::Shape::Rectangle
                } else {
                    icy_engine::Shape::Lines
                };
                selection
            });
            if !ui.input(|input| input.modifiers.shift) {
                if let Err(error) = self.screen.terminal.screen.lock().clear_selection() {
                    self.error = Some(error.to_string());
                }
                ui.ctx().request_repaint();
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) || response.drag_stopped_by(egui::PointerButton::Primary) {
            self.last_reader_click = None;
            if let (Some(mut selection), Some(lead)) = (
                self.selection_anchor,
                ui.input(|input| input.pointer.interact_pos()).and_then(|pos| self.cell(pos)),
            ) {
                selection.lead = lead;
                if let Err(error) = self.screen.terminal.screen.lock().set_selection(selection) {
                    self.error = Some(error.to_string());
                }
                ui.ctx().request_repaint();
            }
        } else if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                let Some(position) = self.cell(pointer) else { return response };
                let time = ui.input(|input| input.time);
                let options = ui.ctx().options(|options| options.input_options);
                // egui's click count is time-only and shared with other widgets.
                let count = self.last_reader_click.map_or(1, |(previous, when, count)| {
                    if pointer.distance(previous) <= options.max_click_dist && time - when < options.max_double_click_delay {
                        (count + 1).min(3)
                    } else {
                        1
                    }
                });
                self.last_reader_click = Some((pointer, time, count));
                let mut screen = self.screen.terminal.screen.lock();
                let selection = if ui.input(|input| input.modifiers.shift) {
                    screen.selection().map(|mut selection| {
                        selection.lead = position;
                        selection
                    })
                } else if count == 3 {
                    let mut selection = Selection::new((0, position.y));
                    selection.lead = (screen.width() - 1, position.y).into();
                    Some(selection)
                } else if count == 2 {
                    let is_space = |column| {
                        screen
                            .buffer_type()
                            .convert_to_unicode(screen.char_at((column, position.y).into()).ch)
                            .is_whitespace()
                    };
                    let whitespace = is_space(position.x);
                    let mut left = position.x;
                    let mut right = position.x;
                    while left > 0 && is_space(left - 1) == whitespace {
                        left -= 1;
                    }
                    while right + 1 < screen.width() && is_space(right + 1) == whitespace {
                        right += 1;
                    }
                    let mut selection = Selection::new((left, position.y));
                    selection.lead = (right, position.y).into();
                    Some(selection)
                } else {
                    None
                };
                let result = match selection {
                    Some(selection) => screen.set_selection(selection),
                    None => screen.clear_selection(),
                };
                if let Err(error) = result {
                    self.error = Some(error.to_string());
                }
                ui.ctx().request_repaint();
            }
        }
        if !ui.input(|input| input.pointer.primary_down()) {
            self.selection_anchor = None;
        }
        response
    }

    /// Renders the draft into the message terminal, like received messages.
    fn render_draft(&mut self, draft: &Draft) {
        let text = draft.text();
        if self.rendered_draft.as_ref().is_some_and(|(id, body)| *id == draft.id && *body == text) {
            return;
        }
        let screen = icy_mail::reader::render_body(&editor::encode_message(&text)).unwrap_or_else(|_| TextScreen::new(Size::new(80, 25)));
        self.screen = ScreenView::new(screen);
        // A body still loading for the message list must not replace the draft.
        self.loader.body_generation = self.loader.body_generation.wrapping_add(1);
        self.body_loading = false;
        self.rendered = None;
        self.rendered_file = None;
        self.selection_anchor = None;
        self.last_reader_click = None;
        self.rendered_draft = Some((draft.id, text));
    }

    /// A bulletin, news or new files screen of the packet.
    fn file_preview(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let Some(file) = self.selected_file().cloned() else {
            return;
        };
        let compact = ui.available_height() < 220.0;
        let mut navigate = None;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 14,
                right: 8,
                top: if compact { 2 } else { 8 },
                bottom: if compact { 2 } else { 8 },
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let actions = 3.0 * 32.0 + 12.0;
                    let width = (ui.available_width() - actions).max(40.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(width);
                        let title = appearance::bold(ui, list::file_title(&file)).size(if compact { 14.0 } else { 17.0 });
                        ui.add(egui::Label::new(title).truncate());
                        ui.add(egui::Label::new(egui::RichText::new(&file.name).weak().size(12.0)).truncate());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let position = self.selected_file.unwrap_or(0);
                        if self
                            .icons
                            .button(ui, Icon::Down, &fl!(LANGUAGE_LOADER, "reader-next-file"), position + 1 < self.file_count())
                            .clicked()
                        {
                            navigate = Some(NavigateDirection::Down);
                        }
                        if self
                            .icons
                            .button(ui, Icon::Up, &fl!(LANGUAGE_LOADER, "reader-previous-file"), position > 0)
                            .clicked()
                        {
                            navigate = Some(NavigateDirection::Up);
                        }
                        ui.add_space(6.0);
                        if self
                            .icons
                            .button(ui, Icon::Copy, &fl!(LANGUAGE_LOADER, "reader-copy-file-text"), !self.body_loading)
                            .clicked()
                        {
                            self.copy_message(&context);
                        }
                    });
                });
            });
        let separator = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), separator);
        if let Some(direction) = navigate {
            let files = self.file_count();
            self.selected_file = self.selected_file.map(|position| icy_mail::reader::step(position, direction, files));
            self.selected_file_page = 0;
            self.reveal_message = true;
        }
        if self.body_loading || self.rendered_file != self.selected_file || self.rendered_file_page != self.selected_file_page {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }
        let modern = self.reading_mode == ReadingMode::Modern;
        let document = Document::File(self.selected_file.unwrap_or(0), self.selected_file_page);
        let response = self.document_body(ui, document);
        if navigate.is_none() && ui.is_enabled() && response.hovered() && !ui.ctx().will_discard() {
            let wheel = ui.input(|input| input.raw_scroll_delta.y);
            if wheel != 0.0 {
                if wheel < 0.0 && self.screen.offset.y + 1.0 >= self.screen.max_offset.y {
                    self.change_file_page(true);
                } else if wheel > 0.0 && self.screen.offset.y <= 1.0 {
                    self.change_file_page(false);
                }
            }
        }
        response.context_menu(|ui| {
            // The modern view copies its own text selection with Ctrl+C.
            if !modern && ui.button(fl!(LANGUAGE_LOADER, "reader-copy")).clicked() {
                self.copy(ui.ctx());
                ui.close();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-copy-file-text")).clicked() {
                self.copy_message(ui.ctx());
                ui.close();
            }
        });
    }

    fn draft_preview(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let Some(store) = &self.drafts else {
            return;
        };
        let Some(draft) = store.drafts().iter().find(|draft| Some(draft.id) == self.selected_draft).cloned() else {
            let image = self.icons.image(&context, Icon::Drafts, 44.0);
            widgets::empty(
                ui,
                image,
                &fl!(LANGUAGE_LOADER, "reader-no-draft-selected"),
                &fl!(LANGUAGE_LOADER, "reader-no-draft-detail"),
            );
            return;
        };
        let mut issues: Vec<String> = store.issues(&draft).into_iter().map(|issue| issue.message).collect();
        issues.dedup();
        let conference = self.choices.iter().find(|(number, _)| *number == draft.conference).map_or_else(
            || fl!(LANGUAGE_LOADER, "reader-conference-number", number = draft.conference),
            |(_, name)| name.clone(),
        );
        let mut edit = false;
        let mut delete = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 14,
                right: 10,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let kind = match draft.kind {
                        DraftKind::New => fl!(LANGUAGE_LOADER, "reader-kind-new-message"),
                        DraftKind::Reply => fl!(LANGUAGE_LOADER, "reader-kind-reply"),
                        DraftKind::Forward => fl!(LANGUAGE_LOADER, "reader-kind-forward"),
                    };
                    appearance::chip(ui, &kind, widgets::accent(ui));
                    let width = (ui.available_width() - 150.0).max(40.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(width);
                        ui.add(egui::Label::new(appearance::bold(ui, draft_title(&draft)).size(17.0)).truncate());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        delete = self
                            .icons
                            .button(ui, Icon::Delete, &fl!(LANGUAGE_LOADER, "reader-delete-draft-short"), true)
                            .clicked();
                        edit = ui
                            .add(appearance::primary_button(fl!(LANGUAGE_LOADER, "reader-edit")))
                            .on_hover_text(fl!(LANGUAGE_LOADER, "reader-edit-draft-short"))
                            .clicked();
                    });
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let weak = |text: String| egui::RichText::new(text).weak().size(12.5);
                    ui.label(weak(fl!(LANGUAGE_LOADER, "reader-to-capital")));
                    let recipient = if draft.to.trim().is_empty() {
                        fl!(LANGUAGE_LOADER, "reader-no-recipient")
                    } else {
                        draft.to.clone()
                    };
                    ui.label(recipient);
                    ui.label(weak(fl!(LANGUAGE_LOADER, "reader-from")));
                    ui.label(&draft.from);
                    ui.label(weak(format!("\u{00b7}  {conference}  \u{00b7}  {}", display_date(&draft.date))));
                    if draft.private {
                        ui.add_space(6.0);
                        appearance::status_badge(ui, &fl!(LANGUAGE_LOADER, "reader-private"), widgets::warning(ui));
                    }
                });
                if !issues.is_empty() {
                    ui.add_space(6.0);
                    issue_box(ui, &mut self.icons, &issues);
                }
            });
        let separator = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), separator);
        self.render_draft(&draft);
        let modern = self.reading_mode == ReadingMode::Modern;
        let text = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            draft.text().hash(&mut hasher);
            hasher.finish()
        };
        let response = self.document_body(ui, Document::Draft(draft.id, text));
        response.context_menu(|ui| {
            if !modern && ui.button(fl!(LANGUAGE_LOADER, "reader-copy")).clicked() {
                self.copy(ui.ctx());
                ui.close();
            }
            if !modern {
                ui.separator();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-edit")).clicked() {
                edit = true;
                ui.close();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-delete")).clicked() {
                delete = true;
                ui.close();
            }
        });
        if edit {
            self.edit_draft(&context, draft.id);
        }
        if delete {
            self.modal = Some(Modal::DeleteDraft(draft.id));
        }
    }
}

/// Warning panel listing what keeps a draft from being exported.
pub fn issue_box(ui: &mut egui::Ui, icons: &mut widgets::Icons, issues: &[String]) {
    let color = widgets::warning(ui);
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.5)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.add(icons.image(ui.ctx(), Icon::Warning, 16.0).tint(color));
                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "reader-fix-before-export")).color(color).strong());
            });
            for issue in issues {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(22.0);
                    ui.label(egui::RichText::new(issue).size(12.5));
                });
            }
        });
}
