//! Unicode editing keeps layer metadata and cell contents together in undo snapshots.
//!
//! Input, deletion, scalar brush writes, selection fill/erase, and plain-text
//! copy/paste support terminal graphemes. A caret may sit at the right insertion
//! edge, never inside a wide glyph after editing. Rich clipboard data uses the
//! native `.icy` encoding; RTF is unavailable.
//! Resizing, merging and cell-placement flips use complete layer snapshots.
//! Flips preserve glyph text rather than applying legacy bitmap glyph substitutions;
//! selections cutting a grapheme and layers containing images are rejected.
//! Other cell-grid transforms (crop, rotate, justify, scroll, and row/column mutation) explicitly fail
//! until they can preserve grapheme metadata.
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{AttributedChar, EngineError, Layer, Position, Rectangle, Result, Size, TextPane};

use super::{EditState, EditorUndoOp};

struct UnicodeClipboardSelection {
    bounds: Rectangle,
    cells: Vec<(Position, AttributedChar, Option<String>)>,
}

impl EditState {
    fn unicode_clipboard_selection(&self) -> Result<Option<UnicodeClipboardSelection>> {
        if !self.get_buffer().unicode_width() {
            return Err(EngineError::Generic("Unicode rich clipboard requires a Unicode-width buffer".into()));
        }
        if !self.is_something_selected() {
            return Ok(None);
        }
        let index = self.get_current_layer()?;
        self.require_cell_layer(index)?;
        let source = &self.get_buffer().layers[index];
        let area = self.selected_rectangle().intersect(&source.rectangle());
        if area.is_empty() {
            return Ok(None);
        }
        if !source.sixels.is_empty() {
            return Err(EngineError::Generic("Copying Unicode selections containing images is not supported".into()));
        }
        let mut left = area.left();
        let mut right = area.right();
        let mut cells = Vec::new();
        for y in area.y_range() {
            let mut x = area.left();
            while x < area.right() {
                let position = Position::new(x, y) - source.offset();
                let start = source.grapheme_start(position).unwrap_or(position);
                let grapheme = source.grapheme_at(start);
                let width = grapheme.map_or(1, |(_, width)| width as i32);
                let document_start = start + source.offset();
                if (0..width).any(|column| self.is_selected(document_start + Position::new(column, 0))) {
                    // Selection masks operate on cells; touching either half selects
                    // the complete glyph, never a fragment of its text.
                    left = left.min(document_start.x);
                    right = right.max(document_start.x + width);
                    let character = source.display_char_at(start);
                    if character.is_visible() {
                        if (0..width).any(|column| source.effects.is_masked(start + Position::new(column, 0))) {
                            return Err(EngineError::Generic("A layer mask cuts a selected Unicode grapheme".into()));
                        }
                        cells.push((document_start, character, grapheme.map(|(text, _)| text.to_owned())));
                    }
                }
                x = document_start.x + width;
            }
        }
        Ok(Some(UnicodeClipboardSelection {
            bounds: Rectangle::from_min_size(Position::new(left, area.top()), Size::new(right - left, area.height())),
            cells,
        }))
    }

    /// Expanded rich-selection bounds in document coordinates.
    ///
    /// Touching either cell of a wide grapheme includes the complete grapheme.
    /// Uses the same selection/mask traversal and validation as clipboard encoding.
    pub fn unicode_clipboard_bounds(&self) -> Result<Option<Rectangle>> {
        Ok(self.unicode_clipboard_selection()?.map(|selection| selection.bounds))
    }

    /// Encode the current rich Unicode selection as native `.icy` clipboard data.
    /// This fallible form reports unsupported selections instead of dropping data.
    pub fn unicode_clipboard_data(&self) -> Result<Option<Vec<u8>>> {
        let Some(selection) = self.unicode_clipboard_selection()? else {
            return Ok(None);
        };
        let mut layer = Layer::new("Unicode selection", selection.bounds.size());
        layer.properties.has_alpha_channel = true;
        for (position, character, grapheme) in selection.cells {
            let position = position - selection.bounds.start;
            if let Some(text) = grapheme {
                layer.set_grapheme(position, &text, character.attribute)?;
            } else {
                layer.set_char(position, character);
            }
        }
        let mut buffer = self.get_buffer().clone();
        buffer.buffer_type = crate::BufferType::Unicode;
        buffer.layers.clear();
        buffer.set_size(layer.size());
        buffer.layers.push(layer);
        buffer.tags.clear();
        let mut options = crate::SaveOptions::icy_draw();
        if let icy_engine::FormatOptions::IcyDraw(options) = &mut options.format {
            options.skip_thumbnail = true;
        }
        icy_engine::FileFormat::IcyDraw.to_bytes(&buffer, &options).map(Some)
    }

