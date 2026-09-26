//! SlyEdit-style message editor: the text is edited on the ANSI terminal renderer,
//! colors, the CP437 table, find and quoting are regular egui panels around it.

use std::hash::{DefaultHasher, Hash, Hasher};

use eframe::egui::{self, emath::GuiRounding, Color32, Key};
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, CaretShape, EditableScreen, IceMode, Position, Size, TextAttribute, TextScreen};
use icy_engine_gui::{egui::screen::ScreenView, MonitorSettings};
use icy_mail::{
    editor::{self, Attr, Editor, Motion, Pos},
    LANGUAGE_LOADER,
};

use super::widgets::{pill, Icon, Icons};

pub const COLUMNS: usize = 80;
const MIN_ROWS: usize = 6;
const FIND_ID: &str = "editor-find";

const BAR: Attr = Attr::new(7, 1, false);
const BAR_KEY: Attr = Attr::new(14, 1, false);
const BAR_TEXT: Attr = Attr::new(15, 1, false);

/// The CP437 table beside (or below) the text.
pub struct CharTable {
    pub open: bool,
    /// Arrow keys move through the table instead of the text.
    pub picking: bool,
    pub code: u8,
    /// Where the table was drawn last.
    pub grid: egui::Rect,
}

/// Lines of the original message that can be quoted, like SlyEdit's quote window.
#[derive(Default)]
pub struct Quotes {
    pub lines: Vec<String>,
    pub open: bool,
    pub selected: usize,
    reveal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Colors,
    Chars,
    Quote,
    Find,
    Insert,
    Help,
}

#[derive(Default)]
pub struct EditorOutput {
    /// The message text changed.
    pub changed: bool,
    pub help: bool,
}

pub struct TerminalEditor {
    pub editor: Editor,
    view: ScreenView,
    rows: usize,
    top: usize,
    follow: bool,
    /// Pending colors while the color picker is open.
    pub colors: Option<Attr>,
    pub chars: CharTable,
    pub quotes: Quotes,
    /// Search text while the find bar is open.
    pub find: Option<String>,
    find_focus: bool,
    find_focused: bool,
    last_find: String,
    pub status: Option<String>,
    drawn: Option<u64>,
    edits: u64,
    focused: bool,
    focus_request: bool,
    id: Option<egui::Id>,
    rect: egui::Rect,
    zoom: f32,
    dragging: bool,
    wheel: f32,
    hotspots: Vec<(i32, i32, Command)>,
    palette: [Color32; 16],
    glyphs: Option<egui::TextureHandle>,
    color_button: egui::Rect,
}

impl TerminalEditor {
    pub fn new(body: &str, quotes: Vec<String>, open_quotes: bool) -> Self {
        let editor = Editor::from_body(body);
        let view = screen_view(MIN_ROWS);
        let palette = {
            let screen = view.terminal.screen.lock();
            std::array::from_fn(|index| {
                let (r, g, b) = screen.palette().rgb(index as u32);
                Color32::from_rgb(r, g, b)
            })
        };
        let mut this = Self {
            edits: editor.edit_count(),
            editor,
            view,
            rows: MIN_ROWS,
            top: 0,
            follow: true,
            colors: None,
            chars: CharTable {
                open: false,
                picking: false,
                code: 0xb0,
                grid: egui::Rect::NOTHING,
            },
            quotes: Quotes {
                open: open_quotes && !quotes.is_empty(),
                lines: quotes,
                ..Default::default()
            },
            find: None,
            find_focus: false,
            find_focused: false,
            last_find: String::new(),
            status: None,
            drawn: None,
            focused: false,
            focus_request: false,
            id: None,
            rect: egui::Rect::NOTHING,
            zoom: 1.0,
            dragging: false,
            wheel: 0.0,
            hotspots: Vec::new(),
            palette,
            glyphs: None,
            color_button: egui::Rect::NOTHING,
        };
        this.redraw();
        this
    }

    pub fn request_focus(&mut self) {
        self.focus_request = true;
    }

    #[cfg(test)]
    pub fn has_focus(&self) -> bool {
        self.focused
    }

    /// Whether Escape closes something inside the editor rather than the composer.
    pub fn captures_escape(&self) -> bool {
        self.colors.is_some() || self.chars.open || self.quotes.open || self.find.is_some() || self.editor.selection().is_some()
    }

    fn text_rows(&self) -> usize {
        self.rows - 1
    }

