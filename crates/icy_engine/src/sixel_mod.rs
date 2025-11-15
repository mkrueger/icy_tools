use crate::{EngineResult, Palette, Position, Rectangle, Size};

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
    pub picture_data: Vec<u8>,

    size: Size,
}

struct SixelParser {
    pos: Position,
    current_sixel_palette: Palette,
    current_sixel_color: u32,
    sixel_cursor: Position,
    parsed_numbers: Vec<i32>,
    state: SixelState,
    picture_data: Vec<Vec<u8>>,
    vertical_scale: i32,
    horizontal_scale: i32,

    height_set: bool,
}

impl Default for SixelParser {
    fn default() -> Self {
        Self {
            pos: Position::default(),
            current_sixel_palette: Palette::default(),
            current_sixel_color: 0,
            sixel_cursor: Position::default(),
            parsed_numbers: Vec::new(),
            state: SixelState::Read,
            height_set: false,
            picture_data: Vec::new(),
            vertical_scale: 1,
            horizontal_scale: 1,
        }
    }
}

impl SixelParser {
    pub fn parse_from(&mut self, _default_bg_color: [u8; 4], data: &str) -> EngineResult<Sixel> {
        for ch in data.chars() {
            self.parse_char(ch)?;
        }
        self.parse_char('#')?;
        let mut picture_data = Vec::new();
        for y in 0..self.height() {
            let line = &self.picture_data[y as usize];
            picture_data.extend(line);
        }
        Ok(Sixel {
            position: self.pos,
            vertical_scale: self.vertical_scale,
            horizontal_scale: self.horizontal_scale,
            picture_data,
            size: (self.width(), self.height()).into(),
        })
    }

    pub fn width(&self) -> i32 {
        if let Some(first_line) = self.picture_data.first() {
            (first_line.len() as i32) / 4
        } else {
            0
        }
    }

    pub fn height(&self) -> i32 {
        self.picture_data.len() as i32
    }

    fn parse_char(&mut self, ch: char) -> EngineResult<bool> {
        match self.state {
            SixelState::Read => {
                self.parse_sixel_data(ch)?;
            }
            SixelState::ReadColor => {
                if ch.is_ascii_digit() {
                    let d = match self.parsed_numbers.pop() {
                        Some(number) => number,
                        _ => 0,
                    };
                    self.parsed_numbers.push(parse_next_number(d, ch as u8));
                } else if ch == ';' {
                    self.parsed_numbers.push(0);
                } else {
                    if let Some(color) = self.parsed_numbers.first() {
                        self.current_sixel_color = *color as u32;
                    }
                    if self.parsed_numbers.len() > 1 {
                        if self.parsed_numbers.len() != 5 {
                            return Err(SixelSixelParserError::InvalidColorInSixelSequence.into());
                        }

                        match self.parsed_numbers.get(1) {
                            Some(2) => {
                                self.current_sixel_palette.set_color_rgb(
                                    self.current_sixel_color,
                                    (self.parsed_numbers[2] * 255 / 100) as u8,
                                    (self.parsed_numbers[3] * 255 / 100) as u8,
                                    (self.parsed_numbers[4] * 255 / 100) as u8,
                                );
                            }
                            Some(1) => {
                                self.current_sixel_palette.set_color_hsl(
                                    self.current_sixel_color,
                                    self.parsed_numbers[2] as f32 * 360.0 / (2.0 * std::f32::consts::PI),
                                    self.parsed_numbers[4] as f32 / 100.0, // sixel is hls
                                    self.parsed_numbers[3] as f32 / 100.0,
                                );
                            }
                            Some(n) => {
                                return Err(SixelSixelParserError::UnsupportedSixelColorformat(*n).into());
                            }
                            None => {
                                return Err(SixelSixelParserError::InvalidColorInSixelSequence.into());
                            }
                        }
                    }
                    self.parse_sixel_data(ch)?;
                }
            }
            SixelState::ReadSize => {
                if ch.is_ascii_digit() {
                    let d = match self.parsed_numbers.pop() {
                        Some(number) => number,
                        _ => 0,
                    };
                    self.parsed_numbers.push(parse_next_number(d, ch as u8));
                } else if ch == ';' {
                    self.parsed_numbers.push(0);
                } else {
                    if self.parsed_numbers.len() < 2 || self.parsed_numbers.len() > 4 {
                        return Err(SixelSixelParserError::InvalidPictureSize.into());
                    }
                    self.vertical_scale = self.parsed_numbers[0];
                    self.horizontal_scale = self.parsed_numbers[1];
                    if self.parsed_numbers.len() == 3 {
                        let height = self.parsed_numbers[2];
                        self.picture_data.resize(height as usize, Vec::new());
                        self.height_set = true;
                    }

                    if self.parsed_numbers.len() == 4 {
                        let height = self.parsed_numbers[3];
                        let width = self.parsed_numbers[2];
                        self.picture_data.resize(height as usize, vec![0; 4 * width as usize]);
                        self.height_set = true;
                    }
                    self.state = SixelState::Read;
                    self.parse_sixel_data(ch)?;
                }
            }
            SixelState::Repeat => {
                if ch.is_ascii_digit() {
                    let d = match self.parsed_numbers.pop() {
                        Some(number) => number,
                        _ => 0,
                    };
                    self.parsed_numbers.push(parse_next_number(d, ch as u8));
                } else {
                    if let Some(i) = self.parsed_numbers.first() {
                        for _ in 0..*i {
                            self.parse_sixel_data(ch)?;
                        }
                    } else {
                        return Err(SixelSixelParserError::NumberMissingInSixelRepeat.into());
                    }
                    self.state = SixelState::Read;
                }
            }
        }
        Ok(true)
    }

