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

    pub fn apply_raster_scale(&mut self) {
        let horizontal = self.horizontal_scale.max(1);
        let vertical = self.vertical_scale.max(1);
        if horizontal == 1 && vertical == 1 {
            return;
        }
        let source_width = self.size.width as usize;
        let source_height = self.size.height as usize;
        let output_width = source_width * horizontal as usize;
        let output_height = source_height * vertical as usize;
        let mut output = vec![0u8; output_width * output_height * 4];
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