    pub fn show(&mut self, ui: &mut egui::Ui, settings: &MonitorSettings, icons: &mut Icons, enabled: bool) -> EditorOutput {
        let mut output = EditorOutput::default();
        let context = ui.ctx().clone();
        self.focused = enabled && self.id.is_some_and(|id| context.memory(|memory| memory.has_focus(id)));
        if self.focused {
            self.keyboard(&context);
        }

        self.toolbar(ui, icons, &mut output);
        if self.find.is_some() {
            self.find_bar(ui, icons);
        }
        ui.add_space(4.0);
        let wide = ui.available_width() >= 760.0;
        if self.chars.open {
            let frame = egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 4));
            if wide {
                egui::SidePanel::right("editor-chars")
                    .resizable(false)
                    .exact_width(CHAR_PANEL_WIDTH)
                    .frame(frame)
                    .show_inside(ui, |ui| self.char_panel(ui, icons));
            } else {
                egui::TopBottomPanel::bottom("editor-chars")
                    .resizable(false)
                    .exact_height(280.0)
                    .frame(frame)
                    .show_inside(ui, |ui| self.char_panel(ui, icons));
            }
        }
        if self.quotes.open {
            let height = ui.available_height();
            egui::TopBottomPanel::bottom("editor-quotes")
                .resizable(true)
                .default_height((height * 0.38).max(120.0))
                .height_range(90.0..=(height - 120.0).max(90.0))
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)))
                .show_inside(ui, |ui| self.quote_panel(ui, icons));
        }
        self.terminal(ui, settings, enabled, &mut output);
        if enabled && self.colors.is_some() {
            self.color_picker(&context);
        }

        let edits = self.editor.edit_count();
        output.changed = edits != self.edits;
        self.edits = edits;
        output
    }

    /// Takes the keyboard input meant for the text while the terminal has the focus.
    fn keyboard(&mut self, context: &egui::Context) {
        let Some(id) = self.id else {
            return;
        };
        context.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
        if egui::Popup::is_any_open(context) {
            return;
        }
        let events = context.input_mut(|input| {
            let mut taken = Vec::new();
            input.events.retain(|event| {
                let take = matches!(
                    event,
                    egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Copy | egui::Event::Cut | egui::Event::Key { pressed: true, .. }
                );
                if take {
                    taken.push(event.clone());
                }
                !take
            });
            taken
        });
        for event in events {
            self.event(context, event);
        }
    }

    fn font(&self) -> egui::Vec2 {
        let size = self.view.terminal.screen.lock().font_dimensions();
        egui::vec2(size.width.max(1) as f32, size.height.max(1) as f32)
    }

    fn event(&mut self, context: &egui::Context, event: egui::Event) {
        match event {
            egui::Event::Text(text) => {
                self.status = None;
                self.text(&text);
            }
            egui::Event::Paste(text) => {
                self.status = None;
                if self.colors.is_none() {
                    let replaced = self.editor.insert_text(&text);
                    if replaced > 0 {
                        self.status = Some(fl!(LANGUAGE_LOADER, "editor-paste-replaced", count = replaced));
                    }
                    self.follow = true;
                }
            }
            egui::Event::Copy => {
                if let Some(text) = self.editor.selected_text() {
                    context.copy_text(text);
                }
            }
            egui::Event::Cut => {
                if let Some(text) = self.editor.cut() {
                    context.copy_text(text);
                    self.follow = true;
                }
            }
            egui::Event::Key { key, modifiers, .. } => {
                self.status = None;
                self.key(key, modifiers);
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if let Some(pending) = &mut self.colors {
            if let Some(fg) = text.chars().last().and_then(|ch| ch.to_digit(16)) {
                pending.fg = fg as u8;
            }
            return;
        }
        self.chars.picking = false;
        self.quotes.open = false;
        for ch in text.chars() {
            if !self.editor.type_char(ch) && !ch.is_control() {
                self.status = Some(fl!(LANGUAGE_LOADER, "editor-character-not-cp437", character = ch.to_string()));
            }
        }
        self.follow = true;
    }

    fn key(&mut self, key: Key, modifiers: egui::Modifiers) {
        let command = modifiers.command || modifiers.ctrl;
        if self.colors.is_some() {
            return self.color_key(key);
        }
        if self.chars.picking && self.char_key(key, command) {
            return;
        }
        if self.quotes.open && self.quote_key(key, command) {
            return;
        }
        let shift = modifiers.shift;
        if command {
            match key {
                Key::K => self.open_colors(),
                Key::G => self.toggle_chars(true),
                Key::Q => self.toggle_quotes(),
                Key::F => self.open_find(),
                Key::D => self.editor.delete_line(),
                Key::Z if shift => {
                    self.editor.redo();
                }
                Key::Z => {
                    self.editor.undo();
                }
                Key::Y => {
                    self.editor.redo();
                }
                Key::A => self.editor.select_all(),
                Key::Home => self.editor.move_caret(Motion::DocumentStart, shift),
                Key::End => self.editor.move_caret(Motion::DocumentEnd, shift),
                Key::ArrowLeft => self.editor.move_caret(Motion::WordLeft, shift),
                Key::ArrowRight => self.editor.move_caret(Motion::WordRight, shift),
                Key::ArrowUp => self.top = self.top.saturating_sub(1),
                Key::ArrowDown => self.top += 1,
                Key::Backspace => self.editor.delete_word_back(),
                _ => return,
            }
            self.follow = !matches!(key, Key::ArrowUp | Key::ArrowDown);
            return;
        }
        let page = self.text_rows().saturating_sub(1).max(1);
        let motion = match key {
            Key::ArrowLeft => Some(Motion::Left),
            Key::ArrowRight => Some(Motion::Right),
            Key::ArrowUp => Some(Motion::Up),
            Key::ArrowDown => Some(Motion::Down),
            Key::Home => Some(Motion::Home),
            Key::End => Some(Motion::End),
            Key::PageUp => Some(Motion::PageUp(page)),
            Key::PageDown => Some(Motion::PageDown(page)),
            _ => None,
        };
        if let Some(motion) = motion {
            self.editor.move_caret(motion, shift);
        } else {
            match key {
                Key::Backspace => self.editor.backspace(),
                Key::Delete => self.editor.delete(),
                Key::Enter => self.editor.newline(),
                Key::Tab if !shift => self.editor.tab(),
                Key::Insert => self.editor.toggle_insert(),
                Key::F3 => self.find_next(),
                Key::Escape => self.escape(),
                _ => return,
            }
        }
        self.follow = true;
    }

    fn escape(&mut self) {
        if self.chars.open {
            self.chars.open = false;
        } else if self.find.is_some() {
            self.find = None;
        } else {
            self.editor.clear_selection();
        }
    }

    fn open_colors(&mut self) {
        self.colors = Some(self.editor.attr());
    }

    fn toggle_chars(&mut self, keyboard: bool) {
        if !self.chars.open {
            self.chars.open = true;
            self.chars.picking = keyboard;
        } else if keyboard && !self.chars.picking {
            self.chars.picking = true;
        } else {
            self.chars.open = false;
            self.chars.picking = false;
        }
    }

    fn toggle_quotes(&mut self) {
        if self.quotes.lines.is_empty() {
            self.status = Some(fl!(LANGUAGE_LOADER, "editor-no-original-to-quote"));
        } else {
            self.quotes.open = !self.quotes.open;
            self.quotes.reveal = true;
        }
    }

    fn open_find(&mut self) {
        if self.find.is_none() {
            self.find = Some(self.last_find.clone());
        }
        self.find_focus = true;
    }

    fn find_next(&mut self) {
        let query = self.find.clone().unwrap_or_else(|| self.last_find.clone());
        if query.is_empty() {
            self.open_find();
            return;
        }
        self.last_find.clone_from(&query);
        if !self.editor.find(&query) {
            self.status = Some(fl!(LANGUAGE_LOADER, "editor-find-not-found", query = query.as_str()));
        }
        self.follow = true;
    }

    fn color_key(&mut self, key: Key) {
        let Some(mut pending) = self.colors else {
            return;
        };
        match key {
            Key::ArrowLeft => pending.fg = (pending.fg + 15) % 16,
            Key::ArrowRight => pending.fg = (pending.fg + 1) % 16,
            Key::ArrowUp => pending.bg = (pending.bg + 7) % 8,
            Key::ArrowDown => pending.bg = (pending.bg + 1) % 8,
            Key::Home => pending.fg = 0,
            Key::End => pending.fg = 15,
            Key::Space => pending.blink = !pending.blink,
            Key::Delete | Key::Backspace => pending = Attr::DEFAULT,
            Key::Enter => return self.apply_color(pending),
            Key::Escape | Key::K => {
                self.colors = None;
                return;
            }
            _ => {}
        }
        self.colors = Some(pending);
    }

    fn apply_color(&mut self, attr: Attr) {
        self.editor.set_attr(attr);
        self.colors = None;
    }

    /// Keys while picking from the character table; returns whether the key was used.
    fn char_key(&mut self, key: Key, command: bool) -> bool {
        let code = self.chars.code;
        let code = match key {
            Key::G if command => {
                self.toggle_chars(true);
                return true;
            }
            _ if command => {
                self.chars.picking = false;
                return false;
            }
            Key::ArrowLeft => code.wrapping_sub(1),
            Key::ArrowRight => code.wrapping_add(1),
            Key::ArrowUp => code.wrapping_sub(16),
            Key::ArrowDown => code.wrapping_add(16),
            Key::Home => code & 0xf0,
            Key::End => code | 0x0f,
            Key::PageUp => code & 0x0f,
            Key::PageDown => code | 0xf0,
            Key::Enter => {
                self.chars.picking = false;
                self.insert_char(code);
                return true;
            }
            Key::Space => {
                self.insert_char(code);
                return true;
            }
            Key::Escape => {
                self.chars.picking = false;
                return true;
            }
            _ => {
                self.chars.picking = false;
                return false;
            }
        };
        self.chars.code = code;
        true
    }

    fn insert_char(&mut self, code: u8) {
        self.chars.code = code;
        if !editor::is_insertable(code) {
            self.status = Some(if code == 0xe3 {
                fl!(LANGUAGE_LOADER, "editor-qwk-separator-unusable")
            } else {
                fl!(LANGUAGE_LOADER, "editor-control-code-unusable", code = code, hex = format!("{code:02X}"))
            });
            return;
        }
        self.editor.insert_symbol(editor::cp437_char(code));
        self.follow = true;
    }

    fn quote_key(&mut self, key: Key, command: bool) -> bool {
        let count = self.quotes.lines.len();
        let selected = self.quotes.selected;
        match key {
            Key::Q if command => self.quotes.open = false,
            Key::A if command => {
                self.insert_quotes(selected..count);
                self.quotes.open = false;
            }
            _ if command => return false,
            Key::ArrowUp => self.quotes.selected = selected.saturating_sub(1),
            Key::ArrowDown => self.quotes.selected = (selected + 1).min(count - 1),
            Key::PageUp => self.quotes.selected = selected.saturating_sub(10),
            Key::PageDown => self.quotes.selected = (selected + 10).min(count - 1),
            Key::Home => self.quotes.selected = 0,
            Key::End => self.quotes.selected = count - 1,
            Key::Enter => self.insert_quotes(selected..selected + 1),
            Key::Escape => self.quotes.open = false,
            _ => return false,
        }
        self.quotes.reveal = true;
        true
    }

    fn insert_quotes(&mut self, range: std::ops::Range<usize>) {
        let count = self.quotes.lines.len();
        let range = range.start.min(count)..range.end.min(count);
        if range.is_empty() {
            return;
        }
        let end = range.end;
        self.editor.insert_lines(&self.quotes.lines[range]);
        self.quotes.selected = end.min(count - 1);
        self.quotes.reveal = true;
        self.follow = true;
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, icons: &mut Icons, output: &mut EditorOutput) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let attr = self.colors.unwrap_or(self.editor.attr());
            let response = color_button(
                ui,
                self.palette[usize::from(attr.fg)],
                self.palette[usize::from(attr.bg)],
                self.colors.is_some(),
            )
            .on_hover_text(fl!(LANGUAGE_LOADER, "editor-text-color-tooltip"));
            self.color_button = response.rect;
            if response.clicked() {
                if self.colors.is_some() {
                    self.colors = None;
                } else {
                    self.open_colors();
                }
                self.request_focus();
            }
            if pill(ui, self.chars.open, &fl!(LANGUAGE_LOADER, "editor-characters"))
                .on_hover_text(fl!(LANGUAGE_LOADER, "editor-characters-tooltip"))
                .clicked()
            {
                self.toggle_chars(false);
                self.request_focus();
            }
            let quote = ui
                .add_enabled_ui(!self.quotes.lines.is_empty(), |ui| {
                    pill(ui, self.quotes.open, &fl!(LANGUAGE_LOADER, "editor-quote"))
                })
                .inner
                .on_hover_text(fl!(LANGUAGE_LOADER, "editor-quote-tooltip"))
                .on_disabled_hover_text(fl!(LANGUAGE_LOADER, "editor-quote-disabled-tooltip"));
            if quote.clicked() {
                self.toggle_quotes();
                self.request_focus();
            }
            if pill(ui, self.find.is_some(), &fl!(LANGUAGE_LOADER, "editor-find"))
                .on_hover_text(fl!(LANGUAGE_LOADER, "editor-find-tooltip"))
                .clicked()
            {
                if self.find.is_some() {
                    self.find = None;
                    self.request_focus();
                } else {
                    self.open_find();
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons.button(ui, Icon::Info, &fl!(LANGUAGE_LOADER, "editor-keys-tooltip"), true).clicked() {
                    output.help = true;
                }
            });
        });
    }

    fn find_bar(&mut self, ui: &mut egui::Ui, icons: &mut Icons) {
        let context = ui.ctx().clone();
        let id = egui::Id::new(FIND_ID);
        let focused = context.memory(|memory| memory.has_focus(id));
        // egui drops the focus on Escape before widgets run, so ask about the previous frame.
        if std::mem::take(&mut self.find_focused) && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.find = None;
            self.request_focus();
            return;
        }
        // Taken before the field sees them, so it keeps the focus for the next search.
        let next =
            focused && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::F3) || input.consume_key(egui::Modifiers::NONE, Key::Enter));
        let Some(mut query) = self.find.take() else {
            return;
        };
        let focus = std::mem::take(&mut self.find_focus);
        let not_found = self.status.clone();
        let (mut search, mut close) = (next, false);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let response = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .id(id)
                    .hint_text(fl!(LANGUAGE_LOADER, "editor-find-in-message"))
                    .desired_width(260.0)
                    .char_limit(80),
            );
            if focus {
                response.request_focus();
            }
            self.find_focused = response.has_focus() || focus;
            if icons
                .button(ui, Icon::Down, &fl!(LANGUAGE_LOADER, "editor-find-next-tooltip"), !query.is_empty())
                .clicked()
            {
                search = true;
            }
            if icons.button(ui, Icon::Close, &fl!(LANGUAGE_LOADER, "editor-close-tooltip"), true).clicked() {
                close = true;
            }
            if let Some(status) = &not_found {
                ui.label(egui::RichText::new(status).color(super::widgets::warning(ui)));
            }
        });
        if close {
            self.request_focus();
            return;
        }
        self.find = Some(query);
        if search {
            self.status = None;
            self.find_next();
        }
    }

    fn quote_panel(&mut self, ui: &mut egui::Ui, icons: &mut Icons) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "editor-quote-from-original")).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons.button(ui, Icon::Close, &fl!(LANGUAGE_LOADER, "editor-close-tooltip"), true).clicked() {
                    self.quotes.open = false;
                    self.request_focus();
                }
                let count = self.quotes.lines.len();
                let selected = self.quotes.selected;
                if ui
                    .button(fl!(LANGUAGE_LOADER, "editor-quote-rest"))
                    .on_hover_text(fl!(LANGUAGE_LOADER, "editor-quote-rest-tooltip"))
                    .clicked()
                {
                    self.insert_quotes(selected..count);
                    self.quotes.open = false;
                    self.request_focus();
                }
                if ui
                    .button(fl!(LANGUAGE_LOADER, "editor-quote-line"))
                    .on_hover_text(fl!(LANGUAGE_LOADER, "editor-quote-line-tooltip"))
                    .clicked()
                {
                    self.insert_quotes(selected..selected + 1);
                    self.request_focus();
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let help = fl!(LANGUAGE_LOADER, "editor-quote-help");
                    ui.add(egui::Label::new(egui::RichText::new(help).size(11.5).color(ui.visuals().weak_text_color())).truncate());
                });
            });
        });
        ui.add_space(4.0);
        let reveal = std::mem::take(&mut self.quotes.reveal);
        let font = egui::FontId::monospace(13.0);
        let row_height = ui.fonts_mut(|fonts| fonts.row_height(&font)) + 4.0;
        let (mut clicked, mut double_clicked) = (None, None);
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(4)
            .inner_margin(4)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (index, line) in self.quotes.lines.iter().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), egui::Sense::click());
                        let selected = index == self.quotes.selected;
                        let visuals = ui.visuals();
                        let color = if selected {
                            ui.painter().rect_filled(rect, 3, visuals.selection.bg_fill);
                            visuals.selection.stroke.color
                        } else {
                            if response.hovered() {
                                ui.painter().rect_filled(rect, 3, visuals.widgets.hovered.weak_bg_fill);
                            }
                            visuals.text_color()
                        };
                        ui.painter()
                            .text(rect.left_center() + egui::vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, line, font.clone(), color);
                        if selected && reveal {
                            response.scroll_to_me(None);
                        }
                        if response.double_clicked() {
                            double_clicked = Some(index);
                        } else if response.clicked() {
                            clicked = Some(index);
                        }
                    }
                });
            });
        if let Some(index) = double_clicked {
            self.insert_quotes(index..index + 1);
            self.request_focus();
        } else if let Some(index) = clicked {
            self.quotes.selected = index;
            self.request_focus();
        }
    }

    fn char_panel(&mut self, ui: &mut egui::Ui, icons: &mut Icons) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "editor-characters")).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons.button(ui, Icon::Close, &fl!(LANGUAGE_LOADER, "editor-close-tooltip"), true).clicked() {
                    self.chars.open = false;
                    self.chars.picking = false;
                    self.request_focus();
                }
            });
        });
        ui.add_space(2.0);
        let glyphs = self.glyphs(ui.ctx());
        let cell = ((ui.available_width() / 16.0).floor())
            .min(((ui.available_height() - 64.0) / 16.0).floor())
            .clamp(12.0, 24.0);
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(cell * 16.0), egui::Sense::click());
        self.chars.grid = rect;
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect.expand(2.0));
        painter.rect_filled(rect, 4, visuals.extreme_bg_color);
        let hovered = response.hover_pos().map(|position| {
            let local = (position - rect.min) / cell;
            (local.y.clamp(0.0, 15.0) as u8) * 16 + local.x.clamp(0.0, 15.0) as u8
        });
        let font = self.font();
        let fit = (cell - 2.0) / font.x.max(font.y);
        let scale = if fit >= 1.0 { fit.floor() } else { fit.max(0.1) };
        for code in 0..=255u8 {
            let cell_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(f32::from(code % 16), f32::from(code / 16)) * cell,
                egui::Vec2::splat(cell),
            );
            let color = if code == self.chars.code {
                painter.rect_filled(cell_rect.shrink(0.5), 3, visuals.selection.bg_fill);
                visuals.selection.stroke.color
            } else {
                if hovered == Some(code) {
                    painter.rect_filled(cell_rect.shrink(0.5), 3, visuals.widgets.hovered.weak_bg_fill);
                }
                if editor::is_insertable(code) {
                    visuals.text_color()
                } else {
                    visuals.weak_text_color().gamma_multiply(0.45)
                }
            };
            paint_glyph(&painter, &glyphs, code, cell_rect.center(), font * scale, color);
        }
        if self.chars.picking {
            painter.rect_stroke(
                rect.expand(1.0),
                4,
                egui::Stroke::new(1.5, visuals.selection.stroke.color),
                egui::StrokeKind::Outside,
            );
        }
        if let Some(code) = hovered {
            if response.clicked() {
                self.chars.picking = false;
                self.insert_char(code);
                self.request_focus();
            }
        }
        let shown = hovered.unwrap_or(self.chars.code);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (preview, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
            ui.painter().rect_filled(preview, 4, Color32::BLACK);
            let scale = (32.0 / font.y).floor().max(1.0);
            paint_glyph(ui.painter(), &glyphs, shown, preview.center(), font * scale, self.palette[7]);
            ui.vertical(|ui| {
                let hex = format!("{shown:02X}");
                let details = if editor::is_insertable(shown) {
                    fl!(LANGUAGE_LOADER, "editor-character-details", code = shown, hex = hex.as_str())
                } else {
                    fl!(LANGUAGE_LOADER, "editor-character-details-reserved", code = shown, hex = hex.as_str())
                };
                ui.label(egui::RichText::new(details).strong());
                let hint = if self.chars.picking {
                    fl!(LANGUAGE_LOADER, "editor-character-picking-hint")
                } else {
                    fl!(LANGUAGE_LOADER, "editor-character-click-hint")
                };
                ui.label(egui::RichText::new(hint).size(11.5).color(ui.visuals().weak_text_color()));
            });
        });
    }

    fn color_picker(&mut self, context: &egui::Context) {
        let Some(mut pending) = self.colors else {
            return;
        };
        let mut apply = false;
        let mut close = false;
        let title = if self.editor.selection().is_some() {
            fl!(LANGUAGE_LOADER, "editor-selection-color")
        } else {
            fl!(LANGUAGE_LOADER, "editor-text-color")
        };
        let area = egui::Area::new(egui::Id::new("editor-colors"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.color_button.left_bottom() + egui::vec2(0.0, 4.0))
            .constrain(true)
            .show(context, |ui| {
                egui::Frame::popup(ui.style()).inner_margin(12).show(ui, |ui| {
                    ui.set_width(8.0 * 30.0);
                    ui.label(egui::RichText::new(title).strong());
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(fl!(LANGUAGE_LOADER, "editor-foreground"))
                            .size(11.5)
                            .color(ui.visuals().weak_text_color()),
                    );
                    for row in 0..2u8 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            for column in 0..8u8 {
                                let index = row * 8 + column;
                                let response = swatch(ui, self.palette[usize::from(index)], pending.fg == index).on_hover_text(color_name(index));
                                if response.clicked() {
                                    pending.fg = index;
                                }
                                if response.double_clicked() {
                                    apply = true;
                                }
                            }
                        });
                    }
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(fl!(LANGUAGE_LOADER, "editor-background"))
                            .size(11.5)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for index in 0..8u8 {
                            let response = swatch(ui, self.palette[usize::from(index)], pending.bg == index).on_hover_text(color_name(index));
                            if response.clicked() {
                                pending.bg = index;
                            }
                            if response.double_clicked() {
                                apply = true;
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.checkbox(&mut pending.blink, fl!(LANGUAGE_LOADER, "editor-blink"));
                    ui.add_space(4.0);
                    let (preview, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
                    ui.painter().rect_filled(preview, 3, self.palette[usize::from(pending.bg)]);
                    ui.painter().text(
                        preview.center(),
                        egui::Align2::CENTER_CENTER,
                        fl!(LANGUAGE_LOADER, "editor-color-preview-text"),
                        egui::FontId::monospace(14.0),
                        self.palette[usize::from(pending.fg)],
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(fl!(LANGUAGE_LOADER, "editor-color-help"))
                            .size(11.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(fl!(LANGUAGE_LOADER, "editor-default"))
                            .on_hover_text(fl!(LANGUAGE_LOADER, "editor-default-color-tooltip"))
                            .clicked()
                        {
                            pending = Attr::DEFAULT;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(fl!(LANGUAGE_LOADER, "editor-apply")).fill(ui.visuals().selection.bg_fill))
                                .on_hover_text("Enter")
                                .clicked()
                            {
                                apply = true;
                            }
                            if ui.button(fl!(LANGUAGE_LOADER, "editor-cancel")).on_hover_text("Esc").clicked() {
                                close = true;
                            }
                        });
                    });
                });
            });
        let outside = context.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|position| !area.response.rect.contains(position) && !self.color_button.contains(position))
        });
        if area.response.contains_pointer() && context.input(|input| input.pointer.any_released()) {
            self.request_focus();
        }
        if apply {
            self.apply_color(pending);
            self.request_focus();
        } else if close || outside {
            self.colors = None;
            self.request_focus();
        } else {
            self.colors = Some(pending);
        }
    }

    fn glyphs(&mut self, context: &egui::Context) -> egui::TextureHandle {
        if let Some(glyphs) = &self.glyphs {
            return glyphs.clone();
        }
        let screen = self.view.terminal.screen.lock();
        let size = screen.font_dimensions();
        let (width, height) = (size.width.max(1) as usize, size.height.max(1) as usize);
        let mut image = egui::ColorImage::filled([width * 16, height * 16], Color32::TRANSPARENT);
        if let Some(font) = screen.font(0) {
            for code in 0..256u32 {
                let glyph = font.glyph(char::from_u32(code).unwrap_or(' '));
                for (row, pixels) in glyph.to_bitmap_pixels().iter().take(height).enumerate() {
                    for (column, &set) in pixels.iter().take(width).enumerate() {
                        if set {
                            image[((code as usize % 16) * width + column, (code as usize / 16) * height + row)] = Color32::WHITE;
                        }
                    }
                }
            }
        }
        drop(screen);
        let texture = context.load_texture("editor-cp437-glyphs", image, egui::TextureOptions::NEAREST);
        self.glyphs = Some(texture.clone());
        texture
    }

    fn terminal(&mut self, ui: &mut egui::Ui, settings: &MonitorSettings, enabled: bool, output: &mut EditorOutput) {
        let available = ui.available_size().max(egui::vec2(1.0, 1.0));
        let font = self.font();
        self.zoom = settings
            .scaling_mode
            .compute_zoom(COLUMNS as f32 * font.x, font.y, available.x, available.y, settings.use_integer_scaling)
            .max(0.01);
        let rows = ((available.y / (font.y * self.zoom)).floor() as usize).max(MIN_ROWS);
        if rows != self.rows {
            self.rows = rows;
            self.view = screen_view(rows);
            self.drawn = None;
            self.follow = true;
        }
        self.view.terminal.has_focus = self.focused;
        self.redraw();
        let response = self.view.show(ui, settings);
        self.id = Some(response.id);
        self.rect = response.rect;
        if enabled && (self.focus_request || response.clicked() || response.drag_started()) {
            self.focus_request = false;
            response.request_focus();
        }
        if enabled {
            self.pointer(ui, &response, output);
        }
        if self.focused {
            let stroke = egui::Stroke::new(1.5, ui.visuals().selection.stroke.color);
            ui.painter().rect_stroke(response.rect, 0.0, stroke, egui::StrokeKind::Inside);
        }
        if self.redraw() {
            ui.ctx().request_repaint();
        }
    }

    fn cell_at(&self, position: egui::Pos2) -> (i32, i32) {
        let info = self.view.terminal.render_info.read();
        if let Some(cell) = info.screen_to_cell(position.x, position.y) {
            return cell;
        }
        drop(info);
        let size = self.font() * self.zoom;
        let local = position - self.rect.min;
        ((local.x / size.x).floor() as i32, (local.y / size.y).floor() as i32)
    }

    fn pointer(&mut self, ui: &egui::Ui, response: &egui::Response, output: &mut EditorOutput) {
        let (position, pressed, down, shift, scroll) = ui.input(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.modifiers.shift,
                input.raw_scroll_delta.y,
            )
        });
        let Some(position) = position else {
            return;
        };
        let (x, y) = self.cell_at(position);
        let text_rows = self.text_rows() as i32;
        if pressed && response.hovered() {
            self.status = None;
            self.press(x, y, shift, output);
        } else if self.dragging && down {
            if y < 0 {
                self.top = self.top.saturating_sub(1);
            } else if y >= text_rows {
                self.top += 1;
            }
            let row = self.top as i32 + y.clamp(0, text_rows - 1);
            self.editor.set_caret_visual(row.max(0) as usize, x.max(0) as usize, true);
        } else if !down {
            self.dragging = false;
        }
        if response.double_clicked() && (0..text_rows).contains(&y) {
            let pos = self.editor.pos_at(self.top + y as usize, x.max(0) as usize);
            self.editor.select_word(pos);
            self.dragging = false;
        }
        if response.hovered() && scroll != 0.0 {
            let row_height = self.font().y * self.zoom;
            self.wheel += scroll;
            let lines = (self.wheel / row_height).trunc();
            if lines != 0.0 {
                self.wheel -= lines * row_height;
                let top = self.top as i64 - lines as i64;
                self.top = top.clamp(0, self.editor.visual_lines().len().saturating_sub(1) as i64) as usize;
            }
        }
    }

    fn press(&mut self, x: i32, y: i32, shift: bool, output: &mut EditorOutput) {
        let text_rows = self.text_rows() as i32;
        if y == self.rows as i32 - 1 {
            let command = self
                .hotspots
                .iter()
                .find(|(start, end, _)| (*start..*end).contains(&x))
                .map(|(_, _, command)| *command);
            match command {
                Some(Command::Colors) => self.open_colors(),
                Some(Command::Chars) => self.toggle_chars(false),
                Some(Command::Quote) => self.toggle_quotes(),
                Some(Command::Find) => self.open_find(),
                Some(Command::Insert) => self.editor.toggle_insert(),
                Some(Command::Help) => output.help = true,
                None => {}
            }
        } else if (0..text_rows).contains(&y) {
            self.editor.set_caret_visual(self.top + y as usize, x.max(0) as usize, shift);
            self.dragging = true;
        }
    }

    /// Renders the text and status bar into the terminal screen when anything changed.
    fn redraw(&mut self) -> bool {
        let text_rows = self.text_rows();
        let lines = self.editor.visual_lines().len();
        let (caret_row, _) = self.editor.caret_visual();
        if self.follow {
            self.follow = false;
            if caret_row < self.top {
                self.top = caret_row;
            } else if caret_row >= self.top + text_rows {
                self.top = caret_row + 1 - text_rows;
            }
        }
        self.top = self.top.min(lines.saturating_sub(1));
        let mut hasher = DefaultHasher::new();
        (self.editor.revision(), self.top, self.rows, &self.status, self.focused).hash(&mut hasher);
        let key = hasher.finish();
        if self.drawn == Some(key) {
            return false;
        }
        self.drawn = Some(key);
        let mut canvas = Canvas::new(COLUMNS, self.rows);
        let caret = self.draw_text(&mut canvas, text_rows);
        self.draw_status(&mut canvas);
        let mut screen = self.view.terminal.screen.lock();
        let Some(screen) = screen.as_editable() else {
            return true;
        };
        for y in 0..self.rows {
            for x in 0..COLUMNS {
                let (ch, attr) = canvas.cells[y * COLUMNS + x];
                screen.set_char(Position::new(x as i32, y as i32), AttributedChar::new(ch, text_attribute(attr)));
            }
        }
        let insert = self.editor.insert_mode();
        let state = screen.caret_mut();
        state.visible = caret.is_some();
        state.blinking = true;
        state.shape = if insert { CaretShape::Underline } else { CaretShape::Block };
        if let Some((x, y)) = caret {
            state.set_position(Position::new(x, y));
        }
        true
    }

    fn draw_text(&self, canvas: &mut Canvas, text_rows: usize) -> Option<(i32, i32)> {
        let lines = self.editor.visual_lines();
        let selection = self.editor.selection();
        for row in 0..text_rows {
            let Some(line) = lines.get(self.top + row) else {
                break;
            };
            let cells = self.editor.cells(line);
            for (index, cell) in cells.iter().enumerate() {
                let pos = Pos::new(line.para, line.start + index);
                let selected = selection.is_some_and(|(start, end)| start <= pos && pos < end);
                canvas.put(index as i32, row as i32, cell.ch, if selected { inverse(cell.attr) } else { cell.attr });
            }
            let end = Pos::new(line.para, line.end);
            if line.last && selection.is_some_and(|(start, stop)| start <= end && end < stop) {
                canvas.put(cells.len() as i32, row as i32, ' ', inverse(Attr::DEFAULT));
            }
        }
        let (row, column) = self.editor.caret_visual();
        (row >= self.top && row < self.top + text_rows).then(|| (column.min(COLUMNS - 1) as i32, (row - self.top) as i32))
    }

    fn draw_status(&mut self, canvas: &mut Canvas) {
        let y = self.rows as i32 - 1;
        canvas.fill(0, y, COLUMNS as i32, 1, ' ', BAR);
        self.hotspots.clear();
        let (row, column) = self.editor.caret_visual();
        let position = format!(
            "{} {} {} {} ",
            fl!(LANGUAGE_LOADER, "editor-status-line"),
            row + 1,
            fl!(LANGUAGE_LOADER, "editor-status-column"),
            column + 1
        );
        let sample = format!(" {} ", fl!(LANGUAGE_LOADER, "editor-status-sample"));
        let sample_width = sample.chars().count() as i32;
        let sample_x = COLUMNS as i32 - sample_width;
        let position_x = sample_x - 1 - position.chars().count() as i32;
        canvas.text(position_x, y, &position, BAR);
        let mode = if self.editor.insert_mode() {
            fl!(LANGUAGE_LOADER, "editor-status-insert")
        } else {
            fl!(LANGUAGE_LOADER, "editor-status-overwrite")
        };
        let mode_width = mode.chars().count() as i32;
        let mode_x = position_x - 2 - mode_width;
        canvas.text(mode_x, y, &mode, BAR_TEXT);
        self.hotspots.push((mode_x, mode_x + mode_width, Command::Insert));
        canvas.text(sample_x, y, &sample, self.editor.attr());
        self.hotspots.push((sample_x, sample_x + sample_width, Command::Colors));
        if let Some(status) = &self.status {
            let max_x = mode_x.saturating_sub(1).max(1);
            canvas.text_limited(1, y, status, BAR_TEXT, max_x);
        } else {
            let mut x = 1;
            for (key, label, command) in [
                ("^K", fl!(LANGUAGE_LOADER, "editor-status-color"), Command::Colors),
                ("^G", fl!(LANGUAGE_LOADER, "editor-status-chars"), Command::Chars),
                ("^Q", fl!(LANGUAGE_LOADER, "editor-status-quote"), Command::Quote),
                ("^F", fl!(LANGUAGE_LOADER, "editor-status-find"), Command::Find),
                ("F1", fl!(LANGUAGE_LOADER, "editor-status-help"), Command::Help),
            ] {
                let max_x = mode_x.saturating_sub(1).max(1);
                if x >= max_x {
                    break;
                }
                let start = x;
                x = canvas.text_limited(x, y, key, BAR_KEY, max_x) + 1;
                x = canvas.text_limited(x, y, &label, BAR, max_x);
                self.hotspots.push((start, x, command));
                x += 2;
            }
        }
    }
}

