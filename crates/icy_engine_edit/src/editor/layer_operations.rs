#![allow(clippy::missing_errors_doc)]
use std::collections::HashMap;

use i18n_embed_fl::fl;

use crate::{AttributedChar, Layer, Position, Properties, Result, Role, Size, TextPane};

use super::{EditState, undo_operation::EditorUndoOp};

impl EditState {
    pub fn add_new_layer(&mut self, layer: usize) -> Result<()> {
        let size = self.screen.buffer.size();
        let mut new_layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-new-name"), size);
        new_layer.properties.has_alpha_channel = true;
        let idx = (layer + 1).clamp(0, self.screen.buffer.layers.len());
        let op = EditorUndoOp::AddLayer {
            index: idx,
            layer: Some(new_layer),
        };
        self.push_undo_action(op)?;
        self.screen.current_layer = idx;
        Ok(())
    }
    //
    pub fn remove_layer(&mut self, layer: usize) -> Result<()> {
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }
        let op = EditorUndoOp::RemoveLayer {
            layer_index: layer,
            layer: None,
        };
        self.push_undo_action(op)
    }

    pub fn raise_layer(&mut self, layer: usize) -> Result<()> {
        if layer + 1 >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }
        let op = EditorUndoOp::RaiseLayer { layer_index: layer };
        self.push_undo_action(op)?;
        self.screen.current_layer = layer + 1;
        Ok(())
    }

    pub fn lower_layer(&mut self, layer: usize) -> Result<()> {
        if layer == 0 {
            return Ok(());
        }
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }

        let op = EditorUndoOp::LowerLayer { layer_index: layer };
        self.push_undo_action(op)?;
        self.screen.current_layer = layer - 1;
        Ok(())
    }

    pub fn duplicate_layer(&mut self, layer: usize) -> Result<()> {
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }
        let mut new_layer = self.screen.buffer.layers[layer].clone();
        new_layer.properties.title = fl!(crate::LANGUAGE_LOADER, "layer-duplicate-name", name = new_layer.properties.title);
        let op = EditorUndoOp::AddLayer {
            index: layer + 1,
            layer: Some(new_layer),
        };
        self.push_undo_action(op)?;
        self.screen.current_layer = layer + 1;
        Ok(())
    }

    pub fn clear_layer(&mut self, layer: usize) -> Result<()> {
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }
        let op = EditorUndoOp::ClearLayer {
            layer_index: layer,
            layer: Vec::new(),
        };
        self.push_undo_action(op)?;
        // Keep the cleared layer selected.
        self.screen.current_layer = layer;
        self.clamp_current_layer();
        Ok(())
    }

    /// Returns the anchor layer of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn anchor_layer(&mut self) -> Result<()> {
        // Find the floating layer by role (it's at current_layer + 1)
        let floating_idx = self.get_current_layer();

        let Ok(floating_idx) = floating_idx else {
            // No floating layer - nothing to anchor
            return Ok(());
        };

        let role = self.screen.buffer.layers[floating_idx].role;

        // PasteImage layers are handled differently (just convert role)
        if matches!(role, Role::Image) {
            // Just convert the role to Image
            self.screen.buffer.layers[floating_idx].role = Role::Image;
            // Keep current_layer pointing to this layer (now a normal Image layer)
            return Ok(());
        }

        // PastePreview layers are merged down
        let _op = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "layer-anchor"));
        let result = self.merge_layer_down(floating_idx);

        // After merge, set current_layer to the layer that was below the floating layer
        if floating_idx > 0 {
            self.screen.current_layer = floating_idx - 1;
        } else {
            self.screen.current_layer = 0;
        }

        result
    }

    pub fn add_floating_layer(&mut self) -> Result<()> {
        let op = EditorUndoOp::AddFloatingLayer {
            current_layer: self.get_current_layer()?,
        };
        self.push_undo_action(op)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn merge_layer_down(&mut self, layer: usize) -> Result<()> {
        if layer == 0 {
            return Err(crate::EngineError::Generic("Cannot merge down base layer".to_string()));
        }
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }
        let Some(cur_layer) = self.get_cur_layer() else {
            return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
        };
        let role = cur_layer.role;
        if matches!(role, Role::Image) {
            return Ok(());
        }

        let base_layer = &self.screen.buffer.layers[layer - 1];
        let cur_layer = &self.screen.buffer.layers[layer];

        let start: Position = Position::new(base_layer.offset().x.min(cur_layer.offset().x), base_layer.offset().y.min(cur_layer.offset().y));

        let mut merge_layer = base_layer.clone();
        merge_layer.clear();

        merge_layer.set_offset(start);

        let width = (base_layer.offset().x + base_layer.width()).max(cur_layer.offset().x + cur_layer.width()) - start.x;
        let height = (base_layer.offset().y + base_layer.height()).max(cur_layer.offset().y + cur_layer.height()) - start.y;
        if width < 0 || height < 0 {
            return Ok(());
        }
        merge_layer.set_size((width, height));

        for y in 0..base_layer.height() {
            for x in 0..base_layer.width() {
                let pos = Position::new(x, y);
                let ch = base_layer.char_at(pos);
                let pos = pos - merge_layer.offset() + base_layer.offset();
                merge_layer.set_char(pos, ch);
            }
        }

        for y in 0..cur_layer.height() {
            for x in 0..cur_layer.width() {
                let pos = Position::new(x, y);
                let mut ch = cur_layer.char_at(pos);
                if !ch.is_visible() {
                    continue;
                }

                let pos = pos - merge_layer.offset() + cur_layer.offset();

                let ch_below = merge_layer.char_at(pos);
                if ch_below.is_visible() && (ch.attribute.is_foreground_transparent() || ch.attribute.is_background_transparent()) {
                    ch = self.screen.buffer.make_solid_color(ch, ch_below);
                }

                merge_layer.set_char(pos, ch);
            }
        }

        let op = EditorUndoOp::MergeLayerDown {
            index: layer,
            merged_layer: Some(merge_layer),
            orig_layers: None,
        };
        self.push_undo_action(op)?;
        self.clamp_current_layer();
        Ok(())
    }

    pub fn toggle_layer_visibility(&mut self, layer: usize) -> Result<()> {
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic("Invalid layer index: {layer}".to_string()));
        }
        let op = EditorUndoOp::ToggleLayerVisibility { index: layer };
        self.push_undo_action(op)
    }

    pub fn move_layer(&mut self, to: Position) -> Result<()> {
        let i = self.screen.current_layer;
        let Some(cur_layer) = self.get_cur_layer_mut() else {
            return Ok(());
        };
        cur_layer.set_preview_offset(None);
        let op = EditorUndoOp::MoveLayer {
            index: i,
            from: cur_layer.offset(),
            to,
        };
        self.push_undo_action(op)
    }

    /// Set preview offset on the current layer (for drag preview without undo)
    pub fn set_layer_preview_offset(&mut self, offset: Option<Position>) {
        if let Some(layer) = self.get_cur_layer_mut() {
            layer.set_preview_offset(offset);
        }
    }

    pub fn set_layer_size(&mut self, layer: usize, size: impl Into<Size>) -> Result<()> {
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic("Invalid layer index: {layer}".to_string()));
        }
        let new_size = size.into();
        let op = EditorUndoOp::SetLayerSize {
            index: layer,
            from: new_size,
            to: new_size,
        };
        self.push_undo_action(op)
    }

    /// Returns the stamp layer down of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn stamp_layer_down(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-stamp-down"));
        let layer_idx = self.screen.current_layer;
        let layer = if let Some(layer) = self.get_cur_layer() {
            layer.clone()
        } else {
            return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
        };

        let base_layer = &mut self.screen.buffer.layers[layer_idx - 1];
        let area = layer.rectangle() + base_layer.offset();
        let old_layer = crate::layer_from_area(base_layer, area);

        for x in 0..layer.width() as u32 {
            for y in 0..layer.height() as u32 {
                let pos = Position::new(x as i32, y as i32);
                let ch = layer.char_at(pos);
                if !ch.is_visible() {
                    continue;
                }

                let dest = pos + area.top_left();
                base_layer.set_char(dest, ch);
            }
        }

        let new_layer = crate::layer_from_area(base_layer, area);
        let op = EditorUndoOp::LayerChange {
            layer: layer_idx - 1,
            pos: area.start,
            old_chars: old_layer,
            new_chars: new_layer,
        };
        self.push_plain_undo(op)
    }

    /// Rotate the floating paste layer 90° clockwise.
    /// This is only used in paste mode and generates collaboration ROTATE command.
    pub fn paste_rotate(&mut self) -> Result<()> {
        let current_layer = self.screen.current_layer;
        if let Some(layer) = self.get_buffer_mut().layers.get_mut(current_layer) {
            let old_size = layer.size();
            let new_size = crate::Size::new(old_size.height, old_size.width);
            let mut new_layer = Layer::new("", new_size);
            for y in 0..old_size.width {
                for x in 0..old_size.height {
                    let ch = layer.char_at((y, old_size.height - 1 - x).into());
                    let ch = map_char_u8(ch, &ROTATE_TABLE);
                    new_layer.set_char((x, y), ch);
                }
            }
            let op = EditorUndoOp::PasteRotate {
                layer: current_layer,
                old_lines: layer.lines.clone(),
                new_lines: new_layer.lines.clone(),
                old_size,
                new_size,
            };
            self.push_undo_action(op)
        } else {
            Err(crate::EngineError::Generic(format!("Invalid layer: {}", current_layer)))
        }
    }

    /// Flip the floating paste layer horizontally.
    /// This is only used in paste mode and generates collaboration FLIP_X command.
    pub fn paste_flip_x(&mut self) -> Result<()> {
        let current_layer = self.screen.current_layer;
        let mut flip_tables = std::collections::HashMap::new();
        self.screen.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, crate::generate_flipx_table(font));
        });

        if let Some(layer) = self.get_buffer_mut().layers.get_mut(current_layer) {
            let old_lines = layer.lines.clone();
            let size = layer.size();
            let max = size.width / 2;

            for y in 0..size.height {
                for x in 0..max {
                    let pos1 = icy_engine::Position::new(x, y);
                    let pos2 = icy_engine::Position::new(size.width - x - 1, y);

                    let pos1ch = layer.char_at(pos1);
                    let pos1ch = crate::map_char(pos1ch, flip_tables.get(&pos1ch.font_page()).unwrap());
                    let pos2ch = layer.char_at(pos2);
                    let pos2ch = crate::map_char(pos2ch, flip_tables.get(&pos2ch.font_page()).unwrap());
                    layer.set_char(pos1, pos2ch);
                    layer.set_char(pos2, pos1ch);
                }
            }

            let op = EditorUndoOp::PasteFlipX {
                layer: current_layer,
                old_lines,
                new_lines: layer.lines.clone(),
            };
            self.push_undo_action(op)
        } else {
            Err(crate::EngineError::Generic(format!("Invalid layer: {}", current_layer)))
        }
    }

    /// Flip the floating paste layer vertically.
    /// This is only used in paste mode and generates collaboration FLIP_Y command.
    pub fn paste_flip_y(&mut self) -> Result<()> {
        let current_layer = self.screen.current_layer;
        let mut flip_tables = std::collections::HashMap::new();
        self.screen.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, crate::generate_flipy_table(font));
        });

        if let Some(layer) = self.get_buffer_mut().layers.get_mut(current_layer) {
            let old_lines = layer.lines.clone();
            let size = layer.size();
            let max = size.height / 2;

            for x in 0..size.width {
                for y in 0..max {
                    let pos1 = icy_engine::Position::new(x, y);
                    let pos2 = icy_engine::Position::new(x, size.height - 1 - y);
                    let pos1ch = layer.char_at(pos1);
                    let pos1ch = crate::map_char(pos1ch, flip_tables.get(&pos1ch.font_page()).unwrap());
                    let pos2ch = layer.char_at(pos2);
                    let pos2ch = crate::map_char(pos2ch, flip_tables.get(&pos2ch.font_page()).unwrap());
                    layer.set_char(pos1, pos2ch);
                    layer.set_char(pos2, pos1ch);
                }
            }

            let op = EditorUndoOp::PasteFlipY {
                layer: current_layer,
                old_lines,
                new_lines: layer.lines.clone(),
            };
            self.push_undo_action(op)
        } else {
            Err(crate::EngineError::Generic(format!("Invalid layer: {}", current_layer)))
        }
    }

    /// Anchor the floating paste layer and generate collaboration DRAW commands.
    /// Returns the anchor data for collaboration sync.
    pub fn paste_anchor(&mut self) -> Result<()> {
        // Collect floating layer data BEFORE anchor for collaboration
        let collab_data: Option<(i32, i32, crate::collaboration::Blocks)> = self
            .get_floating_layer_blocks()
            .and_then(|blocks| self.get_floating_layer_position().map(|(x, y)| (x, y, blocks)));

        // Do the actual anchor
        self.anchor_layer()?;

        // Push PasteAnchor for collaboration sync
        if let Some((x, y, blocks)) = collab_data {
            let op = EditorUndoOp::PasteAnchor { x, y, blocks };
            self.push_plain_undo(op)?;
        }

        Ok(())
    }

    /// Returns the make layer transparent of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn make_layer_transparent(&mut self) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-make_transparent"));
        let layer_idx = self.screen.current_layer;
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = crate::Rectangle {
                start: Position::new(0, 0),
                size: layer.size(),
            };
            let old_layer = crate::layer_from_area(layer, area);

            for x in 0..layer.width() as u32 {
                for y in 0..layer.height() as u32 {
                    let pos = Position::new(x as i32, y as i32);
                    let ch = layer.char_at(pos);
                    if ch.is_transparent() {
                        layer.set_char(pos, crate::AttributedChar::invisible());
                    }
                }
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: layer_idx,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    /// Returns the make layer transparent of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn update_layer_properties(&mut self, layer: usize, new_properties: Properties) -> Result<()> {
        let op = EditorUndoOp::UpdateLayerProperties {
            index: layer,
            old_properties: self.screen.buffer.layers[layer].properties.clone(),
            new_properties,
        };
        self.push_undo_action(op)
    }
}

