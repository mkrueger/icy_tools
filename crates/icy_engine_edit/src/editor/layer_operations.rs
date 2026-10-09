#![allow(clippy::missing_errors_doc)]
use std::collections::HashMap;

use i18n_embed_fl::fl;

use crate::{AttributedChar, Layer, LayerProperties, Position, Result, Role, Size, TextPane};

use super::{undo_operation::EditorUndoOp, EditState};

impl EditState {
    pub fn add_live_text(&mut self, source: icy_engine::LiveText, position: Position) -> Result<()> {
        let mut layer = source.render(self.get_buffer().buffer_type)?;
        layer.set_offset(position);
        let current = self.get_cur_layer().ok_or_else(|| crate::EngineError::Generic("Invalid layer index".into()))?;
        layer.parent_group = current.group.as_ref().map(|group| group.id).or(current.parent_group);
        let index = if current.is_group() {
            self.screen.current_layer
        } else {
            self.screen.current_layer + 1
        };
        self.push_undo_action(EditorUndoOp::AddLayer { index, layer: Box::new(layer) })?;
        self.screen.current_layer = index;
        Ok(())
    }

    pub fn update_live_text(&mut self, index: usize, source: icy_engine::LiveText) -> Result<()> {
        let Some(layer) = self.rendered_live_text(index, &source)? else {
            return Ok(());
        };
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description: fl!(crate::LANGUAGE_LOADER, "undo-live-text"),
        })
    }

    /// Changes the text of a live text layer while typing. With `continue_typing`, it extends the
    /// undo step of the previous change when that is still the last, unsaved one.
    pub fn type_live_text(&mut self, index: usize, source: icy_engine::LiveText, continue_typing: bool) -> Result<()> {
        let Some(layer) = self.rendered_live_text(index, &source)? else {
            return Ok(());
        };
        let description = fl!(crate::LANGUAGE_LOADER, "undo-type-live-text");
        let extend = continue_typing
            && {
                let stack = self.undo_stack.lock().unwrap();
                stack.is_modified()
                    && stack.redo_len() == 0
                    && matches!(stack.undo_stack().last(), Some(EditorUndoOp::ReplaceLayer { index: last, description: text, .. }) if *last == index && *text == description)
            };
        if extend {
            // The stored layer is the one before typing started, so undo still restores it.
            self.screen.buffer.layers[index] = layer;
            self.screen.buffer.mark_dirty();
            return Ok(());
        }
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description,
        })
    }

    /// The live text layer at `index` rendered from `source`, keeping its properties and
    /// effects; `None` when `source` is unchanged.
    fn rendered_live_text(&self, index: usize, source: &icy_engine::LiveText) -> Result<Option<Layer>> {
        let original = self
            .get_buffer()
            .layers
            .get(index)
            .ok_or_else(|| crate::EngineError::Generic("Invalid live text layer".into()))?;
        if !original.is_text() {
            return Err(crate::EngineError::Generic("This is not a live text layer".into()));
        }
        if original.live_text() == Some(source) {
            return Ok(None);
        }
        let mut layer = source.render(self.get_buffer().buffer_type)?;
        layer.properties = original.properties.clone();
        layer.properties.is_locked = true;
        layer.effects = original.effects.clone();
        layer.parent_group = original.parent_group;
        Ok(Some(layer))
    }

    pub fn bake_live_text(&mut self, index: usize) -> Result<()> {
        let mut layer = self
            .get_buffer()
            .layers
            .get(index)
            .ok_or_else(|| crate::EngineError::Generic("Invalid live text layer".into()))?
            .clone();
        if !layer.is_text() {
            return Ok(());
        }
        layer.set_live_text(None);
        layer.properties.is_locked = false;
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description: fl!(crate::LANGUAGE_LOADER, "undo-bake-live-text"),
        })
    }

    pub(crate) fn require_cell_layer(&self, index: usize) -> Result<()> {
        if self.get_buffer().layers.get(index).is_some_and(|layer| layer.is_group()) {
            return Err(crate::EngineError::Generic(fl!(crate::LANGUAGE_LOADER, "layer-group-select-child")));
        }
        if self.get_buffer().layers.get(index).is_some_and(|layer| layer.is_text()) {
            return Err(crate::EngineError::Generic(fl!(crate::LANGUAGE_LOADER, "live-text-bake-required")));
        }
        Ok(())
    }

    pub(crate) fn require_cell_layers(&self) -> Result<()> {
        for index in 0..self.get_buffer().layers.len() {
            if !self.get_buffer().layers[index].is_group() {
                self.require_cell_layer(index)?;
            }
        }
        Ok(())
    }

    pub fn set_layer_effects(&mut self, index: usize, effects: icy_engine::LayerEffects) -> Result<()> {
        effects.validate()?;
        let layer = self
            .get_buffer()
            .layers
            .get(index)
            .ok_or_else(|| crate::EngineError::Generic(format!("Invalid layer index: {index}")))?;
        if (layer.properties.is_locked && !layer.is_text()) || matches!(layer.role, Role::Image | Role::Group) {
            return Err(crate::EngineError::Generic("Layer effects require an unlocked text layer".into()));
        }
        if layer.effects == effects {
            return Ok(());
        }
        let mut changed = layer.clone();
        changed.effects = effects;
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(changed),
            description: fl!(crate::LANGUAGE_LOADER, "undo-layer-effects"),
        })
    }

    pub fn layer_mask_from_selection(&self, index: usize, hide_selected: bool) -> Result<icy_engine::LayerMask> {
        if !self.is_something_selected() {
            return Err(crate::EngineError::Generic("Select an area before creating a layer mask".into()));
        }
        let layer = self
            .get_buffer()
            .layers
            .get(index)
            .ok_or_else(|| crate::EngineError::Generic(format!("Invalid layer index: {index}")))?;
        let mut mask = icy_engine::LayerMask::new(layer.size())?;
        for y in 0..layer.height() {
            for x in 0..layer.width() {
                let local = Position::new(x, y);
                mask.set_hidden(local, self.is_selected(local + layer.offset()) == hide_selected);
            }
        }
        Ok(mask)
    }

    pub fn bake_layer_effects(&mut self, index: usize) -> Result<()> {
        let source = self
            .get_buffer()
            .layers
            .get(index)
            .ok_or_else(|| crate::EngineError::Generic(format!("Invalid layer index: {index}")))?;
        if source.effects.is_empty() {
            return Ok(());
        }
        if (source.properties.is_locked && !source.is_text()) || source.role == Role::Image {
            return Err(crate::EngineError::Generic("Baking effects requires an unlocked text layer".into()));
        }
        let layer = self.get_buffer().layer_with_baked_effects(index)?;
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description: fl!(crate::LANGUAGE_LOADER, "undo-bake-layer-effects"),
        })
    }

    pub fn add_new_layer(&mut self, layer: usize) -> Result<()> {
        let size = self.screen.buffer.size();
        let mut new_layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-new-name"), size);
        new_layer.properties.has_alpha_channel = true;
        let current = self
            .get_buffer()
            .layers
            .get(layer)
            .ok_or_else(|| crate::EngineError::Generic("Invalid layer index".into()))?;
        new_layer.parent_group = current.group.as_ref().map(|group| group.id).or(current.parent_group);
        let idx = if current.is_group() { layer } else { layer + 1 };
        let op = EditorUndoOp::AddLayer {
            index: idx,
            layer: Box::new(new_layer),
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
        if self.get_buffer().layers[layer].is_group() {
            let mut layers = self.get_buffer().layers.clone();
            layers.drain(self.get_buffer().layer_subtree(layer));
            let current = layer.saturating_sub(1).min(layers.len().saturating_sub(1));
            return self.replace_layer_stack(layers, current, fl!(crate::LANGUAGE_LOADER, "undo-remove_layer"));
        }
        let removed = self.screen.buffer.layers[layer].clone();
        let op = EditorUndoOp::RemoveLayer {
            layer_index: layer,
            layer: Box::new(removed),
        };
        self.push_undo_action(op)
    }

    pub fn raise_layer(&mut self, layer: usize) -> Result<()> {
        if self.get_buffer().layers.iter().any(Layer::is_group) {
            return self.reorder_grouped_layer(layer, true);
        }
        if layer + 1 >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }
        let op = EditorUndoOp::RaiseLayer { layer_index: layer };
        self.push_undo_action(op)?;
        self.screen.current_layer = layer + 1;
        Ok(())
    }

    pub fn lower_layer(&mut self, layer: usize) -> Result<()> {
        if self.get_buffer().layers.iter().any(Layer::is_group) {
            return self.reorder_grouped_layer(layer, false);
        }
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
        if self.get_buffer().layers[layer].is_group() {
            return self.duplicate_group(layer);
        }
        let mut new_layer = self.screen.buffer.layers[layer].clone();
        new_layer.properties.title = fl!(crate::LANGUAGE_LOADER, "layer-duplicate-name", name = new_layer.properties.title);
        let op = EditorUndoOp::AddLayer {
            index: layer + 1,
            layer: Box::new(new_layer),
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

        // Anchoring image layers is not supported.
        if matches!(role, Role::Image) {
            return Err(crate::EngineError::Generic("Cannot anchor image layer".to_string()));
        }
        if floating_idx > 0 {
            self.require_cell_layer(floating_idx - 1)?;
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
        println!("Merging layer {} down", layer);
        if layer == 0 {
            return Err(crate::EngineError::Generic("Cannot merge down base layer".to_string()));
        }
        println!("1");
        if layer >= self.screen.buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Invalid layer index: {layer}")));
        }
        println!("2");
        if self.get_buffer().layers[layer].is_group()
            || self.get_buffer().layers[layer - 1].is_group()
            || self.get_buffer().layers[layer].parent_group != self.get_buffer().layers[layer - 1].parent_group
        {
            return Err(crate::EngineError::Generic("Merge requires two layers in the same group".into()));
        }
        let role: Role = self.screen.buffer.layers[layer].role;
        if matches!(role, Role::Image) {
            return Err(crate::EngineError::Generic("Cannot merge down image layer".to_string()));
        }
        println!("3");

        let base_layer = self.screen.buffer.layer_with_baked_effects(layer - 1)?;
        let cur_layer = self.screen.buffer.layer_with_baked_effects(layer)?;

        let start: Position = Position::new(base_layer.offset().x.min(cur_layer.offset().x), base_layer.offset().y.min(cur_layer.offset().y));

        let width = (base_layer.offset().x + base_layer.width()).max(cur_layer.offset().x + cur_layer.width()) - start.x;
        let height = (base_layer.offset().y + base_layer.height()).max(cur_layer.offset().y + cur_layer.height()) - start.y;
        if width < 0 || height < 0 {
            return Err(crate::EngineError::Generic(format!("Invalid merged layer size: {width}x{height}")));
        }
        println!("4");
        let mut merge_layer = base_layer.clone();
        merge_layer.clear();
        merge_layer.set_offset(start);
        // Preallocate and use unchecked writes so merge works regardless of layer lock/visibility.
        merge_layer.preallocate_lines(width, height);

        for y in 0..base_layer.height() {
            for x in 0..base_layer.width() {
                let src = Position::new(x, y);
                let ch = base_layer.char_at(src);
                let dst = src + base_layer.offset() - start;
                if dst.x >= 0 && dst.y >= 0 && dst.x < width && dst.y < height {
                    merge_layer.set_char_unchecked(dst, ch);
                }
            }
        }

        for y in 0..cur_layer.height() {
            for x in 0..cur_layer.width() {
                let src = Position::new(x, y);
                let mut ch = cur_layer.char_at(src);
                if !ch.is_visible() {
                    continue;
                }

                let dst = src + cur_layer.offset() - start;
                if dst.x < 0 || dst.y < 0 || dst.x >= width || dst.y >= height {
                    continue;
                }

                let ch_below = merge_layer.char_at(dst);
                if ch_below.is_visible() && (ch.attribute.is_foreground_transparent() || ch.attribute.is_background_transparent()) {
                    ch = self.screen.buffer.make_solid_color(ch, ch_below);
                }

                merge_layer.set_char_unchecked(dst, ch);
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
        if self.get_buffer().layers.get(i).is_some_and(Layer::is_group) {
            self.set_layer_preview_offset(None);
            return self.move_group(i, to);
        }
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
        let index = self.screen.current_layer;
        if self.get_buffer().layers.get(index).is_some_and(Layer::is_group) {
            let range = self.get_buffer().layer_subtree(index);
            let delta = offset.map(|offset| offset - self.get_buffer().layers[index].base_offset());
            if offset.is_none() || self.can_move_layer(index) {
                for layer in &mut self.get_buffer_mut().layers[range] {
                    layer.set_preview_offset(delta.map(|delta| layer.base_offset() + delta));
                }
            }
            self.screen.buffer.mark_dirty();
            return;
        }
        if let Some(layer) = self.get_cur_layer_mut() {
            layer.set_preview_offset(offset);
        }
        // Mark buffer dirty to trigger re-rendering with new layer position
        self.screen.buffer.mark_dirty();
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
        let _undo: crate::AtomicUndoGuard = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-stamp-down"));
        let layer_idx = self.screen.current_layer;
        println!("Stamping layer {} down", layer_idx);
        if layer_idx == 0 {
            return Err(crate::EngineError::Generic("Cannot stamp down base layer".to_string()));
        }
        if self.get_buffer().layers[layer_idx].is_group()
            || self.get_buffer().layers[layer_idx].parent_group != self.get_buffer().layers[layer_idx - 1].parent_group
        {
            return Err(crate::EngineError::Generic("Stamp requires two layers in the same group".into()));
        }
        self.require_cell_layer(layer_idx - 1)?;

        let source = self.get_buffer().layer_with_baked_effects(layer_idx)?;
        self.bake_layer_effects(layer_idx - 1)?;
        let (src_offset, src_size) = {
            let src = self
                .screen
                .buffer
                .layers
                .get(layer_idx)
                .ok_or_else(|| crate::EngineError::Generic("Current layer is invalid".to_string()))?;
            (src.offset(), src.size())
        };
        let base_offset = self.screen.buffer.layers[layer_idx - 1].offset();
        let target_pos = src_offset - base_offset;

        println!("target_pos: {:?}", target_pos);
        // Capture old and new content in source-local coordinates (offset=0),
        // so undo/redo stamping is independent of document offsets.
        let width = src_size.width.max(0) as usize;
        let height = src_size.height.max(0) as usize;
        let mut old_chars = vec![vec![AttributedChar::invisible(); width]; height];
        let mut new_chars = vec![vec![AttributedChar::invisible(); width]; height];
        println!("Stamping at target pos {:?} with size {:?}", target_pos, src_size);
        for y in 0..src_size.height {
            for x in 0..src_size.width {
                let src_local = Position::new(x, y);
                old_chars[y as usize][x as usize] = self.screen.buffer.layers[layer_idx - 1].char_at(target_pos + src_local);
                new_chars[y as usize][x as usize] = if self.screen.buffer.layers[layer_idx].effects.is_masked(src_local) {
                    old_chars[y as usize][x as usize]
                } else {
                    source.char_at(src_local)
                };
                println!(
                    "  At {:?}: old={:?}, new={:?}",
                    src_local, old_chars[y as usize][x as usize].ch, new_chars[y as usize][x as usize].ch
                );
            }
        }

        let op: EditorUndoOp = EditorUndoOp::LayerChange {
            layer: layer_idx - 1,
            pos: target_pos,
            old_chars,
            new_chars,
        };
        self.push_undo_action(op)
    }

    /// Rotate the floating paste layer 90° clockwise.
    /// This is only used in paste mode and generates collaboration ROTATE command.
    pub fn paste_rotate(&mut self) -> Result<()> {
        self.require_cell_layer(self.get_current_layer()?)?;
        let current_layer = self.screen.current_layer;
        let font_dims = self.get_buffer().font_dimensions();
        if let Some(layer) = self.get_buffer_mut().layers.get_mut(current_layer) {
            let old_size = layer.size();
            let new_size = crate::Size::new(old_size.height, old_size.width);

            // Image/SIXEL paste: rotate pixel data and update sixel positions.
            if !layer.sixels.is_empty() {
                fn rotate_rgba_90_cw(pixels: &[u8], width: i32, height: i32) -> Option<(i32, i32, Vec<u8>)> {
                    if width <= 0 || height <= 0 {
                        return None;
                    }
                    let w = width as usize;
                    let h = height as usize;
                    let expected_len = w.checked_mul(h)?.checked_mul(4)?;
                    if pixels.len() != expected_len {
                        return None;
                    }

                    let new_w = height;
                    let new_h = width;
                    let new_w_usize = new_w as usize;
                    let mut out = vec![0u8; expected_len];

                    for y in 0..h {
                        for x in 0..w {
                            let src_idx = (y * w + x) * 4;
                            let dst_x = (h - 1 - y) as i32;
                            let dst_y = x as i32;
                            let dst_idx = ((dst_y as usize) * new_w_usize + (dst_x as usize)) * 4;
                            out[dst_idx..dst_idx + 4].copy_from_slice(&pixels[src_idx..src_idx + 4]);
                        }
                    }
                    Some((new_w, new_h, out))
                }

                let old_sixels: Vec<_> = layer.sixels.iter().map(Into::into).collect();
                let new_sixels: Vec<_> = layer
                    .sixels
                    .iter()
                    .map(|s| {
                        let rect_old = s.as_rectangle(font_dims);
                        let mut rotated = s.clone();

                        // Rotate pixels + swap dimensions.
                        if let Some((new_w, new_h, rotated_pixels)) = rotate_rgba_90_cw(&rotated.picture_data, rotated.width(), rotated.height()) {
                            rotated.picture_data = rotated_pixels;
                            rotated.set_size(crate::Size::new(new_w, new_h));
                        } else {
                            // Fallback: still swap the pixel size even if data is inconsistent.
                            let size = rotated.size();
                            rotated.set_size(crate::Size::new(size.height, size.width));
                        }

                        // Swap scaling axes.
                        std::mem::swap(&mut rotated.vertical_scale, &mut rotated.horizontal_scale);

                        // Reposition within the rotated layer (cell-space rotation).
                        rotated.position = crate::Position::new(old_size.height - s.position.y - rect_old.size.height, s.position.x);

                        (&rotated).into()
                    })
                    .collect();

                let op = EditorUndoOp::PasteRotateImage {
                    layer: current_layer,
                    old_sixels,
                    new_sixels,
                    old_size,
                    new_size,
                };
                return self.push_undo_action(op);
            }

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
        self.require_cell_layer(self.get_current_layer()?)?;
        let current_layer = self.screen.current_layer;
        let mut flip_tables = std::collections::HashMap::new();
        self.screen.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, crate::generate_flipx_table(font));
        });

        if let Some(layer) = self.get_buffer_mut().layers.get_mut(current_layer) {
            let old_lines = layer.lines.clone();
            let size: Size = layer.size();
            let area = crate::Rectangle::from_min_size(Position::default(), size);
            crate::flip_layer_x(layer, area, &flip_tables);

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
        self.require_cell_layer(self.get_current_layer()?)?;
        let current_layer = self.screen.current_layer;
        let mut flip_tables = std::collections::HashMap::new();
        self.screen.buffer.font_iter().for_each(|(page, font)| {
            flip_tables.insert(*page, crate::generate_flipy_table(font));
        });

        if let Some(layer) = self.get_buffer_mut().layers.get_mut(current_layer) {
            let old_lines = layer.lines.clone();
            let size = layer.size();
            let area = crate::Rectangle::from_min_size(Position::default(), size);
            crate::flip_layer_y(layer, area, &flip_tables);

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
        self.require_cell_layer(self.get_current_layer()?)?;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-make_transparent"));
        let layer_idx = self.screen.current_layer;
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = crate::Rectangle {
                start: Position::new(0, 0),
                size: layer.size(),
            };
            let old_layer = crate::chars_from_area(layer, area);

            for x in 0..layer.width() as u32 {
                for y in 0..layer.height() as u32 {
                    let pos = Position::new(x as i32, y as i32);
                    let ch = layer.char_at(pos);
                    if ch.is_transparent() {
                        layer.set_char(pos, crate::AttributedChar::invisible());
                    }
                }
            }
            let new_layer = crate::chars_from_area(layer, area);
            let op = EditorUndoOp::LayerChange {
                layer: layer_idx,
                pos: area.start,
                old_chars: old_layer,
                new_chars: new_layer,
            };
            // The characters were changed in place, so the renderer has to be told.
            self.screen.buffer.mark_dirty();
            self.push_plain_undo(op)
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    /// Replaces the characters of the current layer, starting at its top left corner (with undo).
    pub fn replace_layer_chars(&mut self, chars: crate::CharGrid) -> Result<()> {
        let layer_idx = self.screen.current_layer;
        let Some(layer) = self.get_cur_layer() else {
            return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
        };
        let size = crate::Size::new(chars.first().map_or(0, Vec::len) as i32, chars.len() as i32);
        let area = crate::Rectangle {
            start: Position::new(0, 0),
            size,
        };
        let op = EditorUndoOp::LayerChange {
            layer: layer_idx,
            pos: area.start,
            old_chars: crate::chars_from_area(layer, area),
            new_chars: chars,
        };
        self.push_undo_action(op)
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
    pub fn update_layer_properties(&mut self, layer: usize, new_properties: LayerProperties) -> Result<()> {
        if self.get_buffer().layers.get(layer).is_some_and(Layer::is_group) {
            let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-move-layer-group"));
            if new_properties.offset != self.get_buffer().layers[layer].base_offset() {
                self.move_group(layer, new_properties.offset)?;
            }
            let mut changed = self.get_buffer().layers[layer].clone();
            changed.properties = new_properties;
            changed.properties.is_locked = true;
            return self.push_undo_action(EditorUndoOp::ReplaceLayer {
                index: layer,
                layer: Box::new(changed),
                description: fl!(crate::LANGUAGE_LOADER, "undo-move-layer-group"),
            });
        }
        if !new_properties.is_locked {
            self.require_cell_layer(layer)?;
        }
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
