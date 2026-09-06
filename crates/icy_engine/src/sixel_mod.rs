use crate::{Position, Rectangle, Result, Size};

#[derive(Clone, Debug, Copy)]
pub enum SixelState {
    Read,
    ReadColor,
    ReadSize,
    Repeat,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sixel {
    pub position: Position,
    pub pixel_offset: Position,

    pub vertical_scale: i32,
    pub horizontal_scale: i32,
    /// Raw sixel data (RGBA format)
    pub picture_data: Vec<u8>,

    size: Size,
}

impl Sixel {
    pub fn new(position: Position) -> Self {
        Self {
            position,
            pixel_offset: Position::default(),
            vertical_scale: 1,
            horizontal_scale: 1,
            picture_data: Vec::new(),
            size: Size::default(),
        }
    }

    pub fn from_data(size: impl Into<Size>, vertical_scale: i32, horizontal_scale: i32, data: Vec<u8>) -> Self {
        Self {
            position: Position::default(),
            pixel_offset: Position::default(),
            vertical_scale,
            horizontal_scale,
            picture_data: data,
            size: size.into(),
        }
    }

    /// Coordinates are points
    pub fn screen_rect(&self, font_dims: Size) -> Rectangle {
        let x = self.position.x * font_dims.width + self.pixel_offset.x;
        let y = self.position.y * font_dims.height + self.pixel_offset.y;
        Rectangle {
            start: Position::new(x, y),
            size: self.size,
        }
    }