pub fn map_char_u8<S: ::std::hash::BuildHasher>(mut ch: AttributedChar, table: &HashMap<u8, u8, S>) -> AttributedChar {
    if let Some(repl) = table.get(&(ch.ch as u8)) {
        ch.ch = *repl as char;
    }
    ch
}

lazy_static::lazy_static! {
    static ref ROTATE_TABLE: HashMap<u8, u8> = HashMap::from([
        // block
        (220, 221),
        (221, 223),
        (223, 222),
        (222, 220),

        // single line
        (179, 196),
        (196, 179),

        // single line corner
        (191, 217),
        (217, 192),
        (192, 218),
        (218, 191),

        // single side
        (180, 193),
        (193, 195),
        (195, 194),
        (194, 180),

        // double line
        (186, 205),
        (205, 186),

        // double line corner
        (187, 188),
        (188, 200),
        (200, 201),
        (201, 187),

        // double line side
        (185, 202),
        (202, 204),
        (204, 203),
        (203, 185),

        // double line to single line corner
        (184, 189),
        (189, 212),
        (212, 214),
        (214, 184),

         // double line to single line side
         (181, 208),
         (208, 198),
         (198, 210),
         (210, 181),

        // double line to single line corner
        (183, 190),
        (190, 211),
        (211, 213),
        (213, 183),

        // single line to double line side
        (182, 207),
        (207, 199),
        (199, 209),
        (209, 182),

         // single line to double line corner
         (183, 190),
         (190, 211),
         (211, 213),
         (213, 183),


        // single line to double crossing
        (215, 216),
        (216, 215),

    ]);
}