    /// Mirror cell placement while keeping each Unicode grapheme intact.
    pub(super) fn flip_unicode_layer(&mut self, horizontal: bool) -> Result<()> {
        let index = self.get_current_layer()?;
        self.require_cell_layer(index)?;
        let source = &self.get_buffer().layers[index];
        if source.properties.is_locked || !source.properties.is_visible {
            return Ok(());
        }
        if !source.sixels.is_empty() {
            return Err(EngineError::Generic("Flipping Unicode layers containing images is not supported".into()));
        }
        let full = crate::Rectangle::from_min_size(Position::default(), source.size());
        let area = self
            .selection()
            .map_or(full, |selection| (selection.as_rectangle() - source.offset()).intersect(&full));
        if area.is_empty() {
            return Ok(());
        }
        let mut cells = Vec::new();
        for y in area.y_range() {
            for x in area.x_range() {
                let position = Position::new(x, y);
                if source.is_grapheme_continuation(position) {
                    if source.grapheme_start(position).is_some_and(|start| start.x < area.left()) {
                        return Err(EngineError::Generic("Flip selection cuts a Unicode grapheme".into()));
                    }
                    continue;
                }
                let grapheme = source.grapheme_at(position);
                let width = grapheme.map_or(1, |(_, width)| width as i32);
                if x + width > area.right() {
                    return Err(EngineError::Generic("Flip selection cuts a Unicode grapheme".into()));
                }
                let target = if horizontal {
                    Position::new(area.left() + area.right() - x - width, y)
                } else {
                    Position::new(x, area.top() + area.bottom() - y - 1)
                };
                cells.push((target, source.char_at(position), grapheme.map(|(text, _)| text.to_owned())));
            }
        }
        let mut layer = source.clone();
        let alpha_locked = layer.properties.is_alpha_channel_locked;
        layer.properties.is_alpha_channel_locked = false;
        for y in area.y_range() {
            for x in area.x_range() {
                layer.set_char(Position::new(x, y), AttributedChar::invisible());
            }
        }
        for (position, character, grapheme) in cells {
            if let Some(text) = grapheme {
                layer.set_grapheme(position, &text, character.attribute)?;
            } else {
                layer.set_char(position, character);
            }
        }
        layer.properties.is_alpha_channel_locked = alpha_locked;
        if let Some(mask) = &source.effects.mask {
            layer.effects.mask = Some(mask.transformed(source.size(), |position| {
                if !area.contains_pt(position) {
                    position
                } else if horizontal {
                    Position::new(area.left() + area.right() - position.x - 1, position.y)
                } else {
                    Position::new(position.x, area.top() + area.bottom() - position.y - 1)
                }
            })?);
        }
        let before = self.get_caret().position();
        let after = if let Some(start) = layer.grapheme_start(before) { start } else { before };
        self.commit_unicode_layer(
            layer,
            before,
            after,
            if horizontal { "Flip Unicode horizontally" } else { "Flip Unicode vertically" },
        )
    }