    /// Gets the position of the sixel in the buffer.
    pub fn as_rectangle(&self, font_dims: Size) -> Rectangle {
        let x = self.position.x;
        let y = self.position.y;
        Rectangle {
            start: Position::new(x, y),
            size: Size::new(
                ((self.pixel_offset.x + self.size.width) as f32 / font_dims.width as f32).ceil() as i32,
                ((self.pixel_offset.y + self.size.height) as f32 / font_dims.height as f32).ceil() as i32,
            ),
        }
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn parse_from(aspect_ratio: Option<u16>, zero_color: Option<u16>, grid_size: Option<u16>, sixel_data: &[u8]) -> Result<Self> {
        let mut decoder = icy_sixel::SixelDecoder::new();
        Self::parse_from_with_decoder(&mut decoder, aspect_ratio, zero_color, grid_size, sixel_data)
    }

    pub fn parse_from_with_decoder(
        decoder: &mut icy_sixel::SixelDecoder,
        aspect_ratio: Option<u16>,
        zero_color: Option<u16>,
        grid_size: Option<u16>,
        sixel_data: &[u8],
    ) -> Result<Self> {
        let settings = icy_sixel::decoder::DcsSettings::new(aspect_ratio, zero_color, grid_size);
        let image = decoder
            .decode_from_dcs(sixel_data, settings)
            .map_err(|e| crate::EngineError::SixelDecodeError { message: e.to_string() })?;
        let (vertical_scale, horizontal_scale) = raster_scale(sixel_data);

        Ok(Sixel {
            position: Position::default(),
            pixel_offset: Position::default(),
            vertical_scale,
            horizontal_scale,
            picture_data: image.pixels,
            size: Size::new(image.width as i32, image.height as i32),
        })
    }

    pub fn width(&self) -> i32 {
        self.size.width
    }

    pub fn height(&self) -> i32 {
        self.size.height
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn set_width(&mut self, width: i32) {
        self.size.width = width;
    }

    pub fn set_height(&mut self, height: i32) {
        self.size.height = height;
    }

    pub fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    /// Apply raster scaling without exceeding the decoded-image memory budget.
    /// Invalid dimensions, inconsistent RGBA data and allocation failures are errors.
    /// The image is unchanged on error.
    pub fn apply_raster_scale(&mut self) -> Result<()> {
        let invalid = |message: &str| crate::EngineError::SixelDecodeError { message: message.to_string() };
        if self.size.width <= 0 || self.size.height <= 0 {
            return Err(invalid("invalid raster dimensions"));
        }
        let horizontal = self.horizontal_scale.max(1);
        let vertical = self.vertical_scale.max(1);
        let output_width = self.size.width.checked_mul(horizontal).ok_or_else(|| invalid("raster width overflow"))?;
        let output_height = self.size.height.checked_mul(vertical).ok_or_else(|| invalid("raster height overflow"))?;
        let output_width = output_width as usize;
        let output_height = output_height as usize;
        let output_len = output_width
            .checked_mul(output_height)
            .and_then(|pixels| pixels.checked_mul(4))
            .filter(|bytes| *bytes <= crate::limits::MAX_SIXEL_BYTES)
            .ok_or_else(|| invalid("scaled raster exceeds sixel memory limit"))?;

        let source_width = self.size.width as usize;
        let source_height = self.size.height as usize;
        let source_len = source_width.checked_mul(source_height).and_then(|pixels| pixels.checked_mul(4));
        if source_len != Some(self.picture_data.len()) {
            return Err(invalid("raster dimensions do not match RGBA data"));
        }
        if horizontal == 1 && vertical == 1 {
            return Ok(());
        }
        let mut output = Vec::new();
        output.try_reserve_exact(output_len).map_err(|e| crate::EngineError::SixelDecodeError {
            message: format!("cannot allocate scaled raster: {e}"),
        })?;
        output.resize(output_len, 0u8);
        for y in 0..output_height {
            let source_y = y / vertical as usize;
            for x in 0..output_width {
                let source_x = x / horizontal as usize;
                let source = (source_y * source_width + source_x) * 4;
                let destination = (y * output_width + x) * 4;
                output[destination..destination + 4].copy_from_slice(&self.picture_data[source..source + 4]);
            }
        }
        self.picture_data = output;
        self.size = Size::new(output_width as i32, output_height as i32);
        self.horizontal_scale = 1;
        self.vertical_scale = 1;
        Ok(())
    }
}

fn raster_scale(data: &[u8]) -> (i32, i32) {
    let Some(start) = data.iter().position(|byte| *byte == b'"') else {
        return (1, 1);
    };
    let mut values = [1i32; 2];
    let mut value = 0i32;
    let mut has_value = false;
    let mut index = 0usize;
    for byte in &data[start + 1..] {
        match byte {
            b'0'..=b'9' => {
                value = value.saturating_mul(10).saturating_add(i32::from(*byte - b'0'));
                has_value = true;
            }
            b';' => {
                if index < values.len() {
                    values[index] = if has_value { value.max(1) } else { 1 };
                    index += 1;
                }
                value = 0;
                has_value = false;
                if index == values.len() {
                    break;
                }
            }
            _ => break,
        }
    }
    if index < values.len() && has_value {
        values[index] = value.max(1);
    }
    // Raster attributes are Pan;Pad: vertical scale first, horizontal second.
    (values[0], values[1])
}

#[inline(always)]
pub fn parse_next_number(x: i32, ch: u8) -> i32 {
    x.saturating_mul(10).saturating_add(ch as i32).saturating_sub(b'0' as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_scale_preserves_pixels_and_is_idempotent() {
        let mut sixel = Sixel::from_data((2, 2), 2, 3, (0..16).collect());
        sixel.apply_raster_scale().unwrap();
        assert_eq!(sixel.size(), Size::new(6, 4));
        for y in 0..4 {
            for x in 0..6 {
                let source = ((y / 2) * 2 + x / 3) * 4;
                let offset = (y * 6 + x) * 4;
                assert_eq!(&sixel.picture_data[offset..offset + 4], &(source as u8..source as u8 + 4).collect::<Vec<_>>());
            }
        }
        let scaled = sixel.clone();
        sixel.apply_raster_scale().unwrap();
        assert_eq!(sixel, scaled);
    }

    #[test]
    fn raster_scale_rejects_amplification_without_changing_image() {
        let mut sixel = Sixel::parse_from(None, None, None, b"\"100000;100000;1;6~").unwrap();
        let original = sixel.clone();
        assert!(sixel.apply_raster_scale().is_err());
        assert_eq!(sixel, original);
    }

    #[test]
    fn raster_scale_rejects_invalid_dimensions_and_data() {
        for (size, data_len) in [((0, 1), 0), ((-1, 1), 0), ((1, -1), 0), ((2, 2), 15), ((2, 2), 17)] {
            for scale in [1, 2] {
                let mut sixel = Sixel::from_data(size, scale, scale, vec![0; data_len]);
                let original = sixel.clone();
                assert!(sixel.apply_raster_scale().is_err());
                assert_eq!(sixel, original);
            }
        }
    }

    #[test]
    fn raster_scale_rejects_dimension_overflow() {
        for (vertical, horizontal) in [(i32::MAX, 1), (1, i32::MAX), (i32::MAX, i32::MAX)] {
            let mut sixel = Sixel::from_data((2, 2), vertical, horizontal, vec![0; 16]);
            assert!(sixel.apply_raster_scale().is_err());
        }
    }

    #[test]
    fn raster_scale_keeps_unscaled_allocation() {
        let mut sixel = Sixel::from_data((1, 1), 1, 1, vec![1, 2, 3, 255]);
        let ptr = sixel.picture_data.as_ptr();
        sixel.apply_raster_scale().unwrap();
        assert_eq!(sixel.picture_data.as_ptr(), ptr);
    }
}