    fn translate_sixel_to_pixel(&mut self, ch: char) -> EngineResult<()> {
        /*let current_sixel = buf.layers[0].sixels.len() - 1;

        let sixel = &mut buf.layers[0].sixels[current_sixel];*/
        if ch < '?' {
            return Err(SixelSixelParserError::InvalidSixelChar(ch).into());
        }
        let mask = ch as u8 - b'?';

        let fg_color = self
            .current_sixel_palette
            .get_color((self.current_sixel_color) % self.current_sixel_palette.len() as u32)
            .clone();
        let x_pos = self.sixel_cursor.x;
        let y_pos = self.sixel_cursor.y * 6;

        let mut last_line = y_pos + 6;
        if self.height_set && last_line > self.height() {
            last_line = self.height();
        }

        if (self.picture_data.len() as i32) < last_line {
            self.picture_data.resize(last_line as usize, vec![0; (self.width() as usize) * 4]);
        }

        for i in 0..6 {
            if mask & (1 << i) != 0 {
                let translated_line = y_pos + i;
                if translated_line >= last_line {
                    break;
                }

                let cur_line = &mut self.picture_data[translated_line as usize];

                let offset = x_pos as usize * 4;
                if cur_line.len() <= offset {
                    cur_line.resize((x_pos as usize + 1) * 4, 0);
                }

                let (r, g, b) = fg_color.clone().get_rgb();
                cur_line[offset] = r;
                cur_line[offset + 1] = g;
                cur_line[offset + 2] = b;
                cur_line[offset + 3] = 0xFF;
            }
        }
        self.sixel_cursor.x += 1;
        Ok(())
    }