    pub(super) fn paste_unicode_text(&mut self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        if text.chars().any(|ch| ch.is_control() && ch != '\r' && ch != '\n') {
            return Err(EngineError::Generic("Control characters are not supported in Unicode editor text".into()));
        }
        let index = self.get_current_layer()?;
        let offset = self.get_caret().position();
        let document_offset = offset + self.get_buffer().layers[index].offset();
        let available = (self.get_buffer().width() - document_offset.x).clamp(1, icy_engine::limits::MAX_BUFFER_WIDTH);
        let mut cells = Vec::new();
        let mut pos = Position::default();
        let mut width = 1;
        for grapheme in text.graphemes(true) {
            if matches!(grapheme, "\r\n" | "\r" | "\n") {
                pos = Position::new(0, pos.y + 1);
            } else {
                let mut grapheme = grapheme.to_owned();
                if grapheme.width() == 0 {
                    grapheme.insert(0, '\u{25cc}');
                }
                let cell_width = grapheme.width() as i32;
                if !(1..=2).contains(&cell_width) || cell_width > available || grapheme.len() > icy_engine::MAX_GRAPHEME_BYTES {
                    return Err(EngineError::Generic("Unicode paste grapheme does not fit terminal cell limits".into()));
                }
                if pos.x + cell_width > available {
                    pos = Position::new(0, pos.y + 1);
                }
                cells.push((pos, grapheme));
                pos.x += cell_width;
                width = width.max(pos.x);
            }
            if pos.y >= icy_engine::limits::MAX_BUFFER_HEIGHT {
                return Err(EngineError::Generic("Unicode paste exceeds layer height limits".into()));
            }
        }
        let mut layer = Layer::new("Pasted Unicode text", Size::new(width, pos.y + 1));
        layer.properties.has_alpha_channel = true;
        layer.set_offset(offset);
        for (position, text) in cells {
            layer.set_grapheme(position, &text, self.get_caret().attribute)?;
        }
        let _undo = self.begin_atomic_undo("Paste Unicode text");
        self.push_undo_action(EditorUndoOp::Paste {
            current_layer: index,
            layer: Box::new(layer),
        })?;
        self.clear_selection()
    }

    fn validate_unicode_size(size: Size) -> Result<()> {
        if size.width <= 0 || size.height <= 0 || size.width > icy_engine::limits::MAX_BUFFER_WIDTH || size.height > icy_engine::limits::MAX_BUFFER_HEIGHT {
            return Err(EngineError::Generic("Invalid Unicode canvas or layer size".into()));
        }
        Ok(())
    }

    fn clip_unicode_layer(layer: &mut Layer, size: Size) {
        layer.set_size(size);
        layer.lines.truncate(size.height.max(0) as usize);
        for line in &mut layer.lines {
            line.chars.truncate(size.width.max(0) as usize);
        }
    }

    pub(super) fn resize_unicode_buffer(&mut self, resize_layers: bool, size: Size) -> Result<()> {
        Self::validate_unicode_size(size)?;
        let orig_size = self.get_buffer().size();
        if size == orig_size {
            return Ok(());
        }
        let before = self.get_caret().position();
        let _undo = self.begin_atomic_undo("Resize Unicode canvas");
        let mut layers = self.get_buffer().layers.clone();
        if resize_layers {
            for layer in &mut layers {
                if layer.is_group() {
                    continue;
                }
                let new_size = if layer.offset() == Position::default() && layer.size() == orig_size {
                    size
                } else {
                    Size::new(
                        layer.width().min((size.width - layer.offset().x).max(0)),
                        layer.height().min((size.height - layer.offset().y).max(0)),
                    )
                };
                Self::clip_unicode_layer(layer, new_size);
            }
        }
        self.push_undo_action(EditorUndoOp::Crop { orig_size, size, layers })?;
        let bounded = Position::new(before.x.clamp(0, size.width), before.y.clamp(0, size.height - 1));
        let after = self.unicode_caret_position(bounded, false);
        self.push_undo_action(EditorUndoOp::ReverseCaretPosition { pos: after, old_pos: before })
    }

