#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::{
    collections::{BTreeMap, HashMap},
    mem,
};

use i18n_embed_fl::fl;

use crate::{AttributedChar, Position, Rectangle, Result, Selection, TextPane};

use super::{EditState, undo_operation::EditorUndoOp};

fn get_area(sel: Option<Selection>, layer: Rectangle) -> Rectangle {
    if let Some(selection) = sel {
        let rect = selection.as_rectangle();
        rect.intersect(&layer) - layer.start
    } else {
        layer - layer.start
    }
}

impl EditState {
    pub fn justify_left(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            let old_layer = crate::layer_from_area(layer, area);
            for y in area.y_range() {
                let mut removed_chars = 0;
                let len = area.width();
                while removed_chars < len {
                    let ch = layer.char_at((area.left() + removed_chars, y).into());
                    if ch.is_visible() && !ch.is_transparent() {
                        break;
                    }
                    removed_chars += 1;
                }
                if len <= removed_chars {
                    continue;
                }
                for x in area.x_range() {
                    let ch = if x + removed_chars < area.right() {
                        layer.char_at((x + removed_chars, y).into())
                    } else {
                        AttributedChar::invisible()
                    };
                    layer.set_char(Position::new(x, y), ch);
                }
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn center(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-center"));
        let sel = self.selection();
        self.justify_left()?;
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            let old_layer = crate::layer_from_area(layer, area);

            for y in area.y_range() {
                let mut removed_chars = 0;
                let len = area.width();
                while removed_chars < len {
                    let ch = layer.char_at((area.right() - removed_chars - 1, y).into());
                    if ch.is_visible() && !ch.is_transparent() {
                        break;
                    }
                    removed_chars += 1;
                }
                if len == removed_chars {
                    continue;
                }
                let removed_chars = removed_chars / 2;
                for x in area.x_range().rev() {
                    let ch = if x - removed_chars >= area.left() {
                        layer.char_at((x - removed_chars, y).into())
                    } else {
                        AttributedChar::invisible()
                    };

                    layer.set_char((x, y), ch);
                }
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn justify_right(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-right"));
        let sel = self.selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            let old_layer = crate::layer_from_area(layer, area);

            for y in area.y_range() {
                let mut removed_chars = 0;
                let len = area.width();
                while removed_chars < len {
                    let ch = layer.char_at((area.right() - removed_chars - 1, y).into());
                    if ch.is_visible() && !ch.is_transparent() {
                        break;
                    }
                    removed_chars += 1;
                }
                if len == removed_chars {
                    continue;
                }
                for x in area.x_range().rev() {
                    let ch = if x - removed_chars >= area.left() {
                        layer.char_at((x - removed_chars, y).into())
                    } else {
                        AttributedChar::invisible()
                    };

                    layer.set_char((x, y), ch);
                }
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn flip_x(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-flip-x"));
        let sel = self.selection();
        let mut flip_tables = HashMap::new();

        self.screen.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, generate_flipx_table(font));
        });

        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            let old_layer = crate::layer_from_area(layer, area);
            flip_layer_x(layer, area, &flip_tables);
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn flip_y(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-flip-y"));
        let sel = self.selection();

        let mut flip_tables = HashMap::new();

        self.screen.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, generate_flipy_table(font));
        });

        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            let old_layer = crate::layer_from_area(layer, area);
            flip_layer_y(layer, area, &flip_tables);
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn crop(&mut self) -> Result<()> {
        if let Some(sel) = self.selection() {
            let sel = sel.as_rectangle();
            self.crop_rect(sel)
        } else {
            Ok(())
        }
    }

    pub fn crop_rect(&mut self, rect: Rectangle) -> Result<()> {
        let old_size = self.get_buffer().size();
        let mut old_layers = Vec::new();
        mem::swap(&mut self.get_buffer_mut().layers, &mut old_layers);

        self.get_buffer_mut().set_size(rect.size);
        self.get_buffer_mut().layers.clear();

        for old_layer in &old_layers {
            let mut new_layer = old_layer.clone();
            new_layer.lines.clear();
            let new_rectangle = old_layer.rectangle().intersect(&rect);
            if new_rectangle.is_empty() {
                continue;
            }

            new_layer.set_offset(new_rectangle.start - rect.start);
            new_layer.set_size(new_rectangle.size);

            for y in 0..new_rectangle.height() {
                for x in 0..new_rectangle.width() {
                    let ch = old_layer.char_at((x + new_rectangle.left(), y + new_rectangle.top()).into());
                    new_layer.set_char((x, y), ch);
                }
            }
            self.get_buffer_mut().layers.push(new_layer);
        }
        let op = EditorUndoOp::Crop {
            orig_size: old_size,
            size: rect.size(),
            layers: old_layers,
        };
        self.push_plain_undo(op)
    }

    /// Returns the delete selection of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn erase_selection(&mut self) -> Result<()> {
        if !self.is_something_selected() {
            return Ok(());
        }
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));
        let layer_idx = self.get_current_layer()?;
        let (area, old_layer) = if let Some(layer) = self.screen.buffer.layers.get_mut(layer_idx) {
            (layer.rectangle(), layer.clone())
        } else {
            return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
        };

