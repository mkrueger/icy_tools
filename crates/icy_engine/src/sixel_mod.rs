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
            vertical_scale: 1,
            horizontal_scale: 1,
            picture_data: Vec::new(),
            size: Size::default(),
        }
    }

    pub fn from_data(size: impl Into<Size>, vertical_scale: i32, horizontal_scale: i32, data: Vec<u8>) -> Self {
        Self {
            position: Position::default(),
            vertical_scale,
            horizontal_scale,
            picture_data: data,
            size: size.into(),
        }
    }

    /// Coordinates are points
    pub fn screen_rect(&self, font_dims: Size) -> Rectangle {
        let x = self.position.x * font_dims.width;
        let y = self.position.y * font_dims.height;
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
                (self.size.width as f32 / font_dims.width as f32).ceil() as i32,
                (self.size.height as f32 / font_dims.height as f32).ceil() as i32,
            ),
        }
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn parse_from(aspect_ratio: Option<u16>, zero_color: Option<u16>, grid_size: Option<u16>, sixel_data: &[u8]) -> Result<Self> {
        let settings = icy_sixel::decoder::DcsSettings::new(aspect_ratio, zero_color, grid_size);
        let image =
            icy_sixel::SixelImage::decode_from_dcs(sixel_data, settings).map_err(|e| crate::EngineError::SixelDecodeError { message: e.to_string() })?;

        Ok(Sixel {
            position: Position::default(),
            vertical_scale: 1,
            horizontal_scale: 1,
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
}

#[inline(always)]
pub fn parse_next_number(x: i32, ch: u8) -> i32 {
    x.saturating_mul(10).saturating_add(ch as i32).saturating_sub(b'0' as i32)
}
