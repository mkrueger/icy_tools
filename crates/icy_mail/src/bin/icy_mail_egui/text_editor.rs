//! The UTF-8 message editor: egui's text editor, which brings IME composition, selection,
//! clipboard and undo, with the BBS editor's quote panel. Text wraps at the 79 columns it is sent
//! with, and quoted lines are dimmed.

use eframe::egui::{
    self,
    text::{CCursor, CCursorRange},
    Key, Modifiers,
};
use i18n_embed_fl::fl;
use icy_mail::{editor::WRAP_WIDTH, LANGUAGE_LOADER};

use super::{
    terminal_editor::{quote_panel, status_hint, EditorOutput, QuotePanelAction, Quotes},
    widgets::{Icon, Icons},
};

pub struct TextEditor {
    /// The message as typed: paragraphs end with a line break, long lines are wrapped when sent.
    pub text: String,
    pub quotes: Quotes,
    pub status: Option<String>,
    focus_request: bool,
    /// Caret position in characters, for the status bar.
    cursor: usize,
}

impl TextEditor {
    pub fn new(text: &str, quotes: Vec<String>, open_quotes: bool) -> Self {
        Self {
            text: text.replace("\r\n", "\n").replace('\r', "\n"),
            quotes: Quotes {
                open: open_quotes && !quotes.is_empty(),
                lines: quotes,
                ..Default::default()
            },
            status: None,
            focus_request: false,
            cursor: 0,
        }
    }

    pub fn id() -> egui::Id {
        egui::Id::new("composer-utf8-text")
    }

    pub fn request_focus(&mut self) {
        self.focus_request = true;
    }

    /// Whether Escape closes something inside the editor rather than the composer.
    pub fn captures_escape(&self) -> bool {
        self.quotes.open
    }

    pub fn toggle_quotes(&mut self) {
        if self.quotes.lines.is_empty() {
            return;
        }
        self.quotes.open = !self.quotes.open;
        self.quotes.reveal = true;
    }