const CHAR_PANEL_WIDTH: f32 = 16.0 * 24.0 + 16.0;

fn color_name(index: u8) -> String {
    let name = match index & 15 {
        0 => fl!(LANGUAGE_LOADER, "editor-color-black"),
        1 => fl!(LANGUAGE_LOADER, "editor-color-blue"),
        2 => fl!(LANGUAGE_LOADER, "editor-color-green"),
        3 => fl!(LANGUAGE_LOADER, "editor-color-cyan"),
        4 => fl!(LANGUAGE_LOADER, "editor-color-red"),
        5 => fl!(LANGUAGE_LOADER, "editor-color-magenta"),
        6 => fl!(LANGUAGE_LOADER, "editor-color-brown"),
        7 => fl!(LANGUAGE_LOADER, "editor-color-light-gray"),
        8 => fl!(LANGUAGE_LOADER, "editor-color-dark-gray"),
        9 => fl!(LANGUAGE_LOADER, "editor-color-light-blue"),
        10 => fl!(LANGUAGE_LOADER, "editor-color-light-green"),
        11 => fl!(LANGUAGE_LOADER, "editor-color-light-cyan"),
        12 => fl!(LANGUAGE_LOADER, "editor-color-light-red"),
        13 => fl!(LANGUAGE_LOADER, "editor-color-light-magenta"),
        14 => fl!(LANGUAGE_LOADER, "editor-color-yellow"),
        _ => fl!(LANGUAGE_LOADER, "editor-color-white"),
    };
    format!("{name} ({index:X})")
}

