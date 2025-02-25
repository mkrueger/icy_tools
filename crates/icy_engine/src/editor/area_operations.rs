#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::{collections::HashMap, mem};

use i18n_embed_fl::fl;

use crate::{AttributedChar, EngineResult, Layer, Position, Rectangle, Selection, TextPane};

use super::{EditState, EditorError};

fn get_area(sel: Option<Selection>, layer: Rectangle) -> Rectangle {
    if let Some(selection) = sel {
        let rect = selection.as_rectangle();
        rect.intersect(&layer) - layer.start
    } else {
        layer - layer.start
    }
}

impl EditState {
    pub fn justify_left(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.get_selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            let old_layer = Layer::from_layer(layer, area);
            for y in area.y_range() {
                let mut removed_chars = 0;
                let len = area.get_width();
                while removed_chars < len {
                    let ch = layer.get_char((area.left() + removed_chars, y));
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
                        layer.get_char((x + removed_chars, y))
                    } else {
                        AttributedChar::invisible()
                    };
                    layer.set_char(Position::new(x, y), ch);
                }
            }
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(super::EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn center(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-center"));
        let sel = self.get_selection();
        self.justify_left()?;
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            let old_layer = Layer::from_layer(layer, area);

            for y in area.y_range() {
                let mut removed_chars = 0;
                let len = area.get_width();
                while removed_chars < len {
                    let ch = layer.get_char((area.right() - removed_chars - 1, y));
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
                        layer.get_char((x - removed_chars, y))
                    } else {
                        AttributedChar::invisible()
                    };

                    layer.set_char((x, y), ch);
                }
            }
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn justify_right(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-right"));
        let sel = self.get_selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            let old_layer = Layer::from_layer(layer, area);

            for y in area.y_range() {
                let mut removed_chars = 0;
                let len = area.get_width();
                while removed_chars < len {
                    let ch = layer.get_char((area.right() - removed_chars - 1, y));
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
                        layer.get_char((x - removed_chars, y))
                    } else {
                        AttributedChar::invisible()
                    };

                    layer.set_char((x, y), ch);
                }
            }
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn flip_x(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-flip-x"));
        let sel = self.get_selection();
        let mut flip_tables = HashMap::new();

        self.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, generate_flipx_table(font));
        });

        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            let old_layer = Layer::from_layer(layer, area);
            let max = area.get_width() / 2;

            for y in area.y_range() {
                for x in 0..max {
                    let pos1 = Position::new(area.left() + x, y);
                    let pos2 = Position::new(area.right() - x - 1, y);

                    let pos1ch = layer.get_char(pos1);
                    let pos1ch = map_char(pos1ch, flip_tables.get(&pos1ch.get_font_page()).unwrap());
                    let pos2ch = layer.get_char(pos2);
                    let pos2ch = map_char(pos2ch, flip_tables.get(&pos2ch.get_font_page()).unwrap());
                    layer.set_char(pos1, pos2ch);
                    layer.set_char(pos2, pos1ch);
                }
            }
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn flip_y(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-flip-x"));
        let sel = self.get_selection();

        let mut flip_tables = HashMap::new();

        self.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, generate_flipy_table(font));
        });

        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            let old_layer = Layer::from_layer(layer, area);
            let max = area.get_height() / 2;

            for x in area.x_range() {
                for y in 0..max {
                    let pos1 = Position::new(x, area.top() + y);
                    let pos2 = Position::new(x, area.bottom() - 1 - y);
                    let pos1ch = layer.get_char(pos1);
                    let pos1ch = map_char(pos1ch, flip_tables.get(&pos1ch.get_font_page()).unwrap());
                    let pos2ch = layer.get_char(pos2);
                    let pos2ch = map_char(pos2ch, flip_tables.get(&pos2ch.get_font_page()).unwrap());
                    layer.set_char(pos1, pos2ch);
                    layer.set_char(pos2, pos1ch);
                }
            }
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn crop(&mut self) -> EngineResult<()> {
        if let Some(sel) = self.get_selection() {
            let sel = sel.as_rectangle();
            self.crop_rect(sel)
        } else {
            Ok(())
        }
    }

    pub fn crop_rect(&mut self, rect: Rectangle) -> EngineResult<()> {
        let old_size = self.get_buffer().get_size();
        let mut old_layers = Vec::new();
        mem::swap(&mut self.get_buffer_mut().layers, &mut old_layers);

        self.get_buffer_mut().set_size(rect.size);
        self.get_buffer_mut().layers.clear();

        for old_layer in &old_layers {
            let mut new_layer = old_layer.clone();
            new_layer.lines.clear();
            let new_rectangle = old_layer.get_rectangle().intersect(&rect);
            if new_rectangle.is_empty() {
                continue;
            }

            new_layer.set_offset(new_rectangle.start - rect.start);
            new_layer.set_size(new_rectangle.size);

            for y in 0..new_rectangle.get_height() {
                for x in 0..new_rectangle.get_width() {
                    let ch = old_layer.get_char((x + new_rectangle.left(), y + new_rectangle.top()));
                    new_layer.set_char((x, y), ch);
                }
            }
            self.get_buffer_mut().layers.push(new_layer);
        }
        let op = super::undo_operations::Crop::new(old_size, rect.get_size(), old_layers);
        self.push_plain_undo(Box::new(op))
    }

    /// Returns the delete selection of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn erase_selection(&mut self) -> EngineResult<()> {
        if !self.is_something_selected() {
            return Ok(());
        }
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));
        let layer_idx = self.get_current_layer()?;
        let (area, old_layer) = if let Some(layer) = self.buffer.layers.get_mut(layer_idx) {
            (layer.get_rectangle(), layer.clone())
        } else {
            return Err(EditorError::CurrentLayerInvalid.into());
        };

        for y in 0..area.get_height() {
            for x in 0..area.get_width() {
                let pos = Position::new(x, y);
                if self.get_is_selected(pos + area.start) {
                    self.buffer.layers.get_mut(layer_idx).unwrap().set_char(pos, AttributedChar::invisible());
                }
            }
        }
        let new_layer = self.buffer.layers.get_mut(layer_idx).unwrap().clone();
        let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
        let _ = self.push_plain_undo(Box::new(op));
        self.clear_selection()
    }

    pub fn scroll_area_up(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.get_selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            if area.is_empty() {
                return Ok(());
            }
            if area.get_width() >= layer.get_width() {
                let op = super::undo_operations::UndoScrollWholeLayerUp::new(self.get_current_layer()?);
                return self.push_undo_action(Box::new(op));
            }

            let old_layer = Layer::from_layer(layer, area);

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
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(super::EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn scroll_area_down(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.get_selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            if area.is_empty() {
                return Ok(());
            }
            if area.get_width() >= layer.get_width() {
                let op = super::undo_operations::UndoScrollWholeLayerDown::new(self.get_current_layer()?);
                return self.push_undo_action(Box::new(op));
            }
            let old_layer = Layer::from_layer(layer, area);

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
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(super::EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn scroll_area_left(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.get_selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            if area.is_empty() {
                return Ok(());
            }
            let old_layer = Layer::from_layer(layer, area);
            for y in area.y_range() {
                let line = &mut layer.lines[y as usize];
                if line.chars.len() < area.right() as usize {
                    line.chars.resize(area.right() as usize, AttributedChar::invisible());
                }
                let ch = line.chars.remove(area.left() as usize);
                line.chars.insert(area.right() as usize - 1, ch);
            }
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(super::EditorError::CurrentLayerInvalid.into())
        }
    }

    pub fn scroll_area_right(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-justify-left"));
        let sel = self.get_selection();
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = get_area(sel, layer.get_rectangle());
            if area.is_empty() {
                return Ok(());
            }
            let old_layer = Layer::from_layer(layer, area);
            for y in area.y_range() {
                let line = &mut layer.lines[y as usize];
                if line.chars.len() < area.right() as usize {
                    line.chars.resize(area.right() as usize, AttributedChar::invisible());
                }
                let ch = line.chars.remove(area.right() as usize - 1);
                line.chars.insert(area.left() as usize, ch);
            }
            let new_layer = Layer::from_layer(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(self.get_current_layer()?, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(super::EditorError::CurrentLayerInvalid.into())
        }
    }
}

fn generate_flipy_table(font: &crate::BitFont) -> HashMap<char, (bool, char)> {
    let mut flip_table = HashMap::new();

    for (ch, cur_glyph) in &font.glyphs {
        let flipped_glyhps = generate_flipy_variants(cur_glyph);
        let Some(flipped_glyhps) = flipped_glyhps else {
            continue;
        };
        let neg_glyphs = negate_glyphs(&flipped_glyhps);

        for (ch2, cmp_glyph) in &font.glyphs {
            if ch == ch2 {
                continue;
            }
            let cmp_glyphs = generate_y_variants(cmp_glyph);
            for cmp_glyph in cmp_glyphs {
                for i in 0..flipped_glyhps.len() {
                    if flipped_glyhps[i].data == cmp_glyph.data {
                        flip_table.insert(*ch, (false, *ch2));
                        break;
                    }
                    if neg_glyphs[i].data == cmp_glyph.data {
                        flip_table.insert(*ch, (true, *ch2));
                        break;
                    }
                }
            }
        }
    }
    check_bidirect(&mut flip_table);
    flip_table
}

fn check_bidirect(flip_table: &mut HashMap<char, (bool, char)>) {
    for (ch, (_b, ch2)) in &flip_table.clone() {
        if !flip_table.contains_key(ch2) {
            flip_table.remove(ch);
        }
    }
}

fn generate_flipx_table(font: &crate::BitFont) -> HashMap<char, (bool, char)> {
    let mut flip_table = HashMap::new();

    flip_table.insert('\\', (false, '/'));
    flip_table.insert('/', (false, '\\'));

    for (ch, cur_glyph) in &font.glyphs {
        let flipped_glyhps: Option<Vec<crate::Glyph>> = generate_flipx_variants(cur_glyph, font.size.width);
        let Some(flipped_glyhps) = flipped_glyhps else {
            continue;
        };
        let neg_glyphs = negate_glyphs(&flipped_glyhps);

        for (ch2, cmp_glyph) in &font.glyphs {
            if ch == ch2 {
                continue;
            }
            let cmp_glyphs = generate_x_variants(cmp_glyph, font.size.width);

            for cmp_glyph in cmp_glyphs {
                for i in 0..flipped_glyhps.len() {
                    if flipped_glyhps[i].data == cmp_glyph.data {
                        flip_table.insert(*ch, (false, *ch2));
                        break;
                    }
                    if neg_glyphs[i].data == cmp_glyph.data {
                        flip_table.insert(*ch, (true, *ch2));
                        break;
                    }
                }
            }
        }
    }
    check_bidirect(&mut flip_table);
    flip_table
}

fn negate_glyphs(flipped_glyhps: &Vec<crate::Glyph>) -> Vec<crate::Glyph> {
    let mut neg_glyhps = Vec::new();
    for flipped_glyph in flipped_glyhps {
        let mut neg_glyph = flipped_glyph.clone();
        for i in 0..neg_glyph.data.len() {
            neg_glyph.data[i] = (!neg_glyph.data[i]) & 0xFF;
        }
        neg_glyhps.push(neg_glyph);
    }
    neg_glyhps
}

fn generate_flipx_variants(cur_glyph: &crate::Glyph, font_width: i32) -> Option<Vec<crate::Glyph>> {
    let mut flipped_glyph = cur_glyph.clone();
    let w = 8 - font_width;

    for i in 0..flipped_glyph.data.len() {
        flipped_glyph.data[i] = ((flipped_glyph.data[i] as u8).reverse_bits() << w) as u32;
    }
    if cur_glyph.data == flipped_glyph.data {
        return None;
    }
    Some(generate_x_variants(&flipped_glyph, font_width))
}

fn generate_x_variants(flipped_glyph: &crate::Glyph, _font_width: i32) -> Vec<crate::Glyph> {
    let mut cmp_glyhps = vec![flipped_glyph.clone()];

    let mut left_glyph = cmp_glyhps[0].clone();
    for i in 0..left_glyph.data.len() {
        left_glyph.data[i] <<= 1;
        left_glyph.data[i] &= 0xFF;
    }
    let mut left_by2_glyph = left_glyph.clone();
    for i in 0..left_glyph.data.len() {
        left_by2_glyph.data[i] <<= 1;
        left_by2_glyph.data[i] &= 0xFF;
    }
    cmp_glyhps.push(left_glyph);
    cmp_glyhps.push(left_by2_glyph);

    let mut right_glyph = cmp_glyhps[0].clone();
    for i in 0..right_glyph.data.len() {
        right_glyph.data[i] >>= 1;
        right_glyph.data[i] &= 0xFF;
    }
    let mut right_by2_glyph = right_glyph.clone();
    for i in 0..right_glyph.data.len() {
        right_by2_glyph.data[i] >>= 1;
        right_by2_glyph.data[i] &= 0xFF;
    }
    cmp_glyhps.push(right_glyph);
    cmp_glyhps.push(right_by2_glyph);

    cmp_glyhps
}

fn generate_flipy_variants(cur_glyph: &crate::Glyph) -> Option<Vec<crate::Glyph>> {
    let mut flipped_glyph = cur_glyph.clone();
    flipped_glyph.data = cur_glyph.data.iter().rev().copied().collect();
    if cur_glyph.data == flipped_glyph.data {
        return None;
    }
    Some(generate_y_variants(&flipped_glyph))
}

fn generate_y_variants(flipped_glyph: &crate::Glyph) -> Vec<crate::Glyph> {
    let mut cmp_glyhps = vec![flipped_glyph.clone()];

    let mut up_glyph = cmp_glyhps[0].clone();
    up_glyph.data.remove(0);
    up_glyph.data.push(*up_glyph.data.last().unwrap());

    let mut up_by2_glyph = up_glyph.clone();
    up_by2_glyph.data.remove(0);
    up_by2_glyph.data.push(*up_by2_glyph.data.last().unwrap());

    cmp_glyhps.push(up_glyph);
    cmp_glyhps.push(up_by2_glyph);

    let mut down_glyph = cmp_glyhps[0].clone();
    down_glyph.data.insert(0, down_glyph.data[0]);
    down_glyph.data.pop();

    let mut down_by2_glyph = cmp_glyhps[0].clone();
    down_by2_glyph.data.insert(0, down_by2_glyph.data[0]);
    down_by2_glyph.data.pop();

    cmp_glyhps.push(down_glyph);
    cmp_glyhps.push(down_by2_glyph);

    cmp_glyhps
}

pub fn map_char<S: ::std::hash::BuildHasher>(mut ch: AttributedChar, table: &HashMap<char, (bool, char), S>) -> AttributedChar {
    if let Some((flip, repl)) = table.get(&(ch.ch)) {
        ch.ch = *repl;
        if *flip {
            let tmp = ch.attribute.get_foreground();
            ch.attribute.set_foreground(ch.attribute.get_background());
            ch.attribute.set_background(tmp);
        }
    }
    ch
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        BitFont, Layer, Position, Rectangle, Size, TextPane,
        editor::{EditState, UndoState},
    };

    use super::{generate_flipx_table, generate_flipy_table};

    #[test]
    fn test_generate_flipx_table() {
        let table = generate_flipx_table(&BitFont::default());
        let cp437_table = HashMap::from([
            (40 as char, 41 as char),
            (41 as char, 40 as char),
            (47 as char, 92 as char),
            (92 as char, 47 as char),
            (60 as char, 62 as char),
            (62 as char, 60 as char),
            (91 as char, 93 as char),
            (93 as char, 91 as char),
            (123 as char, 125 as char),
            (125 as char, 123 as char),
            (169 as char, 170 as char),
            (170 as char, 169 as char),
            (174 as char, 175 as char),
            (175 as char, 174 as char),
            (180 as char, 195 as char),
            (195 as char, 180 as char),
            (181 as char, 198 as char),
            (198 as char, 181 as char),
            (182 as char, 199 as char),
            (199 as char, 182 as char),
            (183 as char, 214 as char),
            (214 as char, 183 as char),
            (185 as char, 204 as char),
            (204 as char, 185 as char),
            (187 as char, 201 as char),
            (201 as char, 187 as char),
            (188 as char, 200 as char),
            (200 as char, 188 as char),
            (189 as char, 211 as char),
            (211 as char, 189 as char),
            (190 as char, 212 as char),
            (212 as char, 190 as char),
            (191 as char, 218 as char),
            (218 as char, 191 as char),
            (192 as char, 217 as char),
            (217 as char, 192 as char),
            (221 as char, 222 as char),
            (222 as char, 221 as char),
            (242 as char, 243 as char),
            (243 as char, 242 as char),
            (27 as char, 26 as char),
            (26 as char, 27 as char),
            ('p', 'q'),
            ('q', 'p'),
            (186 as char, 199 as char),
            (199 as char, 186 as char),
            (17 as char, 16 as char),
            (16 as char, 17 as char),
            (213 as char, 184 as char),
            (184 as char, 213 as char),
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

        let rect = Rectangle::from(5, 5, 10, 10);
        state.set_selection(rect).unwrap();
        state.erase_selection().unwrap();
        for y in 0..20 {
            for x in 0..20 {
                let pos = Position::new(x, y);
                let ch = state.get_buffer().get_char(pos);

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
                let ch = state.get_buffer().get_char(pos);
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
                let ch = state.get_buffer().get_char((x, y));
                assert_eq!(ch.ch, '#');
            }
        }

        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().get_char((x, y));
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
                let ch = state.get_buffer().get_char((x, y));

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
                let ch = state.get_buffer().get_char((x, y));
                assert_eq!(ch.ch, '#');
            }
        }

        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().get_char((x, y));
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
                let ch = state.get_buffer().get_char((x, y));

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
                let ch = state.get_buffer().get_char((x, y));
                assert_eq!(ch.ch, '#');
            }
        }

        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().get_char((x, y));
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
                let ch = state.get_buffer().get_char((x, y));

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
                let ch = state.get_buffer().get_char((x, y));
                assert_eq!(ch.ch, '#');
            }
        }
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().get_char((x, y));
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
                let ch = state.get_buffer().get_char((x, y));

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
                let ch = state.get_buffer().get_char((x, y));
                assert_eq!(ch.ch, '#');
            }
        }
        for y in 0..10 {
            for x in 0..10 {
                let ch = state.get_buffer().get_char((x, y));
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
                let ch = state.get_buffer().get_char((x, y));

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

        assert_eq!(state.get_buffer().get_width(), 5);
        assert_eq!(state.get_buffer().get_height(), 4);
        assert_eq!(state.get_buffer().layers[1].get_size(), Size::new(5, 4));
        assert_eq!(state.get_buffer().layers[2].get_size(), Size::new(2, 2));

        state.undo().unwrap();

        assert_eq!(state.get_buffer().get_width(), 80);
        assert_eq!(state.get_buffer().get_height(), 25);
        assert_eq!(state.get_buffer().layers[1].get_size(), Size::new(100, 100));
        assert_eq!(state.get_buffer().layers[2].get_size(), Size::new(2, 2));
    }
}