    pub(super) fn resize_unicode_layer(&mut self, index: usize, size: Size) -> Result<()> {
        Self::validate_unicode_size(size)?;
        self.require_cell_layer(index)?;
        let mut layer = self
            .get_buffer()
            .layers
            .get(index)
            .cloned()
            .ok_or_else(|| EngineError::Generic("Invalid layer index".into()))?;
        if layer.size() == size {
            return Ok(());
        }
        Self::clip_unicode_layer(&mut layer, size);
        let before = self.get_caret().position();
        let _undo = self.begin_atomic_undo("Resize Unicode layer");
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description: "Resize Unicode layer".into(),
        })?;
        let after = self.unicode_caret_position(before, false);
        self.push_undo_action(EditorUndoOp::ReverseCaretPosition { pos: after, old_pos: before })
    }

    pub(super) fn merge_unicode_layer_down(&mut self, index: usize) -> Result<()> {
        if index == 0 || index >= self.get_buffer().layers.len() {
            return Err(EngineError::Generic("Merge requires a layer below the current layer".into()));
        }
        self.require_cell_layer(index)?;
        self.require_cell_layer(index - 1)?;
        let buffer = self.get_buffer();
        let base = &buffer.layers[index - 1];
        let top = &buffer.layers[index];
        if base.parent_group != top.parent_group || base.role == crate::Role::Image || top.role == crate::Role::Image {
            return Err(EngineError::Generic("Merge requires two cell layers in the same group".into()));
        }
        let base = buffer.layer_with_baked_effects(index - 1)?;
        let top = buffer.layer_with_baked_effects(index)?;
        let start = Position::new(base.offset().x.min(top.offset().x), base.offset().y.min(top.offset().y));
        let size = Size::new(
            (base.offset().x + base.width()).max(top.offset().x + top.width()) - start.x,
            (base.offset().y + base.height()).max(top.offset().y + top.height()) - start.y,
        );
        Self::validate_unicode_size(size)?;
        let mut merged = base.clone();
        merged.clear();
        merged.set_offset(start);
        merged.set_size(size);
        let properties = merged.properties.clone();
        // A merge is an explicit structural operation, independent of paint locks.
        merged.properties.is_locked = false;
        merged.properties.is_visible = true;
        merged.properties.is_alpha_channel_locked = false;
        for source in [&base, &top] {
            for y in 0..source.height() {
                for x in 0..source.width() {
                    let local = Position::new(x, y);
                    if source.is_grapheme_continuation(local) {
                        continue;
                    }
                    let mut character = source.char_at(local);
                    if !character.is_visible() {
                        continue;
                    }
                    let target = local + source.offset() - start;
                    let below = merged.char_at(target);
                    if below.is_visible() && (character.attribute.is_foreground_transparent() || character.attribute.is_background_transparent()) {
                        character = buffer.make_solid_color(character, below);
                    }
                    if let Some((text, _)) = source.grapheme_at(local) {
                        merged.set_grapheme(target, text, character.attribute)?;
                    } else {
                        merged.set_char(target, character);
                    }
                }
            }
            for sixel in &source.sixels {
                let mut sixel = sixel.clone();
                sixel.position += source.offset() - start;
                merged.sixels.push(sixel);
            }
        }
        merged.properties = properties;
        let before = self.get_caret().position();
        let document_caret = before + top.offset();
        let _undo = self.begin_atomic_undo("Merge Unicode layers");
        self.push_undo_action(EditorUndoOp::MergeLayerDown {
            index,
            merged_layer: Some(merged),
            orig_layers: None,
        })?;
        let after = self.unicode_caret_position(document_caret - start, false);
        self.push_undo_action(EditorUndoOp::ReverseCaretPosition { pos: after, old_pos: before })
    }

    pub(super) fn require_legacy_cells(&self) -> Result<()> {
        if self.get_buffer().unicode_width() {
            return Err(EngineError::Generic("This cell transformation is not supported for Unicode graphemes".into()));
        }
        Ok(())
    }

    pub(super) fn fill_unicode_selection(&mut self, character: AttributedChar) -> Result<()> {
        let index = self.get_current_layer()?;
        self.require_cell_layer(index)?;
        let mut layer = self.get_buffer().layers[index].clone();
        if layer.properties.is_locked || !layer.properties.is_visible {
            return Ok(());
        }
        if character.is_visible() && character.ch.to_string().width() != 1 {
            return Err(EngineError::Generic("Unicode selection fill requires a one-cell character".into()));
        }
        for y in 0..layer.height() {
            for x in 0..layer.width() {
                let pos = Position::new(x, y);
                if self.is_selected(pos + layer.offset()) {
                    if character.is_visible() {
                        layer.set_grapheme(pos, &character.ch.to_string(), character.attribute)?;
                    } else {
                        layer.set_char(pos, character);
                    }
                }
            }
        }
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description: "Fill Unicode selection".into(),
        })
    }

    pub(super) fn copy_unicode_text(&self) -> Option<String> {
        if !self.is_something_selected() {
            return None;
        }
        let area = self.selected_rectangle();
        let buffer = self.get_buffer();
        let mut result = String::new();
        for y in area.y_range() {
            let mut x = area.left();
            while x < area.right() {
                let pos = Position::new(x, y);
                if !self.is_selected(pos) {
                    x += 1;
                    continue;
                }
                let mut start = pos;
                while start.x > 0 && buffer.is_grapheme_continuation(start) {
                    start.x -= 1;
                }
                if let Some((text, width)) = buffer.grapheme_at(start) {
                    result.push_str(text);
                    x = start.x + width as i32;
                } else {
                    result.push(buffer.char_at(pos).ch);
                    x += 1;
                }
            }
            result.push('\n');
        }
        if self.selection().is_some_and(|sel| sel.shape == crate::Shape::Lines) {
            result.pop();
        }
        Some(result)
    }
    /// Normalize a cell position to a grapheme boundary. The right edge is a valid
    /// insertion position; typing there wraps to the next row if one exists.
    pub fn unicode_caret_position(&self, position: Position, forward: bool) -> Position {
        let Some(layer) = self.get_cur_layer() else { return position };
        let mut position = Position::new(position.x.clamp(0, layer.width().max(0)), position.y.clamp(0, (layer.height() - 1).max(0)));
        if let Some(start) = layer.grapheme_start(position) {
            if start != position {
                position.x = if forward {
                    start.x + layer.grapheme_at(start).map_or(1, |(_, width)| width as i32)
                } else {
                    start.x
                };
            }
        }
        position
    }

    pub(super) fn commit_unicode_layer(&mut self, layer: Layer, before: Position, after: Position, description: &str) -> Result<()> {
        let index = self.get_current_layer()?;
        self.require_cell_layer(index)?;
        let _undo = self.begin_atomic_undo(description);
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description: description.into(),
        })?;
        self.push_undo_action(EditorUndoOp::ReverseCaretPosition { pos: after, old_pos: before })
    }

    /// Type Unicode text as one undoable action, without codepage conversion.
    /// CR/LF are line breaks; other controls are rejected.
    pub fn type_unicode_text(&mut self, text: &str) -> Result<()> {
        if !self.get_buffer().unicode_width() {
            return Err(EngineError::Generic("Unicode text input requires a Unicode-width buffer".into()));
        }
        if text.chars().any(|ch| ch.is_control() && ch != '\r' && ch != '\n') {
            return Err(EngineError::Generic("Control characters are not supported in Unicode editor text".into()));
        }
        let _undo = self.begin_atomic_undo("Type Unicode text");
        let mut previous_cr = false;
        for ch in text.chars() {
            if ch == '\n' && previous_cr {
                previous_cr = false;
                continue;
            }
            self.type_unicode_key(ch)?;
            previous_cr = ch == '\r';
        }
        Ok(())
    }

    pub(super) fn type_unicode_key(&mut self, ch: char) -> Result<()> {
        if self.mirror_mode {
            return Err(EngineError::Generic("Mirror typing is not supported for Unicode graphemes".into()));
        }
        let index = self.get_current_layer()?;
        self.require_cell_layer(index)?;
        let before = self.get_caret().position();
        let mut pos = self.unicode_caret_position(before, false);
        let mut layer = self.get_buffer().layers[index].clone();
        if layer.properties.is_locked || !layer.properties.is_visible || layer.width() <= 0 || layer.height() <= 0 {
            return Ok(());
        }
        if ch == '\r' || ch == '\n' {
            if pos.y + 1 < layer.height() {
                let after = Position::new(0, pos.y + 1);
                return self.push_undo_action(EditorUndoOp::ReverseCaretPosition { pos: after, old_pos: before });
            }
            return Ok(());
        }
        if ch.is_control() {
            return Err(EngineError::Generic("Control characters are not supported in Unicode editor text".into()));
        }
        let mut text = ch.to_string();
        let mut attribute = self.get_caret().attribute;
        let mut previous_width = 0;
        // Re-segment the preceding grapheme so scalar-at-a-time input supports
        // accents, emoji modifiers, regional indicators and ZWJ sequences.
        if pos.x > 0 {
            let previous_cell = Position::new(pos.x - 1, pos.y);
            let start = layer.grapheme_start(previous_cell).unwrap_or(previous_cell);
            let previous = layer
                .grapheme_at(start)
                .map_or_else(|| layer.char_at(start).ch.to_string(), |(s, _)| s.to_owned());
            let joined = format!("{previous}{ch}");
            if joined.graphemes(true).count() == 1 {
                previous_width = layer.grapheme_at(start).map_or(1, |(_, width)| width as i32);
                text = joined;
                attribute = layer.char_at(start).attribute;
                pos = start;
            }
        }
        if text.width() == 0 {
            text.insert(0, '\u{25cc}');
        }
        let width = text.width() as i32;
        if !(1..=2).contains(&width) || text.len() > icy_engine::MAX_GRAPHEME_BYTES {
            return Err(EngineError::Generic("Unicode grapheme exceeds terminal cell or byte limits".into()));
        }
        if pos.x + width > layer.width() {
            if previous_width != 0 || pos.y + 1 >= layer.height() {
                return Ok(());
            }
            pos = Position::new(0, pos.y + 1);
        }
        if self.get_caret().insert_mode && width != previous_width {
            let start = Position::new(pos.x + previous_width.min(width), pos.y);
            layer.shift_cells(
                start,
                Position::new(layer.width() - 1, pos.y),
                Position::new(width - previous_width, 0),
                attribute,
            );
        }
        layer.set_grapheme(pos, &text, attribute)?;
        let after = Position::new(pos.x + width, pos.y);
        let _undo = self.begin_atomic_undo("Type Unicode grapheme");
        self.clear_selection()?;
        self.commit_unicode_layer(layer, before, after, "Type Unicode grapheme")
    }

    pub(super) fn delete_unicode(&mut self, backspace: bool) -> Result<()> {
        let before = self.get_caret().position();
        let mut pos = self.unicode_caret_position(before, false);
        let index = self.get_current_layer()?;
        self.require_cell_layer(index)?;
        let mut layer = self.get_buffer().layers[index].clone();
        if layer.properties.is_locked || !layer.properties.is_visible {
            return Ok(());
        }
        if backspace {
            if pos.x <= 0 {
                return Ok(());
            }
            pos.x -= 1;
            pos = layer.grapheme_start(pos).unwrap_or(pos);
        }
        if pos.x >= layer.width() {
            return Ok(());
        }
        let width = layer.grapheme_at(pos).map_or(1, |(_, width)| width as i32);
        let attribute = self.get_caret().attribute;
        if self.get_caret().insert_mode || !backspace {
            layer.shift_cells(pos, Position::new(layer.width() - 1, pos.y), Position::new(-width, 0), attribute);
        } else {
            for x in pos.x..pos.x + width {
                layer.set_char(Position::new(x, pos.y), AttributedChar::new(' ', attribute));
            }
        }
        self.commit_unicode_layer(layer, before, pos, if backspace { "Unicode backspace" } else { "Unicode delete" })
    }

    pub(super) fn set_unicode_char(&mut self, index: usize, pos: Position, character: AttributedChar) -> Result<()> {
        if index >= self.get_buffer().layers.len() {
            return Err(EngineError::Generic("Invalid layer index".into()));
        }
        self.require_cell_layer(index)?;
        if self.mirror_mode {
            return Err(EngineError::Generic("Mirroring Unicode cells is not supported".into()));
        }
        let layer = &self.get_buffer().layers[index];
        let mut text = character.ch.to_string();
        if let Some((existing, _)) = layer.grapheme_at(pos) {
            if existing.starts_with(character.ch) {
                text = existing.to_owned();
            }
        }
        if text.width() == 0 {
            text.insert(0, '\u{25cc}');
        }
        // Only the touched cells are kept for undo; shapes set hundreds of cells at once.
        let mut span = Box::new(layer.unicode_span(pos.y, pos.x..pos.x + text.width().clamp(1, 2) as i32));
        let buffer = self.get_buffer_mut();
        buffer.layers[index].set_grapheme(pos, &text, character.attribute)?;
        buffer.layers[index].swap_unicode_span(&mut span);
        self.push_undo_action(EditorUndoOp::ReplaceUnicodeSpan { layer: index, span })
    }
}