    /// Inserts quote lines on their own lines at the caret, which then stands after them.
    pub fn insert_quotes(&mut self, context: &egui::Context, range: std::ops::Range<usize>) {
        let count = self.quotes.lines.len();
        let range = range.start.min(count)..range.end.min(count);
        if range.is_empty() {
            return;
        }
        let mut state = egui::TextEdit::load_state(context, Self::id()).unwrap_or_default();
        let total = self.text.chars().count();
        let index = state
            .cursor
            .char_range()
            .map_or(total, |range| range.primary.index.max(range.secondary.index))
            .min(total);
        let byte = self.text.char_indices().nth(index).map_or(self.text.len(), |(byte, _)| byte);
        let mut insert = String::new();
        if byte > 0 && !self.text[..byte].ends_with('\n') {
            insert.push('\n');
        }
        insert.push_str(&self.quotes.lines[range.clone()].join("\n"));
        insert.push('\n');
        self.text.insert_str(byte, &insert);
        let caret = index + insert.chars().count();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(caret))));
        state.store(context, Self::id());
        self.cursor = caret;
        self.quotes.selected = range.end.min(count - 1);
        self.quotes.reveal = true;
    }

    /// Quote panel keys, taken before the text editor sees them, as in the BBS editor.
    fn keyboard(&mut self, context: &egui::Context, output: &mut EditorOutput) {
        if context.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Ime(egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_) | egui::ImeEvent::Commit(_))
                )
            })
        }) {
            self.quotes.open = false;
            return;
        }
        let consume = |modifiers: Modifiers, key: Key| context.input_mut(|input| input.consume_key(modifiers, key));
        if consume(Modifiers::COMMAND, Key::Q) {
            self.toggle_quotes();
            return;
        }
        if !self.quotes.open || self.quotes.lines.is_empty() {
            return;
        }
        let count = self.quotes.lines.len();
        let selected = self.quotes.selected;
        if consume(Modifiers::COMMAND, Key::A) {
            self.insert_quotes(context, selected..count);
            self.quotes.open = false;
            output.changed = true;
            return;
        }
        let moves = [
            (Key::ArrowUp, selected.saturating_sub(1)),
            (Key::ArrowDown, (selected + 1).min(count - 1)),
            (Key::PageUp, selected.saturating_sub(10)),
            (Key::PageDown, (selected + 10).min(count - 1)),
            (Key::Home, 0),
            (Key::End, count - 1),
        ];
        for (key, target) in moves {
            if consume(Modifiers::NONE, key) {
                self.quotes.selected = target;
                self.quotes.reveal = true;
                return;
            }
        }
        if consume(Modifiers::NONE, Key::Enter) {
            self.insert_quotes(context, selected..selected + 1);
            output.changed = true;
        } else if consume(Modifiers::NONE, Key::Escape) {
            self.quotes.open = false;
        } else if context.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Text(_)))) {
            // Typing goes on with the message.
            self.quotes.open = false;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, icons: &mut Icons, enabled: bool, font: egui::FontId) -> EditorOutput {
        let mut output = EditorOutput::default();
        let context = ui.ctx().clone();
        if enabled && context.memory(|memory| memory.has_focus(Self::id())) {
            self.keyboard(&context, &mut output);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let quote_tooltip = if self.quotes.lines.is_empty() {
                fl!(LANGUAGE_LOADER, "editor-quote-disabled-tooltip")
            } else {
                fl!(LANGUAGE_LOADER, "editor-quote-tooltip")
            };
            let caption = fl!(LANGUAGE_LOADER, "editor-quote");
            if icons
                .tool(ui, Icon::Quote, Some(&caption), &quote_tooltip, !self.quotes.lines.is_empty(), self.quotes.open)
                .clicked()
            {
                self.toggle_quotes();
                self.request_focus();
            }
            ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "editor-utf8-hint")).weak().size(12.0));
        });
        ui.add_space(4.0);
        let quotes_height = if self.quotes.open {
            (ui.available_height() * 0.4).clamp(120.0, 260.0)
        } else {
            0.0
        };
        let text_height = (ui.available_height() - quotes_height).max(80.0);
        let (column, row_height) = ui.fonts_mut(|fonts| (fonts.glyph_width(&font, '0'), fonts.row_height(&font)));
        let margin = WRAP_WIDTH as f32 * column;
        let weak = ui.visuals().weak_text_color();
        let strong = ui.visuals().text_color();
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job = egui::text::LayoutJob::default();
            for line in text.as_str().split_inclusive('\n') {
                let color = if is_quote(line) { weak } else { strong };
                job.append(
                    line,
                    0.0,
                    egui::TextFormat {
                        font_id: font.clone(),
                        color,
                        ..Default::default()
                    },
                );
            }
            // As sent: lines wrap at the message width even in a wider pane.
            job.wrap.max_width = wrap_width.min(margin);
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        let edit = egui::ScrollArea::vertical()
            .id_salt("composer-utf8-scroll")
            .max_height(text_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut self.text)
                    .id(Self::id())
                    .font(font.clone())
                    .desired_width(f32::INFINITY)
                    .desired_rows(((text_height / row_height) as usize).max(4))
                    .lock_focus(false)
                    .interactive(enabled)
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;
        if edit.response.changed() {
            if self.text.contains('\t') {
                self.text = self.text.replace('\t', "    ");
            }
            output.changed = true;
        }
        if let Some(range) = edit.cursor_range {
            self.cursor = range.primary.index;
        }
        let guide = edit.galley_pos.x + margin;
        if guide < edit.response.rect.right() {
            let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
            ui.painter().vline(guide, edit.response.rect.y_range(), stroke);
        }
        if std::mem::take(&mut self.focus_request) {
            edit.response.request_focus();
        }
        if self.quotes.open {
            ui.add_space(6.0);
            match quote_panel(ui, icons, &mut self.quotes) {
                Some(QuotePanelAction::Insert { range, close }) => {
                    self.insert_quotes(&context, range);
                    output.changed = true;
                    if close {
                        self.quotes.open = false;
                    }
                    self.request_focus();
                }
                Some(QuotePanelAction::Select(index)) => {
                    self.quotes.selected = index;
                    self.request_focus();
                }
                Some(QuotePanelAction::Close) => {
                    self.quotes.open = false;
                    self.request_focus();
                }
                None => {}
            }
        }
        if output.changed {
            self.status = None;
        }
        output
    }

    /// Key hints and the caret's line and column, like the BBS editor's status bar.
    pub fn status_bar(&mut self, ui: &mut egui::Ui) -> bool {
        let mut help = false;
        ui.spacing_mut().item_spacing.x = 10.0;
        if let Some(status) = &self.status {
            ui.add(egui::Label::new(egui::RichText::new(status).size(12.0).color(super::widgets::warning(ui))).truncate());
        } else {
            if status_hint(ui, "Ctrl+Q", &fl!(LANGUAGE_LOADER, "editor-status-quote")).clicked() {
                self.toggle_quotes();
            }
            if status_hint(ui, "F1", &fl!(LANGUAGE_LOADER, "editor-status-help")).clicked() {
                help = true;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (row, column) = line_and_column(&self.text, self.cursor);
            ui.label(egui::RichText::new("UTF-8").size(12.0).weak());
            let position = format!(
                "{} {}, {} {}",
                fl!(LANGUAGE_LOADER, "editor-status-line"),
                row + 1,
                fl!(LANGUAGE_LOADER, "editor-status-column"),
                column + 1
            );
            ui.label(egui::RichText::new(position).size(12.0).weak());
        });
        help
    }
}