        for y in 0..area.height() {
            for x in 0..area.width() {
                let pos = Position::new(x, y);
                if self.is_selected(pos + area.start) {
                    self.screen.buffer.layers.get_mut(layer_idx).unwrap().set_char(pos, AttributedChar::invisible());
                }
            }
        }
        let new_layer = self.screen.buffer.layers.get_mut(layer_idx).unwrap().clone();
        let op = EditorUndoOp::LayerChange {
            layer: self.get_current_layer()?,
            pos: area.start,
            old_chars: old_layer,
            new_chars: new_layer,
        };
        let _ = self.push_plain_undo(op);
        self.clear_selection()
    }

    pub fn scroll_area_up(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            if area.is_empty() {
                return Ok(());
            }
            if area.width() >= layer.width() {
                let op = EditorUndoOp::ScrollWholeLayerUp {
                    layer: self.get_current_layer()?,
                };
                return self.push_undo_action(op);
            }

            let old_layer = crate::layer_from_area(layer, area);

            let mut saved_line = Vec::new();

            for y in area.y_range() {
                let line = &mut layer.lines[y as usize];
                if line.chars.len() < area.right() as usize {
                    line.chars.resize(area.right() as usize, AttributedChar::invisible());
                }
                if y == area.top() {
                    saved_line.extend(line.chars.drain(area.left() as usize..area.right() as usize));
                    continue;
                }
                if y == area.bottom() - 1 {
                    line.chars.splice(area.right() as usize..area.right() as usize, saved_line.iter().copied());
                }
                let chars = line.chars.drain(area.left() as usize..area.right() as usize).collect::<Vec<AttributedChar>>();
                let line_above = &mut layer.lines[y as usize - 1];
                line_above.chars.splice(area.left() as usize..area.left() as usize, chars);
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn scroll_area_down(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            if area.is_empty() {
                return Ok(());
            }
            if area.width() >= layer.width() {
                let op = EditorUndoOp::ScrollWholeLayerDown {
                    layer: self.get_current_layer()?,
                };
                return self.push_undo_action(op);
            }
            let old_layer = crate::layer_from_area(layer, area);

            let mut saved_line = Vec::new();

            for y in area.y_range().rev() {
                let line = &mut layer.lines[y as usize];
                if line.chars.len() < area.right() as usize {
                    line.chars.resize(area.right() as usize, AttributedChar::invisible());
                }
                if y == area.bottom() - 1 {
                    saved_line.extend(line.chars.drain(area.left() as usize..area.right() as usize));
                    continue;
                }
                if y == area.top() {
                    line.chars.splice(area.right() as usize..area.right() as usize, saved_line.iter().copied());
                }
                let chars = line.chars.drain(area.left() as usize..area.right() as usize).collect::<Vec<AttributedChar>>();
                let line_below = &mut layer.lines[y as usize + 1];
                line_below.chars.splice(area.left() as usize..area.left() as usize, chars);
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn scroll_area_left(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            if area.is_empty() {
                return Ok(());
            }
            let old_layer = crate::layer_from_area(layer, area);
            for y in area.y_range() {
                let line = &mut layer.lines[y as usize];
                if line.chars.len() < area.right() as usize {
                    line.chars.resize(area.right() as usize, AttributedChar::invisible());
                }
                let ch = line.chars.remove(area.left() as usize);
                line.chars.insert(area.right() as usize - 1, ch);
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    pub fn scroll_area_right(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.rectangle());
            if area.is_empty() {
                return Ok(());
            }
            let old_layer = crate::layer_from_area(layer, area);
            for y in area.y_range() {
                let line = &mut layer.lines[y as usize];
                if line.chars.len() < area.right() as usize {
                    line.chars.resize(area.right() as usize, AttributedChar::invisible());
                }
                let ch = line.chars.remove(area.right() as usize - 1);
                line.chars.insert(area.left() as usize, ch);
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: self.get_current_layer()?,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }
}

// ============================================================================
// Packed glyph data type for efficient flip table generation
// Uses [u8; 32] to match CompactGlyph.data format (one byte per row, MSB first)
// ============================================================================

type PackedGlyph = [u8; 32];

/// Extract packed glyph data from a CompactGlyph
#[inline]
fn get_packed_glyph(glyph: &crate::CompactGlyph) -> PackedGlyph {
    glyph.data
}

/// Check if two packed glyphs are equal (considering only valid height)
#[inline]
fn packed_eq(a: &PackedGlyph, b: &PackedGlyph, height: usize) -> bool {
    a[..height] == b[..height]
}

/// Generate flip-x variants for a packed glyph (horizontal flip)
fn generate_flipx_variants_packed(glyph: &PackedGlyph, height: usize, font_width: i32) -> Option<Vec<PackedGlyph>> {
    let w = 8 - font_width as u32;
    let mut flipped: PackedGlyph = [0; 32];
    for i in 0..height {
        flipped[i] = glyph[i].reverse_bits() << w;
    }

    if packed_eq(glyph, &flipped, height) {
        return None;
    }

    Some(generate_x_variants_packed(&flipped, height))
}

/// Generate horizontal shift variants for a packed glyph
fn generate_x_variants_packed(glyph: &PackedGlyph, height: usize) -> Vec<PackedGlyph> {
    let mut variants = Vec::with_capacity(5);
    variants.push(*glyph);

    // Shift left by 1
    let mut left: PackedGlyph = [0; 32];
    for i in 0..height {
        left[i] = glyph[i] << 1;
    }
    variants.push(left);

    // Shift left by 2
    let mut left2: PackedGlyph = [0; 32];
    for i in 0..height {
        left2[i] = glyph[i] << 2;
    }
    variants.push(left2);

    // Shift right by 1
    let mut right: PackedGlyph = [0; 32];
    for i in 0..height {
        right[i] = glyph[i] >> 1;
    }
    variants.push(right);

    // Shift right by 2
    let mut right2: PackedGlyph = [0; 32];
    for i in 0..height {
        right2[i] = glyph[i] >> 2;
    }
    variants.push(right2);

    variants
}

/// Generate flip-y variants for a packed glyph (vertical flip)
fn generate_flipy_variants_packed(glyph: &PackedGlyph, height: usize) -> Option<Vec<PackedGlyph>> {
    let mut flipped: PackedGlyph = [0; 32];
    for i in 0..height {
        flipped[i] = glyph[height - 1 - i];
    }

    if packed_eq(glyph, &flipped, height) {
        return None;
    }

    Some(generate_y_variants_packed(&flipped, height))
}

/// Generate vertical shift variants for a packed glyph
fn generate_y_variants_packed(glyph: &PackedGlyph, height: usize) -> Vec<PackedGlyph> {
    let mut variants = Vec::with_capacity(5);
    variants.push(*glyph);

    // Shift up by 1
    let mut up: PackedGlyph = [0; 32];
    for i in 0..height.saturating_sub(1) {
        up[i] = glyph[i + 1];
    }
    variants.push(up);

    // Shift up by 2
    let mut up2: PackedGlyph = [0; 32];
    for i in 0..height.saturating_sub(2) {
        up2[i] = glyph[i + 2];
    }
    variants.push(up2);

    // Shift down by 1
    let mut down: PackedGlyph = [0; 32];
    for i in 1..height {
        down[i] = glyph[i - 1];
    }
    variants.push(down);

    // Shift down by 2
    let mut down2: PackedGlyph = [0; 32];
    for i in 2..height {
        down2[i] = glyph[i - 2];
    }
    variants.push(down2);

    variants
}

/// Negate packed glyph data (invert all bits)
fn negate_packed(glyph: &PackedGlyph, height: usize) -> PackedGlyph {
    let mut neg: PackedGlyph = [0; 32];
    for i in 0..height {
        neg[i] = !glyph[i];
    }
    neg
}

pub fn generate_flipy_table(font: &crate::BitFont) -> BTreeMap<char, (bool, char)> {
    let mut flip_table = BTreeMap::new();

    // List of characters that should never be included in the flip table
    // These are symmetrical characters or ones that produce false matches
    let excluded_chars = [
        0 as char,   // null
        32 as char,  // space
        46 as char,  // . (period)
        95 as char,  // _ (underscore)
        196 as char, // ─ (horizontal line)
        205 as char, // ═ (double horizontal line)
        219 as char, // █ (full block)
        255 as char, // non-breaking space
    ];

    let height = font.height as usize;

    // Add known vertical flip mappings for box-drawing characters
    let vertical_pairs = [
        (183, 189),
        (184, 190),
        (187, 188),
        (191, 217),
        (192, 218),
        (193, 194),
        (200, 201),
        (202, 203),
        (207, 209),
        (208, 210),
        (211, 214),
        (212, 213),
        (220, 223),
        (24, 25),
        (30, 31),
        (33, 173),
    ];

    for (ch1, ch2) in vertical_pairs {
        if let (Some(c1), Some(c2)) = (char::from_u32(ch1), char::from_u32(ch2)) {
            flip_table.insert(c1, (false, c2));
            flip_table.insert(c2, (false, c1));
        }
    }

    for ch_code in 0u8..=255 {
        let ch = ch_code as char;
        if excluded_chars.contains(&ch) {
            continue;
        }

        let cur_glyph = get_packed_glyph(font.glyph(ch));

        let Some(flipped_variants) = generate_flipy_variants_packed(&cur_glyph, height) else {
            continue;
        };

        'outer: for ch2_code in 0u8..=255 {
            let ch2 = ch2_code as char;
            if ch == ch2 || excluded_chars.contains(&ch2) {
                continue;
            }

            let cmp_glyph = get_packed_glyph(font.glyph(ch2));
            let cmp_variants = generate_y_variants_packed(&cmp_glyph, height);

            for cmp in &cmp_variants {
                for (i, flipped) in flipped_variants.iter().enumerate() {
                    if packed_eq(flipped, cmp, height) {
                        flip_table.insert(ch, (false, ch2));
                        break 'outer;
                    }
                    // Check negated version
                    let neg = negate_packed(flipped, height);
                    if packed_eq(&neg, cmp, height) {
                        flip_table.insert(ch, (true, ch2));
                        break 'outer;
                    }
                }
            }
        }
    }
    check_bidirect(&mut flip_table);
    flip_table
}

pub fn generate_flipx_table(font: &crate::BitFont) -> BTreeMap<char, (bool, char)> {
    let mut flip_table = BTreeMap::new();

    // Hardcoded slash mappings
    flip_table.insert('\\', (false, '/'));
    flip_table.insert('/', (false, '\\'));

    // Characters that should never be included
    let excluded_chars = [45 as char, 186 as char, 61 as char]; // -, ║, =

    let height = font.height as usize;
    let font_width = font.width as i32;

    for ch_code in 0u8..=255 {
        let ch = ch_code as char;
        if excluded_chars.contains(&ch) {
            continue;
        }

        let cur_glyph = get_packed_glyph(font.glyph(ch));

        let Some(flipped_variants) = generate_flipx_variants_packed(&cur_glyph, height, font_width) else {
            continue;
        };

        'outer: for ch2_code in 0u8..=255 {
            let ch2 = ch2_code as char;
            if ch == ch2 || excluded_chars.contains(&ch2) {
                continue;
            }

            if ch2 == 186 as char && ch != 186 as char {
                continue;
            }

            let cmp_glyph = get_packed_glyph(font.glyph(ch2));
            let cmp_variants = generate_x_variants_packed(&cmp_glyph, height);

            for (idx, cmp) in cmp_variants.iter().enumerate() {
                for (i, flipped) in flipped_variants.iter().enumerate() {
                    // Only accept exact flips for problematic characters
                    if (ch == 186 as char || ch2 == 186 as char) && (i != 0 || idx != 0) {
                        continue;
                    }

                    if packed_eq(flipped, cmp, height) {
                        flip_table.insert(ch, (false, ch2));
                        break 'outer;
                    }
                    let neg = negate_packed(flipped, height);
                    if packed_eq(&neg, cmp, height) {
                        flip_table.insert(ch, (true, ch2));
                        break 'outer;
                    }
                }
            }
        }
    }
    check_bidirect(&mut flip_table);
    flip_table
}

fn check_bidirect(flip_table: &mut BTreeMap<char, (bool, char)>) {
    let original_table = flip_table.clone();

    // Don't clear the table, instead add missing bidirectional mappings
    for (ch1, (flip1, ch2)) in &original_table {
        // Skip self-mappings
        if ch1 == ch2 {
            continue;
        }

        // Check if the reverse mapping exists
        if let Some((flip2, ch3)) = original_table.get(ch2) {
            if *ch3 == *ch1 && *flip1 == *flip2 {
                // Ensure both directions are present
                flip_table.insert(*ch2, (*flip2, *ch3));
            }
        }
    }
}

// Update the map_char function to accept BTreeMap as well
pub fn map_char(mut ch: AttributedChar, table: &BTreeMap<char, (bool, char)>) -> AttributedChar {
    if let Some((flip, repl)) = table.get(&(ch.ch)) {
        ch.ch = *repl;
        if *flip {
            let tmp = ch.attribute.foreground();
            ch.attribute.set_foreground(ch.attribute.background());
            ch.attribute.set_background(tmp);
        }
    }
    ch
}

/// Flip a layer horizontally in-place within the given area.
/// This is the shared implementation used by both `flip_x` and `paste_flip_x`.
pub(crate) fn flip_layer_x(layer: &mut crate::Layer, area: Rectangle, flip_tables: &HashMap<u8, BTreeMap<char, (bool, char)>>) {
    let max = area.width() / 2;
    for y in area.y_range() {
        for x in 0..max {
            let pos1 = Position::new(area.left() + x, y);
            let pos2 = Position::new(area.right() - x - 1, y);

            let pos1ch = layer.char_at(pos1);
            let pos1ch = map_char(pos1ch, flip_tables.get(&pos1ch.font_page()).unwrap());
            let pos2ch = layer.char_at(pos2);
            let pos2ch = map_char(pos2ch, flip_tables.get(&pos2ch.font_page()).unwrap());
            layer.set_char(pos1, pos2ch);
            layer.set_char(pos2, pos1ch);
        }
    }
}

/// Flip a layer vertically in-place within the given area.
/// This is the shared implementation used by both `flip_y` and `paste_flip_y`.
pub(crate) fn flip_layer_y(layer: &mut crate::Layer, area: Rectangle, flip_tables: &HashMap<u8, BTreeMap<char, (bool, char)>>) {
    let max = area.height() / 2;
    for x in area.x_range() {
        for y in 0..max {
            let pos1 = Position::new(x, area.top() + y);
            let pos2 = Position::new(x, area.bottom() - 1 - y);
            let pos1ch = layer.char_at(pos1);
            let pos1ch = map_char(pos1ch, flip_tables.get(&pos1ch.font_page()).unwrap());
            let pos2ch = layer.char_at(pos2);
            let pos2ch = map_char(pos2ch, flip_tables.get(&pos2ch.font_page()).unwrap());
            layer.set_char(pos1, pos2ch);
            layer.set_char(pos2, pos1ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use crate::{
        BitFont, Layer, Position, Rectangle, Size, TextPane,
        editor::{EditState, UndoState},
    };

    use super::{generate_flipx_table, generate_flipy_table};

    #[test]
    fn test_generate_flipx_table() {
        let table: BTreeMap<char, (bool, char)> = generate_flipx_table(&BitFont::default());
        let cp437_table = HashMap::from([
            (40 as char, 41 as char),   // ( ↔ )
            (41 as char, 40 as char),   // ) ↔ (
            (47 as char, 92 as char),   // / ↔ \
            (92 as char, 47 as char),   // \ ↔ /
            (60 as char, 62 as char),   // < ↔ >
            (62 as char, 60 as char),   // > ↔ <
            (91 as char, 93 as char),   // [ ↔ ]
            (93 as char, 91 as char),   // ] ↔ [
            (123 as char, 125 as char), // { ↔ }
            (125 as char, 123 as char), // } ↔ {
            (169 as char, 170 as char), // ⌐ ↔ ¬
            (170 as char, 169 as char), // ¬ ↔ ⌐
            (174 as char, 175 as char), // « ↔ »
            (175 as char, 174 as char), // » ↔ «
            (180 as char, 195 as char), // ┤ ↔ ├
            (195 as char, 180 as char), // ├ ↔ ┤
            (181 as char, 198 as char), // ╡ ↔ ╞
            (198 as char, 181 as char), // ╞ ↔ ╡
            (182 as char, 199 as char), // ╢ ↔ ╟
            (199 as char, 182 as char), // ╟ ↔ ╢
            (183 as char, 214 as char), // ╖ ↔ ╓
            (214 as char, 183 as char), // ╓ ↔ ╖
            (185 as char, 204 as char), // ╣ ↔ ╠
            (204 as char, 185 as char), // ╠ ↔ ╣
            (187 as char, 201 as char), // ╗ ↔ ╔
            (201 as char, 187 as char), // ╔ ↔ ╗
            (188 as char, 200 as char), // ╝ ↔ ╚
            (200 as char, 188 as char), // ╚ ↔ ╝
            (189 as char, 211 as char), // ╜ ↔ ╙
            (211 as char, 189 as char), // ╙ ↔ ╜
            (190 as char, 212 as char), // ╛ ↔ ╘
            (212 as char, 190 as char), // ╘ ↔ ╛
            (191 as char, 218 as char), // ┐ ↔ ┌
            (218 as char, 191 as char), // ┌ ↔ ┐
            (192 as char, 217 as char), // └ ↔ ┘
            (217 as char, 192 as char), // ┘ ↔ └
            (221 as char, 222 as char), // ▌ ↔ ▐
            (222 as char, 221 as char), // ▐ ↔ ▌
            (242 as char, 243 as char), // ≥ ↔ ≤
            (243 as char, 242 as char), // ≤ ↔ ≥
            (27 as char, 26 as char),   // ← ↔ →
            (26 as char, 27 as char),   // → ↔ ←
            ('p', 'q'),                 // p ↔ q
            ('q', 'p'),                 // q ↔ p
            (17 as char, 16 as char),   // ◄ ↔ ►
            (16 as char, 17 as char),   // ► ↔ ◄
            (213 as char, 184 as char), // ╒ ↔ ╕
            (184 as char, 213 as char), // ╕ ↔ ╒
        ]);

        for k in table.keys() {
            assert!(cp437_table.contains_key(k), "invalid key in flip table {}", *k as u32);
        }
        for k in cp437_table.keys() {
            assert!(table.contains_key(k), "missing key {}", *k as u32);
        }
    }
    #[test]
    fn test_generate_flipy_table() {
        let table = generate_flipy_table(&BitFont::default());
        let cp437_table = HashMap::from([
            (183 as char, 189 as char),
            (189 as char, 183 as char),
            (184 as char, 190 as char),
            (190 as char, 184 as char),
            (187 as char, 188 as char),
            (188 as char, 187 as char),
            (191 as char, 217 as char),
            (217 as char, 191 as char),
            (192 as char, 218 as char),
            (218 as char, 192 as char),
            (193 as char, 194 as char),
            (194 as char, 193 as char),
            (200 as char, 201 as char),
            (201 as char, 200 as char),
            (202 as char, 203 as char),
            (203 as char, 202 as char),
            (207 as char, 209 as char),
            (209 as char, 207 as char),
            (208 as char, 210 as char),
            (210 as char, 208 as char),
            (211 as char, 214 as char),
            (214 as char, 211 as char),
            (212 as char, 213 as char),
            (213 as char, 212 as char),
            (220 as char, 223 as char),
            (223 as char, 220 as char),
            (24 as char, 25 as char),
            (25 as char, 24 as char),
            (30 as char, 31 as char),
            (31 as char, 30 as char),
            (33 as char, 173 as char),
            (173 as char, 33 as char),
        ]);
        for k in table.keys() {
            assert!(cp437_table.contains_key(k), "invalid key in flip table {}", *k as u32);
        }
        for k in cp437_table.keys() {
            assert!(table.contains_key(k), "missing key {}", *k as u32);
        }
    }

    #[test]
    fn test_delete_selection() {
        let mut state = EditState::default();
        for y in 0..20 {
            for x in 0..20 {
                state.set_char((x, y), '#'.into()).unwrap();
            }
        }

        let rect = Rectangle::from(5, 5, 9, 9);
        state.set_selection(rect).unwrap();
        state.erase_selection().unwrap();
        for y in 0..20 {
            for x in 0..20 {
                let pos = Position::new(x, y);
                let ch = state.get_buffer().char_at(pos);

                if rect.is_inside(pos) {
                    assert_eq!(ch.ch, ' ');
                } else {
                    assert_eq!(ch.ch, '#');
                }
            }
        }

        state.undo().unwrap();

        for y in 0..20 {
            for x in 0..20 {
                let pos = Position::new(x, y);
                let ch = state.get_buffer().char_at(pos);
                assert_eq!(ch.ch, '#');
            }
        }
    }

    #[test]
    fn test_flip_x() {
        let mut state = EditState::default();
        for y in 0..20 {
            for x in 0..20 {
                state.set_char((x, y), '#'.into()).unwrap();
            }
        }

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.erase_selection().unwrap();

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.set_char((3, 5), '#'.into()).unwrap();
        state.set_char((0, 9), '#'.into()).unwrap();

        state.flip_x().unwrap();
        for y in 10..20 {
            for x in 10..20 {
                let ch = state.get_buffer().char_at((x, y).into());
                assert_eq!(ch.ch, '#');
            }
        }

        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());
                if x == 9 && y == 9 || x == 6 && y == 5 {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }

        state.undo().unwrap();
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());

                if x == 3 && y == 5 || x == 0 && y == 9 {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }
    }

    #[test]
    fn test_flip_y() {
        let mut state = EditState::default();
        for y in 0..20 {
            for x in 0..20 {
                state.set_char((x, y), '#'.into()).unwrap();
            }
        }

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.erase_selection().unwrap();

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.set_char((3, 3), '#'.into()).unwrap();
        state.set_char((9, 9), '#'.into()).unwrap();

        state.flip_y().unwrap();
        for y in 10..20 {
            for x in 10..20 {
                let ch = state.get_buffer().char_at((x, y).into());
                assert_eq!(ch.ch, '#');
            }
        }

        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());
                if x == 9 && y == 0 || x == 3 && y == 6 {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }

        state.undo().unwrap();
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());

                if x == 3 && y == 3 || x == 9 && y == 9 {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }
    }

    #[test]
    fn test_justify_right() {
        let mut state = EditState::default();
        for y in 0..20 {
            for x in 0..20 {
                state.set_char((x, y), '#'.into()).unwrap();
            }
        }

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.erase_selection().unwrap();

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.set_char((5, 5), '#'.into()).unwrap();
        state.set_char((0, 9), '#'.into()).unwrap();

        state.justify_right().unwrap();

        for y in 10..20 {
            for x in 10..20 {
                let ch = state.get_buffer().char_at((x, y).into());
                assert_eq!(ch.ch, '#');
            }
        }

        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());
                if x == 9 && (y == 5 || y == 9) {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }

        state.undo().unwrap();
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());

                if x == 5 && y == 5 || x == 0 && y == 9 {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }
    }

    #[test]
    fn test_center() {
        let mut state = EditState::default();
        for y in 0..20 {
            for x in 0..20 {
                state.set_char((x, y), '#'.into()).unwrap();
            }
        }

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.erase_selection().unwrap();

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.set_char((0, 5), '#'.into()).unwrap();
        state.set_char((9, 9), '#'.into()).unwrap();

        state.center().unwrap();

        for y in 10..20 {
            for x in 10..20 {
                let ch = state.get_buffer().char_at((x, y).into());
                assert_eq!(ch.ch, '#');
            }
        }
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());
                if x == 4 && (y == 5 || y == 9) {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }
        state.undo().unwrap();
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());

                if x == 0 && y == 5 || x == 9 && y == 9 {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }
    }

    #[test]
    fn test_justify_left() {
        let mut state = EditState::default();
        for y in 0..20 {
            for x in 0..20 {
                state.set_char((x, y), '#'.into()).unwrap();
            }
        }

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.erase_selection().unwrap();

        state.set_selection(Rectangle::from(0, 0, 10, 10)).unwrap();
        state.set_char((5, 5), '#'.into()).unwrap();
        state.set_char((9, 9), '#'.into()).unwrap();

        state.justify_left().unwrap();

        for y in 10..20 {
            for x in 10..20 {
                let ch = state.get_buffer().char_at((x, y).into());
                assert_eq!(ch.ch, '#');
            }
        }
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());
                if x == 0 && (y == 5 || y == 9) {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }

        state.undo().unwrap();
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().char_at((x, y).into());

                if x == 5 && y == 5 || x == 9 && y == 9 {
                    assert_eq!(ch.ch, '#');
                } else {
                    assert_eq!(ch.ch, ' ');
                }
            }
        }
    }

    #[test]
    fn test_crop() {
        let mut state = EditState::default();

        let mut layer = Layer::new("1", Size::new(100, 100));
        layer.set_offset((-5, -5));
        state.get_buffer_mut().layers.push(layer);

        let mut layer = Layer::new("2", Size::new(2, 2));
        layer.set_offset((7, 6));
        state.get_buffer_mut().layers.push(layer);

        state.set_selection(Rectangle::from(5, 5, 5, 4)).unwrap();

        state.crop().unwrap();

        assert_eq!(state.get_buffer().width(), 5);
        assert_eq!(state.get_buffer().height(), 4);
        assert_eq!(state.get_buffer().layers[1].size(), Size::new(5, 4));
        assert_eq!(state.get_buffer().layers[2].size(), Size::new(2, 2));

        state.undo().unwrap();

        assert_eq!(state.get_buffer().width(), 80);
        assert_eq!(state.get_buffer().height(), 25);
        assert_eq!(state.get_buffer().layers[1].size(), Size::new(100, 100));
        assert_eq!(state.get_buffer().layers[2].size(), Size::new(2, 2));
    }
}
