use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{AttributedChar, AutoWrapMode, EditableScreen, Layer, Position, Screen, TextAttribute, TextPane, TextScreen};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredGrapheme")]
pub(crate) struct Grapheme {
    pub text: String,
    pub width: usize,
}

#[derive(Deserialize)]
struct StoredGrapheme {
    text: String,
    width: usize,
}

impl TryFrom<StoredGrapheme> for Grapheme {
    type Error = &'static str;

    fn try_from(value: StoredGrapheme) -> Result<Self, Self::Error> {
        if value.text.len() > MAX_GRAPHEME_BYTES + '\u{25cc}'.len_utf8()
            || value.width == 0
            || value.width != value.text.width()
            || value.text.graphemes(true).count() != 1
        {
            return Err("invalid Unicode grapheme");
        }
        Ok(Self {
            text: value.text,
            width: value.width,
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct UnicodeState {
    pending: Option<PendingGrapheme>,
}

/// Further extending scalars beyond this UTF-8 size are ignored by the screen only.
pub const MAX_GRAPHEME_BYTES: usize = 4096;

#[derive(Clone)]
struct PendingGrapheme {
    text: String,
    layer: usize,
    position: Position,
    after: Position,
    wrap_pending: bool,
    attribute: TextAttribute,
    width: usize,
}

impl Layer {
    pub(crate) fn shift_cells(&mut self, start: Position, end: Position, delta: Position, attribute: TextAttribute) {
        let start = Position::new(start.x.max(0), start.y.max(0));
        let end = Position::new(end.x.min(self.width() - 1), end.y.min(self.height() - 1));
        let mut cells = Vec::new();
        for row in start.y..=end.y {
            for column in start.x..=end.x {
                let position = Position::new(column, row);
                if self.is_grapheme_continuation(position) {
                    continue;
                }
                let grapheme = self.grapheme_at(position).map(|(text, width)| (text.to_owned(), width));
                let width = grapheme.as_ref().map_or(1, |(_, width)| *width) as i32;
                let target = position + delta;
                if column + width - 1 <= end.x && target.x >= start.x && target.x + width - 1 <= end.x && target.y >= start.y && target.y <= end.y {
                    cells.push((target, self.char_at(position), grapheme));
                }
            }
        }
        for row in start.y..=end.y {
            for column in start.x..=end.x {
                self.set_char(Position::new(column, row), AttributedChar::new(' ', attribute));
            }
        }
        for (position, character, grapheme) in cells {
            if let Some((text, width)) = grapheme {
                self.put_grapheme(position, text, width, character.attribute);
            } else {
                self.set_char(position, character);
            }
        }
    }

    pub(crate) fn clip_graphemes(&mut self, size: crate::Size) {
        let removed: Vec<_> = self
            .graphemes
            .iter()
            .flat_map(|(&row, columns)| {
                columns.iter().filter_map(move |(&column, grapheme)| {
                    (row >= size.height || column + grapheme.width as i32 > size.width).then_some(Position::new(column, row))
                })
            })
            .collect();
        for position in removed {
            self.clear_grapheme_at(position, self.char_at(position).attribute);
        }
    }

    pub fn grapheme_at(&self, position: Position) -> Option<(&str, usize)> {
        if position.x < 0 || position.y < 0 || position.x >= self.width() || position.y >= self.height() {
            return None;
        }
        let grapheme = self.graphemes.get(&position.y)?.get(&position.x)?;
        Some((&grapheme.text, grapheme.width))
    }

    pub fn is_grapheme_continuation(&self, position: Position) -> bool {
        position.x >= 0
            && position.y >= 0
            && position.x < self.width()
            && position.y < self.height()
            && self.grapheme_start(position).is_some_and(|start| start != position)
    }

    fn grapheme_start(&self, position: Position) -> Option<Position> {
        let (&column, grapheme) = self.graphemes.get(&position.y)?.range(..=position.x).next_back()?;
        (position.x < column + grapheme.width as i32).then_some(Position::new(column, position.y))
    }

    pub(crate) fn clear_grapheme_at(&mut self, position: Position, attribute: TextAttribute) {
        let Some(start) = self.grapheme_start(position) else { return };
        let row = self.graphemes.get_mut(&start.y).unwrap();
        let grapheme = row.remove(&start.x).unwrap();
        if row.is_empty() {
            self.graphemes.remove(&start.y);
        }
        if let Some(line) = self.lines.get_mut(start.y as usize) {
            for column in start.x..start.x + grapheme.width as i32 {
                line.set_char(column, AttributedChar::new(' ', attribute));
            }
        }
    }

    pub(crate) fn put_grapheme(&mut self, position: Position, text: String, width: usize, attribute: TextAttribute) {
        if self.properties.is_locked || !self.properties.is_visible {
            return;
        }
        if position.x < 0 || position.y < 0 || position.y >= self.height() || position.x + width as i32 > self.width() {
            return;
        }
        if self.properties.has_alpha_channel
            && self.properties.is_alpha_channel_locked
            && (position.x..position.x + width as i32).any(|column| !self.char_at(Position::new(column, position.y)).is_visible())
        {
            return;
        }
        for column in position.x..position.x + width as i32 {
            self.set_char(Position::new(column, position.y), AttributedChar::new(' ', attribute));
        }
        self.set_char(position, AttributedChar::new(text.chars().next().unwrap_or(' '), attribute));
        self.graphemes.entry(position.y).or_default().insert(position.x, Grapheme { text, width });
    }
}

impl TextScreen {
    pub(crate) fn finish_grapheme(&mut self) {
        self.unicode.pending = None;
    }

    pub(crate) fn shift_unicode_row(&mut self, amount: i32) {
        self.finish_grapheme();
        let start = self.caret.position();
        let end = Position::new(self.last_editable_column(), start.y);
        self.buffer.layers[self.current_layer].shift_cells(start, end, Position::new(amount, 0), self.caret.attribute);
    }
    /// Opts into grapheme/cell layout without changing the input encoding or legacy defaults.
    pub fn set_unicode_width(&mut self, enabled: bool) {
        if self.buffer.unicode_width != enabled {
            self.unicode.pending = None;
        }
        self.buffer.unicode_width = enabled;
    }

    pub fn unicode_width(&self) -> bool {
        self.buffer.unicode_width
    }

    pub fn grapheme_at(&self, position: Position) -> Option<(&str, usize)> {
        self.buffer.grapheme_at(position)
    }

    pub fn is_grapheme_continuation(&self, position: Position) -> bool {
        self.buffer.is_grapheme_continuation(position)
    }

    pub(crate) fn print_unicode(&mut self, character: AttributedChar) {
        let layer = &self.buffer.layers[self.current_layer];
        if layer.properties.is_locked || !layer.properties.is_visible {
            self.finish_grapheme();
            return;
        }
        if self.caret.x < 0
            || self.caret.y < 0
            || self.width() <= 0
            || self.height() <= 0
            || self.caret.x >= crate::limits::MAX_BUFFER_WIDTH
            || self.caret.y >= crate::limits::MAX_BUFFER_HEIGHT
        {
            self.finish_grapheme();
            return;
        }
        let mut text = character.ch.to_string();
        let mut attribute = character.attribute;
        let mut extending = false;
        let mut previous_width = 0;
        if let Some(pending) = self.unicode.pending.take() {
            let joined = format!("{}{text}", pending.text);
            if pending.after == self.caret.position()
                && pending.layer == self.current_layer
                && pending.wrap_pending == self.terminal_state().wrap_pending
                && self.buffer.layers[self.current_layer].grapheme_at(pending.position).is_some()
                && joined.graphemes(true).count() == 1
            {
                if joined.len() > MAX_GRAPHEME_BYTES {
                    self.unicode.pending = Some(pending);
                    return;
                }
                self.buffer.layers[self.current_layer].clear_grapheme_at(pending.position, pending.attribute);
                self.caret.set_position(pending.position);
                self.terminal_state_mut().wrap_pending = false;
                text = joined;
                attribute = pending.attribute;
                extending = true;
                previous_width = pending.width;
            }
        }

        let raw_text = text.clone();
        let mut width = text.width();
        if width == 0 {
            text.insert(0, '\u{25cc}');
            width = text.width().max(1);
        }
        if self.terminal_state().wrap_pending {
            self.terminal_state_mut().wrap_pending = false;
            self.lf();
            self.caret.x = self.first_editable_column();
        }
        let last_column = if self.terminal_state().in_margin(self.caret.position()) {
            self.last_editable_column()
        } else {
            self.width() - 1
        };
        if self.caret.x + width as i32 > last_column + 1 {
            if self.terminal_state().auto_wrap_mode == AutoWrapMode::AutoWrap && width as i32 <= last_column - self.first_editable_column() + 1 {
                self.lf();
                self.caret.x = self.first_editable_column();
                extending = false;
            } else {
                text = ".".into();
                width = 1;
            }
        }
        let position = self.caret.position();
        if !self.terminal_state().is_terminal_buffer && position.y >= self.height() {
            self.set_height(position.y + 1);
        }
        if self.caret.insert_mode {
            if extending {
                let change = width as i32 - previous_width as i32;
                if change != 0 {
                    let start = Position::new(position.x + width.min(previous_width) as i32, position.y);
                    self.buffer.layers[self.current_layer].shift_cells(start, Position::new(last_column, position.y), Position::new(change, 0), attribute);
                }
            } else {
                self.shift_unicode_row(width as i32);
            }
        }
        self.buffer.layers[self.current_layer].put_grapheme(position, text, width, attribute);
        self.buffer.mark_line_dirty(position.y);
        let next_column = position.x + width as i32;
        self.caret.x = next_column.min(last_column);
        self.terminal_state_mut().wrap_pending = next_column > last_column && self.terminal_state().auto_wrap_mode == AutoWrapMode::AutoWrap;
        self.unicode.pending = Some(PendingGrapheme {
            text: raw_text,
            layer: self.current_layer,
            position,
            after: self.caret.position(),
            wrap_pending: self.terminal_state().wrap_pending,
            attribute,
            width,
        });
    }
}