/// A line quoted from another message (` JD> …`, `> …`).
fn is_quote(line: &str) -> bool {
    let line = line.trim_start();
    let initials = line.chars().take_while(|ch| ch.is_uppercase()).count();
    initials <= 3 && line.chars().nth(initials) == Some('>')
}

/// Zero-based line and column of a character index, counting paragraphs as typed.
fn line_and_column(text: &str, index: usize) -> (usize, usize) {
    let before: String = text.chars().take(index).collect();
    let row = before.matches('\n').count();
    let column = before.rsplit('\n').next().map_or(0, |line| line.chars().count());
    (row, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ime_events_leave_quote_keys_for_the_text_editor() {
        for event in [
            egui::ImeEvent::Enabled,
            egui::ImeEvent::Preedit("にほん".into()),
            egui::ImeEvent::Commit("日本".into()),
        ] {
            let context = egui::Context::default();
            let mut editor = TextEditor::new("", vec![" AB> quote".into()], true);
            let _ = context.run(
                egui::RawInput {
                    events: vec![
                        egui::Event::Ime(event),
                        egui::Event::Key {
                            key: Key::Enter,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |context| {
                    let mut output = EditorOutput::default();
                    editor.keyboard(context, &mut output);
                    assert!(!editor.quotes.open);
                    assert!(context.input(|input| input.key_pressed(Key::Enter)));
                    assert!(!output.changed);
                    assert!(editor.text.is_empty());
                },
            );
        }
    }

    #[test]
    fn quote_lines_and_positions() {
        assert!(is_quote(" JD> text") && is_quote("> text") && is_quote("ДЛ>> текст"));
        assert!(!is_quote("Hello> there") && !is_quote("ABCD> no"));
        assert_eq!(line_and_column("ab\nсde", 5), (1, 2));
        assert_eq!(line_and_column("", 0), (0, 0));
    }

    #[test]
    fn quotes_are_inserted_on_their_own_lines_at_the_caret() {
        let context = egui::Context::default();
        let mut editor = TextEditor::new("Hello world", vec![" AB> one".into(), " AB> two".into()], true);
        let mut state = egui::TextEdit::load_state(&context, TextEditor::id()).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(5))));
        state.store(&context, TextEditor::id());
        editor.insert_quotes(&context, 0..2);
        assert_eq!(editor.text, "Hello\n AB> one\n AB> two\n world");
        assert_eq!(editor.cursor, "Hello\n AB> one\n AB> two\n".chars().count());
        editor.insert_quotes(&context, 1..2);
        assert_eq!(editor.text, "Hello\n AB> one\n AB> two\n AB> two\n world");
    }
}