    fn parse_sixel_data(&mut self, ch: char) -> EngineResult<()> {
        match ch {
            '#' => {
                self.parsed_numbers.clear();
                self.state = SixelState::ReadColor;
            }
            '!' => {
                self.parsed_numbers.clear();
                self.state = SixelState::Repeat;
            }
            '-' => {
                self.sixel_cursor.x = 0;
                self.sixel_cursor.y += 1;
            }
            '$' => {
                self.sixel_cursor.x = 0;
            }
            '"' => {
                self.parsed_numbers.clear();
                self.state = SixelState::ReadSize;
            }
            _ => {
                if ch > '\x7F' {
                    return Ok(());
                }
                self.translate_sixel_to_pixel(ch)?;
            }
        }
        Ok(())
    }
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
    pub fn get_screen_rect(&self, font_dims: Size) -> Rectangle {
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
    pub fn parse_from(pos: Position, horizontal_scale: i32, vertical_scale: i32, default_bg_color: [u8; 4], data: &str) -> EngineResult<Self> {
        let mut parser = SixelParser {
            pos,
            vertical_scale,
            horizontal_scale,
            ..SixelParser::default()
        };
        parser.parse_from(default_bg_color, data)
    }

    pub fn get_width(&self) -> i32 {
        self.size.width
    }

    pub fn get_height(&self) -> i32 {
        self.size.height
    }

    pub fn get_size(&self) -> Size {
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

#[derive(Debug, Clone)]
pub enum SixelSixelParserError {
    InvalidChar(char),
    InvalidBuffer,
    UnsupportedEscapeSequence,
    UnsupportedCustomCommand(i32),
    Description(&'static str),
    UnsupportedControlCode(u32),
    UnsupportedFont(usize),
    UnsupportedSauceFont(String),
    UnexpectedSixelEnd(char),
    InvalidColorInSixelSequence,
    NumberMissingInSixelRepeat,
    InvalidSixelChar(char),
    UnsupportedSixelColorformat(i32),
    ErrorInSixelEngine(&'static str),
    InvalidPictureSize,

    InvalidRipAnsiQuery(i32),

    Error(String),
}

impl std::fmt::Display for SixelSixelParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SixelSixelParserError::InvalidChar(ch) => write!(f, "invalid character {ch}"),
            SixelSixelParserError::UnsupportedEscapeSequence => {
                write!(f, "unsupported escape sequence")
            }
            SixelSixelParserError::Description(str) => write!(f, "{str}"),
            SixelSixelParserError::UnsupportedControlCode(code) => {
                write!(f, "unsupported control code 0x{:02X}", *code)
            }
            SixelSixelParserError::UnsupportedCustomCommand(code) => {
                write!(f, "unsupported custom ansi command: {}", *code)
            }
            SixelSixelParserError::UnsupportedFont(code) => write!(f, "font {} not supported", *code),
            SixelSixelParserError::UnsupportedSauceFont(name) => write!(f, "font {name} not supported"),
            SixelSixelParserError::UnexpectedSixelEnd(ch) => {
                write!(f, "sixel sequence ended with <esc>{ch} expected '\\'")
            }
            SixelSixelParserError::InvalidBuffer => write!(f, "output buffer is invalid"),
            SixelSixelParserError::InvalidColorInSixelSequence => {
                write!(f, "invalid color in sixel sequence")
            }
            SixelSixelParserError::NumberMissingInSixelRepeat => {
                write!(f, "sixel repeat sequence is missing number")
            }
            SixelSixelParserError::InvalidSixelChar(ch) => write!(f, "{ch} invalid in sixel data"),
            SixelSixelParserError::UnsupportedSixelColorformat(i) => {
                write!(f, "{i} invalid color format in sixel data")
            }
            SixelSixelParserError::ErrorInSixelEngine(err) => write!(f, "sixel engine error: {err}"),
            SixelSixelParserError::InvalidPictureSize => write!(f, "invalid sixel picture size description"),
            SixelSixelParserError::InvalidRipAnsiQuery(i) => write!(f, "invalid rip ansi query <esc>[{i}!"),
            SixelSixelParserError::Error(err) => write!(f, "Parse error: {err}"),
        }
    }
}

impl std::error::Error for SixelSixelParserError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}
