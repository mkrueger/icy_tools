use serde::{Deserialize, Serialize};

use crate::{AttributeColor, AttributedChar, EngineError, Layer, Position, Result, Size, TextBuffer, TextPane};

impl TextBuffer {
    pub fn layer_with_baked_effects(&self, index: usize) -> Result<Layer> {
        let source = self
            .layers
            .get(index)
            .ok_or_else(|| EngineError::Generic(format!("Invalid layer index: {index}")))?;
        source.effects.validate()?;
        let mut layer = source.clone();
        if layer.is_text() {
            layer.set_live_text(None);
            layer.properties.is_locked = false;
        }
        if source.effects.is_empty() {
            return Ok(layer);
        }
        // An opaque layer normally fills undefined cells from the lower stack.
        // Once masked holes become ordinary transparent cells, retain that fill elsewhere.
        let needs_background = !source.properties.has_alpha_channel && source.effects.mask.as_ref().is_some_and(|mask| mask.enabled);
        let composite = if needs_background {
            let mut buffer = self.clone();
            for (index, layer) in buffer.layers.iter_mut().enumerate() {
                layer.properties.is_visible = self.layer_is_visible(index);
                layer.parent_group = None;
            }
            buffer.layers.truncate(index + 1);
            buffer.layers[index].properties.is_visible = true;
            buffer.show_tags = false;
            Some(buffer)
        } else {
            None
        };
        layer.graphemes.clear();
        layer.preallocate_lines(source.width(), source.height());
        for y in 0..source.height() {
            for x in 0..source.width() {
                let pos = Position::new(x, y);
                let cell = if source.effects.is_masked(pos) {
                    AttributedChar::invisible()
                } else if let Some(buffer) = &composite {
                    buffer.char_at(pos + source.offset())
                } else {
                    source.display_char_at(pos)
                };
                layer.set_char_unchecked(pos, cell);
            }
        }
        for y in 0..source.height() {
            for x in 0..source.width() {
                let pos = Position::new(x, y);
                if !source.effects.is_masked(pos) {
                    let grapheme = if let Some(buffer) = &composite {
                        buffer.grapheme_at(pos + source.offset())
                    } else {
                        source.grapheme_at(pos)
                    };
                    if let Some((text, width)) = grapheme.filter(|(_, width)| x + *width as i32 <= source.width()) {
                        layer
                            .graphemes
                            .entry(y)
                            .or_default()
                            .insert(x, super::unicode::Grapheme { text: text.to_owned(), width });
                    }
                }
            }
        }
        layer.effects = LayerEffects::default();
        if needs_background {
            layer.properties.has_alpha_channel = true;
            layer.properties.mode = crate::Mode::Normal;
        }
        Ok(layer)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteRemap {
    pub enabled: bool,
    pub colors: [u8; 16],
}

impl Default for PaletteRemap {
    fn default() -> Self {
        Self {
            enabled: true,
            colors: std::array::from_fn(|i| i as u8),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerMask {
    pub enabled: bool,
    size: Size,
    hidden: Vec<u8>,
}

fn mask_length(size: Size) -> Option<usize> {
    let width = usize::try_from(size.width).ok().filter(|width| *width > 0)?;
    let height = usize::try_from(size.height).ok().filter(|height| *height > 0)?;
    let cells = width.checked_mul(height)?;
    (cells <= crate::limits::MAX_BUFFER_WIDTH as usize * crate::limits::MAX_BUFFER_HEIGHT as usize).then(|| cells.div_ceil(8))
}

impl LayerMask {
    pub fn new(size: Size) -> Result<Self> {
        let length = mask_length(size).ok_or_else(|| EngineError::Generic(format!("Invalid layer mask dimensions: {size}")))?;
        Ok(Self {
            enabled: true,
            size,
            hidden: vec![0; length],
        })
    }

    pub fn size(&self) -> Size {
        self.size
    }

    fn index(&self, pos: Position) -> Option<usize> {
        (pos.x >= 0 && pos.y >= 0 && pos.x < self.size.width && pos.y < self.size.height).then(|| (pos.y * self.size.width + pos.x) as usize)
    }

    pub fn is_hidden(&self, pos: Position) -> bool {
        self.enabled
            && self
                .index(pos)
                .is_some_and(|i| self.hidden.get(i / 8).is_some_and(|byte| byte & (1 << (i % 8)) != 0))
    }

    pub fn set_hidden(&mut self, pos: Position, hidden: bool) {
        if let Some(i) = self.index(pos) {
            if hidden {
                self.hidden[i / 8] |= 1 << (i % 8);
            } else {
                self.hidden[i / 8] &= !(1 << (i % 8));
            }
        }
    }

    pub fn transformed(&self, size: Size, source_position: impl Fn(Position) -> Position) -> Result<Self> {
        let mut result = Self::new(size)?;
        for y in 0..size.height {
            for x in 0..size.width {
                let pos = Position::new(x, y);
                let hidden = self.index(source_position(pos)).is_some_and(|i| self.hidden[i / 8] & (1 << (i % 8)) != 0);
                result.set_hidden(pos, hidden);
            }
        }
        result.enabled = self.enabled;
        Ok(result)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerEffects {
    pub remap: Option<PaletteRemap>,
    pub mask: Option<LayerMask>,
}

impl LayerEffects {
    pub fn is_empty(&self) -> bool {
        self.remap.is_none() && self.mask.is_none()
    }

    pub fn is_masked(&self, pos: Position) -> bool {
        self.mask.as_ref().is_some_and(|mask| mask.is_hidden(pos))
    }

    pub fn remap_char(&self, mut cell: AttributedChar) -> AttributedChar {
        let Some(remap) = self.remap.as_ref().filter(|remap| remap.enabled) else {
            return cell;
        };
        let color = |value| match value {
            AttributeColor::Palette(index @ 0..=15) => AttributeColor::Palette(remap.colors[usize::from(index)]),
            _ => value,
        };
        let mut fg = cell.attribute.foreground_color();
        if let AttributeColor::Palette(index @ 0..=15) = fg {
            fg = AttributeColor::Palette(if index < 8 && cell.attribute.is_bold() { index + 8 } else { index });
            cell.attribute.set_is_bold(false);
        }
        cell.attribute.set_foreground_color(color(fg));
        cell.attribute.set_background_color(color(cell.attribute.background_color()));
        cell
    }

    pub fn validate(&self) -> Result<()> {
        if self.remap.as_ref().is_some_and(|remap| remap.colors.iter().any(|index| *index >= 16)) {
            return Err(EngineError::Generic("Layer remap colors must be in 0..15".into()));
        }
        if let Some(mask) = &self.mask {
            if mask_length(mask.size) != Some(mask.hidden.len()) {
                return Err(EngineError::Generic("Invalid layer mask data".into()));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mode, TextAttribute};

    fn buffer() -> TextBuffer {
        let mut buffer = TextBuffer::new((4, 1));
        for x in 0..4 {
            buffer.layers[0].set_char((x, 0), AttributedChar::new('B', TextAttribute::new(7, 1)));
        }
        let mut layer = Layer::new("Effect", (2, 1));
        layer.set_offset((1, 0));
        for x in 0..2 {
            layer.set_char((x, 0), AttributedChar::new('A', TextAttribute::new(4, 0)));
        }
        let mut remap = PaletteRemap::default();
        remap.colors[4] = 2;
        let mut mask = LayerMask::new(layer.size()).unwrap();
        mask.set_hidden(Position::default(), true);
        layer.effects = LayerEffects {
            remap: Some(remap),
            mask: Some(mask),
        };
        buffer.layers.push(layer);
        buffer
    }

    #[test]
    fn layer_effects_are_reversible_and_mask_opaque_layers_in_every_mode() {
        for mode in [Mode::Normal, Mode::Chars, Mode::Attributes] {
            let mut buffer = buffer();
            buffer.layers[1].properties.mode = mode;
            let raw = buffer.layers[1].char_at(Position::default());
            assert_eq!(buffer.char_at((1, 0).into()), buffer.layers[0].char_at((1, 0).into()));
            assert_eq!(buffer.layers[1].char_at(Position::default()), raw);
            buffer.layers[1].effects.mask.as_mut().unwrap().enabled = false;
            if mode != Mode::Chars {
                assert_eq!(buffer.char_at((1, 0).into()).attribute.foreground(), 2);
            }
            buffer.layers[1].effects.remap.as_mut().unwrap().enabled = false;
            if mode == Mode::Normal {
                assert_eq!(buffer.char_at((1, 0).into()), raw);
            }
        }
    }

    #[test]
    fn layer_effects_move_and_bake_without_changing_the_visible_result() {
        let mut buffer = buffer();
        buffer.layers[1].set_offset((0, 0));
        assert_eq!(buffer.char_at((0, 0).into()).ch, 'B');
        assert_eq!(buffer.char_at((1, 0).into()).attribute.foreground(), 2);
        let before = buffer.render_to_rgba(&crate::Rectangle::from(0, 0, 4, 1).into(), false);
        buffer.layers[1] = buffer.layer_with_baked_effects(1).unwrap();
        assert!(buffer.layers[1].effects.is_empty());
        assert_eq!(buffer.render_to_rgba(&crate::Rectangle::from(0, 0, 4, 1).into(), false), before);
    }

    #[test]
    fn layer_effects_bake_sparse_layers_and_retain_unmasked_backgrounds() {
        let mut buffer = TextBuffer::new((3, 1));
        buffer.layers[0].clear();
        let mut mask = LayerMask::new(buffer.size()).unwrap();
        mask.set_hidden(Position::new(1, 0), true);
        buffer.layers[0].effects.mask = Some(mask);
        let before: Vec<_> = (0..3).map(|x| buffer.char_at((x, 0).into())).collect();
        buffer.layers[0] = buffer.layer_with_baked_effects(0).unwrap();
        assert_eq!((0..3).map(|x| buffer.char_at((x, 0).into())).collect::<Vec<_>>(), before);
    }

    #[test]
    fn layer_effects_bake_preserves_graphemes_without_overwriting_masked_cells() {
        for mode in [Mode::Normal, Mode::Chars, Mode::Attributes] {
            let mut buffer = buffer();
            let source = &mut buffer.layers[1];
            source.put_grapheme(Position::default(), "\u{754c}".into(), 2, TextAttribute::new(4, 0));
            source.effects.mask.as_mut().unwrap().set_hidden(Position::default(), false);
            source.effects.mask.as_mut().unwrap().set_hidden(Position::new(1, 0), true);
            source.properties.mode = mode;
            let before: Vec<_> = (0..4).map(|x| buffer.char_at((x, 0).into())).collect();
            let grapheme = buffer.grapheme_at(Position::new(1, 0)).map(|(text, width)| (text.to_owned(), width));
            buffer.layers[1] = buffer.layer_with_baked_effects(1).unwrap();
            assert_eq!((0..4).map(|x| buffer.char_at((x, 0).into())).collect::<Vec<_>>(), before);
            assert_eq!(buffer.grapheme_at(Position::new(1, 0)).map(|(text, width)| (text.to_owned(), width)), grapheme);
            assert!(!buffer.layers[1].char_at(Position::new(1, 0)).is_visible());
        }
    }

    #[test]
    fn layer_effects_remap_keeps_rgb_extended_colors_and_transparency() {
        let mut remap = PaletteRemap::default();
        remap.colors[12] = 1;
        let effects = LayerEffects {
            remap: Some(remap),
            mask: None,
        };
        let mut cell = AttributedChar::new('A', TextAttribute::new(4, 0));
        cell.attribute.set_is_bold(true);
        assert_eq!(effects.remap_char(cell).attribute.foreground(), 1);
        assert!(!effects.remap_char(cell).attribute.is_bold());
        for color in [AttributeColor::Rgb(1, 2, 3), AttributeColor::ExtendedPalette(12), AttributeColor::Transparent] {
            cell.attribute.set_foreground_color(color);
            cell.attribute.set_background_color(color);
            assert_eq!(effects.remap_char(cell), cell);
        }
    }

    #[test]
    fn layer_effects_encoding_validates_payloads_and_keeps_disabled_masks() {
        let mut effects = buffer().layers[1].effects.clone();
        effects.mask.as_mut().unwrap().enabled = false;
        effects.remap.as_mut().unwrap().enabled = false;
        let bytes = effects.encode().unwrap();
        assert_eq!(LayerEffects::decode(&bytes).unwrap(), effects);
        for end in 0..bytes.len() {
            assert!(LayerEffects::decode(&bytes[..end]).is_err(), "truncated at {end}");
        }
        for (index, invalid) in [(0, 2), (2, 2), (3, 2), (4, 16), (20, 2), (21, 2)] {
            let mut broken = bytes.clone();
            broken[index] = invalid;
            assert!(LayerEffects::decode(&broken).is_err(), "invalid byte {index}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(LayerEffects::decode(&trailing).is_err());
        assert!(LayerMask::new(Size::new(i32::MAX, i32::MAX)).is_err());
        assert!(LayerMask::new(Size::new(0, 1)).is_err());
        effects.remap.as_mut().unwrap().colors[0] = 16;
        assert!(effects.encode().is_err());
    }

    #[test]
    fn layer_effects_clipboard_contains_the_appearance_not_the_recipe() {
        let buffer = buffer();
        let selection = Some(crate::Selection::from(crate::Rectangle::from(1, 0, 2, 1)));
        let bytes = crate::clipboard::clipboard_data(&buffer, 1, &crate::SelectionMask::default(), &selection).unwrap();
        let pasted = crate::clipboard::from_clipboard_data(buffer.buffer_type, &bytes).unwrap();
        assert!(pasted.effects.is_empty());
        assert!(!pasted.char_at(Position::default()).is_visible());
        assert_eq!(pasted.char_at((1, 0).into()).attribute.foreground(), 2);
        assert_eq!(buffer.layers[1].char_at((1, 0).into()).attribute.foreground(), 4);
    }
}

impl LayerEffects {
    pub(crate) fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = vec![1, 0];
        bytes.push(u8::from(self.remap.is_some()));
        if let Some(remap) = &self.remap {
            bytes.push(u8::from(remap.enabled));
            bytes.extend(remap.colors);
        }
        bytes.push(u8::from(self.mask.is_some()));
        if let Some(mask) = &self.mask {
            bytes.push(u8::from(mask.enabled));
            bytes.extend(mask.size.width.to_le_bytes());
            bytes.extend(mask.size.height.to_le_bytes());
            bytes.extend(&mask.hidden);
        }
        Ok(bytes)
    }

    pub(crate) fn decode(mut bytes: &[u8]) -> Result<Self> {
        fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Result<&'a [u8]> {
            if bytes.len() < length {
                return Err(EngineError::Generic("Truncated layer effects".into()));
            }
            let (value, rest) = bytes.split_at(length);
            *bytes = rest;
            Ok(value)
        }
        fn flag(bytes: &mut &[u8]) -> Result<bool> {
            match take(bytes, 1)?[0] {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(EngineError::Generic("Invalid layer effects flag".into())),
            }
        }
        if take(&mut bytes, 2)? != [1, 0] {
            return Err(EngineError::Generic("Unsupported layer effects version".into()));
        }
        let remap = if flag(&mut bytes)? {
            Some(PaletteRemap {
                enabled: flag(&mut bytes)?,
                colors: take(&mut bytes, 16)?.try_into().unwrap(),
            })
        } else {
            None
        };
        let mask = if flag(&mut bytes)? {
            let enabled = flag(&mut bytes)?;
            let width = i32::from_le_bytes(take(&mut bytes, 4)?.try_into().unwrap());
            let height = i32::from_le_bytes(take(&mut bytes, 4)?.try_into().unwrap());
            if mask_length(Size::new(width, height)) != Some(bytes.len()) {
                return Err(EngineError::Generic("Invalid layer mask dimensions or payload".into()));
            }
            let hidden = bytes.to_vec();
            bytes = &[];
            Some(LayerMask {
                enabled,
                size: Size::new(width, height),
                hidden,
            })
        } else {
            None
        };
        if !bytes.is_empty() {
            return Err(EngineError::Generic("Trailing layer effects data".into()));
        }
        let effects = Self { remap, mask };
        effects.validate()?;
        Ok(effects)
    }
}
