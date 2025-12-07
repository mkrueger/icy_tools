#![allow(clippy::missing_errors_doc)]
use std::collections::HashMap;

use i18n_embed_fl::fl;

use crate::{AttributedChar, EngineResult, Layer, Position, Properties, Role, Size, TextAttribute, TextPane};

use super::{EditState, undo_operations};

impl EditState {
    pub fn add_new_layer(&mut self, layer: usize) -> EngineResult<()> {
        let size = self.buffer.get_size();
        let mut new_layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-new-name"), size);
        new_layer.properties.has_alpha_channel = true;
        let idx = (layer + 1).clamp(0, self.buffer.layers.len());
        let op = undo_operations::AddLayer::new(idx, new_layer);
        self.push_undo_action(Box::new(op))?;
        self.current_layer = idx;
        Ok(())
    }
    //
    pub fn remove_layer(&mut self, layer: usize) -> EngineResult<()> {
        if layer >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }
        let op = undo_operations::RemoveLayer::new(layer);
        self.push_undo_action(Box::new(op))
    }

    pub fn raise_layer(&mut self, layer: usize) -> EngineResult<()> {
        if layer + 1 >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }
        let op = undo_operations::RaiseLayer::new(layer);
        self.push_undo_action(Box::new(op))?;
        self.current_layer = layer + 1;
        Ok(())
    }

    pub fn lower_layer(&mut self, layer: usize) -> EngineResult<()> {
        if layer == 0 {
            return Ok(());
        }
        if layer >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }

        let op = undo_operations::LowerLayer::new(layer);
        self.push_undo_action(Box::new(op))?;
        self.current_layer = layer - 1;
        Ok(())
    }

    pub fn duplicate_layer(&mut self, layer: usize) -> EngineResult<()> {
        if layer >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }
        let mut new_layer = self.buffer.layers[layer].clone();
        new_layer.properties.title = fl!(crate::LANGUAGE_LOADER, "layer-duplicate-name", name = new_layer.properties.title);
        let op = undo_operations::AddLayer::new(layer + 1, new_layer);
        self.push_undo_action(Box::new(op))?;
        self.current_layer = layer + 1;
        Ok(())
    }

    pub fn clear_layer(&mut self, layer: usize) -> EngineResult<()> {
        if layer >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }
        let op = undo_operations::ClearLayer::new(layer);
        self.push_undo_action(Box::new(op))?;
        self.current_layer = layer + 1;
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
    pub fn anchor_layer(&mut self) -> EngineResult<()> {
        let Some(cur_layer) = self.get_cur_layer() else {
            return Err(super::EditorError::CurrentLayerInvalid.into());
        };
        let role = cur_layer.role;
        if !matches!(role, Role::PastePreview) {
            return Ok(());
        }
        let _op = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "layer-anchor"));
        self.merge_layer_down(self.get_current_layer()?)
    }

    pub fn add_floating_layer(&mut self) -> EngineResult<()> {
        let op = undo_operations::AddFloatingLayer::new(self.get_current_layer()?);
        self.push_undo_action(Box::new(op))
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
    pub fn merge_layer_down(&mut self, layer: usize) -> EngineResult<()> {
        if layer == 0 {
            return Err(anyhow::anyhow!("Cannot merge down base layer"));
        }
        if layer >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }
        let Some(cur_layer) = self.get_cur_layer() else {
            return Err(super::EditorError::CurrentLayerInvalid.into());
        };
        let role = cur_layer.role;
        if matches!(role, Role::PasteImage) {
            return Ok(());
        }

        let base_layer = &self.buffer.layers[layer - 1];
        let cur_layer = &self.buffer.layers[layer];

        let start = Position::new(
            base_layer.get_offset().x.min(cur_layer.get_offset().x),
            base_layer.get_offset().y.min(cur_layer.get_offset().y),
        );

        let mut merge_layer = base_layer.clone();
        merge_layer.clear();

        merge_layer.set_offset(start);

        let width = (base_layer.get_offset().x + base_layer.get_width()).max(cur_layer.get_offset().x + cur_layer.get_width()) - start.x;
        let height = (base_layer.get_offset().y + base_layer.get_height()).max(cur_layer.get_offset().y + cur_layer.get_height()) - start.y;
        if width < 0 || height < 0 {
            return Ok(());
        }
        merge_layer.set_size((width, height));

        for y in 0..base_layer.get_height() {
            for x in 0..base_layer.get_width() {
                let pos = Position::new(x, y);
                let ch = base_layer.get_char(pos);
                let pos = pos - merge_layer.get_offset() + base_layer.get_offset();
                merge_layer.set_char(pos, ch);
            }
        }

        for y in 0..cur_layer.get_height() {
            for x in 0..cur_layer.get_width() {
                let pos = Position::new(x, y);
                let mut ch = cur_layer.get_char(pos);
                if !ch.is_visible() {
                    continue;
                }

                let pos = pos - merge_layer.get_offset() + cur_layer.get_offset();

                let ch_below = merge_layer.get_char(pos);
                if ch_below.is_visible()
                    && (ch.attribute.get_foreground() == TextAttribute::TRANSPARENT_COLOR || ch.attribute.get_background() == TextAttribute::TRANSPARENT_COLOR)
                {
                    ch = self.buffer.make_solid_color(ch, ch_below);
                }

                merge_layer.set_char(pos, ch);
            }
        }

        let op = undo_operations::MergeLayerDown::new(layer, merge_layer);
        self.push_undo_action(Box::new(op))?;
        self.clamp_current_layer();
        Ok(())
    }

    pub fn toggle_layer_visibility(&mut self, layer: usize) -> EngineResult<()> {
        if layer >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }
        let op = undo_operations::ToggleLayerVisibility::new(layer);
        self.push_undo_action(Box::new(op))
    }

    pub fn move_layer(&mut self, to: Position) -> EngineResult<()> {
        let i = self.current_layer;
        let Some(cur_layer) = self.get_cur_layer_mut() else {
            return Ok(());
        };
        cur_layer.set_preview_offset(None);
        let op = undo_operations::MoveLayer::new(i, cur_layer.get_offset(), to);
        self.push_undo_action(Box::new(op))
    }

    pub fn set_layer_size(&mut self, layer: usize, size: impl Into<Size>) -> EngineResult<()> {
        if layer >= self.buffer.layers.len() {
            return Err(anyhow::anyhow!("Invalid layer index: {layer}"));
        }
        let op = undo_operations::SetLayerSize::new(layer, size.into());
        self.push_undo_action(Box::new(op))
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
    pub fn stamp_layer_down(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-stamp-down"));
        let layer_idx = self.current_layer;
        let layer = if let Some(layer) = self.get_cur_layer() {
            layer.clone()
        } else {
            return Err(super::EditorError::CurrentLayerInvalid.into());
        };

        let base_layer = &mut self.buffer.layers[layer_idx - 1];
        let area = layer.get_rectangle() + base_layer.get_offset();
        let old_layer = crate::layer_from_area(base_layer, area);

        for x in 0..layer.get_width() as u32 {
            for y in 0..layer.get_height() as u32 {
                let pos = Position::new(x as i32, y as i32);
                let ch = layer.get_char(pos);
                if !ch.is_visible() {
                    continue;
                }

                let dest = pos + area.top_left();
                base_layer.set_char(dest, ch);
            }
        }

        let new_layer = crate::layer_from_area(base_layer, area);
        let op = super::undo_operations::UndoLayerChange::new(layer_idx - 1, area.start, old_layer, new_layer);
        self.push_plain_undo(Box::new(op))
    }

    pub fn rotate_layer(&mut self) -> EngineResult<()> {
        let current_layer = self.current_layer;
        if let Some(layer) = self.get_buffer_mut().layers.get_mut(current_layer) {
            let size = layer.get_size();
            let mut new_layer = Layer::new("", (size.height, size.width));
            for y in 0..size.width {
                for x in 0..size.height {
                    let ch = layer.get_char((y, size.height - 1 - x).into());
                    let ch = map_char_u8(ch, &ROTATE_TABLE);
                    new_layer.set_char((x, y), ch);
                }
            }
            let op = super::undo_operations::RotateLayer::new(current_layer, layer.lines.clone(), new_layer.lines.clone());
            self.push_undo_action(Box::new(op))
        } else {
            Err(super::EditorError::InvalidLayer(current_layer).into())
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
    pub fn make_layer_transparent(&mut self) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-make_transparent"));
        let layer_idx = self.current_layer;
        if let Some(layer) = self.get_cur_layer_mut() {
            let area = crate::Rectangle {
                start: Position::new(0, 0),
                size: layer.get_size(),
            };
            let old_layer = crate::layer_from_area(layer, area);

            for x in 0..layer.get_width() as u32 {
                for y in 0..layer.get_height() as u32 {
                    let pos = Position::new(x as i32, y as i32);
                    let ch = layer.get_char(pos);
                    if ch.is_transparent() {
                        layer.set_char(pos, crate::AttributedChar::invisible());
                    }
                }
            }
            let new_layer = crate::layer_from_area(layer, area);
            let op = super::undo_operations::UndoLayerChange::new(layer_idx, area.start, old_layer, new_layer);
            self.push_plain_undo(Box::new(op))
        } else {
            Err(super::EditorError::CurrentLayerInvalid.into())
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
    pub fn update_layer_properties(&mut self, layer: usize, new_properties: Properties) -> EngineResult<()> {
        let op = undo_operations::UpdateLayerProperties::new(layer, self.buffer.layers[layer].properties.clone(), new_properties);
        self.push_undo_action(Box::new(op))
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

#[cfg(test)]
mod tests {
    use crate::{
        AttributedChar, Layer, Position, Size, TextAttribute, TextPane,
        editor::{EditState, UndoState},
    };

    #[test]
    fn test_add_layer() {
        let mut state = EditState::default();
        assert_eq!(1, state.buffer.layers.len());
        state.add_new_layer(0).unwrap();
        assert_eq!(2, state.buffer.layers.len());
    }

    #[test]
    fn test_add_layer_transparency_behavior() {
        // test an issue where new layer was created w/o alpha channel when there was no base layer
        let mut state = EditState::default();
        state.buffer.layers.clear();
        state.add_new_layer(0).unwrap();
        assert!(state.buffer.layers[0].properties.has_alpha_channel);
    }

    #[test]
    fn test_add_layer_size() {
        let mut state = EditState::default();
        let size = Size::new(160, 1000);
        state.buffer.set_size(size);
        state.add_new_layer(0).unwrap();
        assert_eq!(size, state.buffer.layers[1].get_size());
    }

    #[test]
    fn test_add_layer_undo_redo() {
        let mut state = EditState::default();
        assert_eq!(1, state.buffer.layers.len());
        state.add_new_layer(0).unwrap();
        assert_eq!(2, state.buffer.layers.len());
        state.undo().unwrap();
        assert_eq!(1, state.buffer.layers.len());
        state.redo().unwrap();
        assert_eq!(2, state.buffer.layers.len());
    }

    #[test]
    fn test_remove_layer() {
        let mut state = EditState::default();
        state.add_new_layer(0).unwrap();
        state.add_new_layer(0).unwrap();
        assert_eq!(3, state.buffer.layers.len());
        state.remove_layer(1).unwrap();
        assert_eq!(2, state.buffer.layers.len());
    }

    #[test]
    fn test_remove_layer_undo_redo() {
        let mut state = EditState::default();
        state.add_new_layer(0).unwrap();
        state.add_new_layer(0).unwrap();
        assert_eq!(3, state.buffer.layers.len());
        state.remove_layer(1).unwrap();
        assert_eq!(2, state.buffer.layers.len());
        state.undo().unwrap();
        assert_eq!(3, state.buffer.layers.len());
        state.redo().unwrap();
        assert_eq!(2, state.buffer.layers.len());
    }

    #[test]
    fn test_raise_layer() {
        let mut state = EditState::default();
        let name = state.buffer.layers[0].properties.title.clone();
        state.add_new_layer(0).unwrap();
        state.raise_layer(0).unwrap();
        assert_eq!(name, state.buffer.layers[1].properties.title);
        state.undo().unwrap();
        assert_ne!(name, state.buffer.layers[1].properties.title);
    }

    #[test]
    fn test_lower_layer() {
        let mut state: EditState = EditState::default();
        state.add_new_layer(0).unwrap();
        let name = state.buffer.layers[1].properties.title.clone();
        state.lower_layer(1).unwrap();
        assert_eq!(name, state.buffer.layers[0].properties.title);
        state.undo().unwrap();
        assert_ne!(name, state.buffer.layers[0].properties.title);
    }

    #[test]
    fn test_toggle_layer_visibility() {
        let mut state = EditState::default();
        assert!(state.buffer.layers[0].properties.is_visible);
        state.toggle_layer_visibility(0).unwrap();
        assert!(!state.buffer.layers[0].properties.is_visible);
        state.undo().unwrap();
        assert!(state.buffer.layers[0].properties.is_visible);
    }

    #[test]
    fn test_merge_layer_down() {
        let mut state = EditState::default();
        let mut new_layer = Layer::new("1", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
        state.buffer.layers.push(new_layer);

        let mut new_layer = Layer::new("2", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_char((6, 6), AttributedChar::new('b', TextAttribute::default()));
        state.buffer.layers.push(new_layer);

        state.merge_layer_down(2).unwrap();
        assert_eq!(2, state.buffer.layers.len());

        assert_eq!('a', state.buffer.get_char((5, 5).into()).ch);
        assert_eq!('b', state.buffer.get_char((6, 6).into()).ch);
        assert_eq!(Position::new(0, 0), state.buffer.layers[1].get_offset());
        assert_eq!(Size::new(10, 10), state.buffer.layers[1].get_size());
        state.undo().unwrap();
        assert_eq!(3, state.buffer.layers.len());
    }

    #[test]
    fn test_merge_layer_down_case1() {
        let mut state = EditState::default();
        let mut new_layer = Layer::new("1", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_offset((2, 2));
        new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
        state.buffer.layers.push(new_layer);

        let mut new_layer = Layer::new("2", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_char((6, 6), AttributedChar::new('b', TextAttribute::default()));
        state.buffer.layers.push(new_layer);

        state.merge_layer_down(2).unwrap();
        assert_eq!(2, state.buffer.layers.len());

        assert_eq!('a', state.buffer.get_char((7, 7).into()).ch);
        assert_eq!('b', state.buffer.get_char((6, 6).into()).ch);
        assert_eq!(Position::new(0, 0), state.buffer.layers[1].get_offset());
        assert_eq!(Size::new(12, 12), state.buffer.layers[1].get_size());
    }

    #[test]
    fn test_merge_layer_down_case2() {
        let mut state = EditState::default();
        let mut new_layer = Layer::new("1", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_offset((-1, -1));
        new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
        state.buffer.layers.push(new_layer);

        let mut new_layer = Layer::new("2", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_char((6, 6), AttributedChar::new('b', TextAttribute::default()));
        state.buffer.layers.push(new_layer);

        state.merge_layer_down(2).unwrap();

        assert_eq!(2, state.buffer.layers.len());

        assert_eq!(Position::new(-1, -1), state.buffer.layers[1].get_offset());
        assert_eq!(Size::new(11, 11), state.buffer.layers[1].get_size());

        assert_eq!('a', state.buffer.layers[1].get_char((5, 5).into()).ch);
        assert_eq!('b', state.buffer.layers[1].get_char((7, 7).into()).ch);
    }
}