/// Toolbar button showing the current colors.
fn color_button(ui: &mut egui::Ui, fg: Color32, bg: Color32, open: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(58.0, 24.0), egui::Sense::click());
    let visuals = ui.visuals();
    let frame = if open || response.hovered() {
        visuals.widgets.hovered.bg_stroke
    } else {
        visuals.widgets.inactive.bg_stroke
    };
    ui.painter()
        .rect(rect, 5, visuals.widgets.inactive.weak_bg_fill, frame, egui::StrokeKind::Inside);
    let sample = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), egui::vec2(32.0, 16.0));
    ui.painter().rect_filled(sample, 2, bg);
    ui.painter().text(
        sample.center(),
        egui::Align2::CENTER_CENTER,
        fl!(LANGUAGE_LOADER, "editor-status-sample"),
        egui::FontId::monospace(12.0),
        fg,
    );
    let center = egui::pos2(rect.right() - 11.0, rect.center().y);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![center + egui::vec2(-4.0, -2.0), center + egui::vec2(4.0, -2.0), center + egui::vec2(0.0, 3.0)],
        visuals.text_color(),
        egui::Stroke::NONE,
    ));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, fl!(LANGUAGE_LOADER, "editor-text-color")));
    response
}

fn swatch(ui: &mut egui::Ui, color: Color32, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 24.0), egui::Sense::click());
    ui.painter().rect_filled(rect.shrink(2.0), 3, color);
    let visuals = ui.visuals();
    if selected {
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            4,
            egui::Stroke::new(2.0, visuals.selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_stroke(rect.shrink(0.5), 4, egui::Stroke::new(1.0, visuals.text_color()), egui::StrokeKind::Inside);
    } else {
        ui.painter()
            .rect_stroke(rect.shrink(2.0), 3, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Outside);
    }
    response
}

fn paint_glyph(painter: &egui::Painter, glyphs: &egui::TextureHandle, code: u8, center: egui::Pos2, size: egui::Vec2, color: Color32) {
    let uv = egui::Rect::from_min_size(
        egui::pos2(f32::from(code % 16) / 16.0, f32::from(code / 16) / 16.0),
        egui::Vec2::splat(1.0 / 16.0),
    );
    let rect = egui::Rect::from_center_size(center, size).round_to_pixels(painter.pixels_per_point());
    painter.image(glyphs.id(), rect, uv, color);
}

fn screen_view(rows: usize) -> ScreenView {
    let mut screen = TextScreen::new(Size::new(COLUMNS as i32, rows as i32));
    screen.terminal_state_mut().is_terminal_buffer = false;
    screen.buffer.ice_mode = IceMode::Blink;
    ScreenView::new(screen)
}

/// Selection highlight: swapped colors, like DOS editors.
fn inverse(attr: Attr) -> Attr {
    let fg = attr.bg;
    let bg = attr.fg & 7;
    if fg == bg {
        Attr::new(0, 7, false)
    } else {
        Attr::new(fg, bg, false)
    }
}

fn text_attribute(attr: Attr) -> TextAttribute {
    let mut attribute = TextAttribute::new(u32::from(attr.fg), u32::from(attr.bg));
    attribute.set_is_blinking(attr.blink);
    attribute
}

/// Off-screen character grid in CP437 codes.
struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<(char, Attr)>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![(' ', Attr::DEFAULT); width * height],
        }
    }

    fn put(&mut self, x: i32, y: i32, ch: char, attr: Attr) {
        let code = editor::cp437_byte(ch).unwrap_or(b'?');
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.cells[y as usize * self.width + x as usize] = (char::from(code), attr);
        }
    }

    /// Writes `text` and returns the column after it.
    fn text(&mut self, x: i32, y: i32, text: &str, attr: Attr) -> i32 {
        let mut x = x;
        for ch in text.chars() {
            self.put(x, y, ch, attr);
            x += 1;
        }
        x
    }

    fn text_limited(&mut self, x: i32, y: i32, text: &str, attr: Attr, max_x: i32) -> i32 {
        let mut x = x;
        for ch in text.chars() {
            if x >= max_x {
                break;
            }
            self.put(x, y, ch, attr);
            x += 1;
        }
        x
    }

    fn fill(&mut self, x: i32, y: i32, width: i32, height: i32, ch: char, attr: Attr) {
        for row in y..y + height {
            for column in x..x + width {
                self.put(column, row, ch, attr);
            }
        }
    }
}
