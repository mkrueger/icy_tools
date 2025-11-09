use std::{path::PathBuf, str::FromStr};

use super::{BufferParser, ansi};
use crate::{
    BitFont, CallbackAction, EditableScreen, EngineResult, Palette, Position, SKYPIX_PALETTE, Size, Spacing,
    ansi::EngineState,
    load_amiga_fonts,
    rip::bgi::{Bgi, Image, WriteMode},
};
const SKYPIX_SCREEN_SIZE: Size = Size { width: 640, height: 200 };

pub enum SkypixParseMode {
    Default,
    ParseFont(i32),
    ParseXModemTransfer(i32, i32, i32),
}

#[derive(Clone, Copy, PartialEq)]
pub enum DisplayMode {
    BitPlanes3,
    BitPlanes4,
}

pub struct Parser {
    fallback_parser: Box<ansi::Parser>,
    pub bgi: Bgi,
    display_mode: DisplayMode,
    brush: Image,
    parse_mode: SkypixParseMode,
    parameter: String,
    last_cmd_update: i32,
    cmd_counter: i32,

    graphic_cursor: Position,
    font: Option<BitFont>,
    fonts: Vec<(String, usize, &'static str)>,
}

impl Parser {
    pub fn new(fallback_parser: Box<ansi::Parser>, file_path: PathBuf) -> Self {
        let fonts = load_amiga_fonts();
        Self {
            fallback_parser,
            bgi: Bgi::new(file_path),
            display_mode: DisplayMode::BitPlanes4,
            parse_mode: SkypixParseMode::Default,
            parameter: String::new(),
            last_cmd_update: -1,
            cmd_counter: 0,
            fonts,
            font: None,
            graphic_cursor: Position::default(),
            brush: Image {
                width: 0,
                height: 0,
                data: Vec::new(),
            },
        }
    }

    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) {
        if let Some(font) = &self.font {
            let Some(glyph) = font.get_glyph(ch) else {
                return;
            };
            let x = self.graphic_cursor.x;
            let y = self.graphic_cursor.y;

            let lb = glyph.left_bearing;

            for i in 0..glyph.data.len() {
                for j in 0..glyph.width {
                    if (glyph.data[i] & (1 << (glyph.width - j - 1))) != 0 {
                        self.bgi.put_pixel(
                            buf,
                            lb - glyph.shift_left - font.shift_left + x + j as i32,
                            glyph.top_bearing - glyph.shift_up - font.shift_up + y + i as i32,
                            self.bgi.get_color(),
                        );
                    } /* else {
                    self.bgi
                    .put_pixel(glyph.left_bearing + x + j as i32, glyph.top_bearing + y + i as i32, self.bgi.get_bk_color());
                    }*/
                }
            }
            match font.spacing {
                Spacing::Proportional => {
                    self.graphic_cursor.x += lb + glyph.width as i32 + glyph.right_bearing as i32;
                }
                Spacing::CharacterCell => {
                    self.graphic_cursor.x += font.cell_size.width as i32;
                }
                Spacing::Monospace => {
                    self.graphic_cursor.x += font.size.width as i32;
                }
                Spacing::MultiCell => {
                    self.graphic_cursor.x += font.cell_size.width as i32;
                }
            }
            if self.graphic_cursor.x >= SKYPIX_SCREEN_SIZE.width {
                self.graphic_cursor.x = 0;
                let h = font.cell_size.height.max(font.raster_size.height).max(font.size.height);
                self.graphic_cursor.y += h;

                if self.graphic_cursor.y > SKYPIX_SCREEN_SIZE.height {
                    self.scroll_down(buf, self.graphic_cursor.y - SKYPIX_SCREEN_SIZE.height);
                    self.graphic_cursor.y = SKYPIX_SCREEN_SIZE.height;
                }
            }
        }
    }

    fn run_skypix_sequence(&mut self, cmd: i32, parameters: &[i32], buf: &mut dyn EditableScreen) -> EngineResult<CallbackAction> {
        self.cmd_counter += 1;
        match cmd {
            1 => {
                // SET_PIXEL
                if parameters.len() != 2 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command SET_PIXEL"));
                }
                let x = parameters[0];
                let y = parameters[1];
                self.bgi.put_pixel(buf, x, y, self.bgi.get_color());
                return Ok(CallbackAction::NoUpdate);
            }
            2 => {
                // DRAW_LINE
                if parameters.len() != 2 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command DRAW_LINE"));
                }
                let x = parameters[0];
                let y = parameters[1];
                self.bgi.line_to(buf, x, y);
                return Ok(CallbackAction::NoUpdate);
            }
            3 => {
                // AREA_FILL
                if parameters.len() != 3 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command AREA_FILL"));
                }
                // TODO: mode
                let _mode = parameters[0];
                let x = parameters[1];
                let y = parameters[2];
                self.bgi.flood_fill(buf, x, y, self.bgi.get_color());
                return Ok(CallbackAction::NoUpdate);
            }
            4 => {
                // RECTANGLE_FILL
                if parameters.len() != 4 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command RECTANGLE_FILL"));
                }
                let x1 = parameters[0];
                let y1 = parameters[1];
                let x2 = parameters[2];
                let y2 = parameters[3];
                self.bgi.bar(buf, x1, y1, x2, y2);
                return Ok(CallbackAction::NoUpdate);
            }
            5 => {
                // ELLIPSE
                if parameters.len() != 4 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command ELLIPSE"));
                }
                let x1 = parameters[0];
                let y1 = parameters[1];
                let a = parameters[2];
                let b = parameters[3];
                self.bgi.ellipse(buf, x1, y1, 0, 360, a, b);
                return Ok(CallbackAction::NoUpdate);
            }
            6 => {
                // GRAB_BRUSH
                if parameters.len() != 4 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command GRAB_BRUSH"));
                }
                let x1 = parameters[0];
                let y1 = parameters[1];
                let x2 = parameters[2];
                let y2 = parameters[3];
                self.brush = self.bgi.get_image(buf, x1, y1, x2, y2);
                return Ok(CallbackAction::NoUpdate);
            }
            7 => {
                // USE_BRUSH
                if parameters.len() != 8 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command USE_BRUSH"));
                }
                let src_x = parameters[0];
                let src_y = parameters[1];
                let dst_x = parameters[2];
                let dst_y = parameters[3];
                let width = parameters[4];
                let height = parameters[5];
                let _minterm = parameters[6];
                let _mask = parameters[7];
                self.bgi
                    .put_image2(buf, src_x, src_y, width, height, dst_x, dst_y, &self.brush, WriteMode::Copy);
                return Ok(CallbackAction::NoUpdate);
            }
            8 => {
                // MOVE_PEN
                if parameters.len() != 2 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command MOVE_PEN"));
                }
                let x = parameters[0];
                let y = parameters[1];
                self.bgi.move_to(x, y);
                return Ok(CallbackAction::NoUpdate);
            }
            9 => {
                // PLAY_SAMPLE
                if parameters.len() != 4 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command PLAY_SAMPLE"));
                }
                // not implemented originally, so we just ignore it
                log::info!("todo: SKYPIX_PLAY_SAMPLE");
                return Ok(CallbackAction::NoUpdate);
            }
            10 => {
                // SET_FONT
                if parameters.len() != 1 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command SET_FONT"));
                }
                let size = parameters[0];
                self.parameter.clear();
                self.parse_mode = SkypixParseMode::ParseFont(size);
                return Ok(CallbackAction::NoUpdate);
            }
            11 => {
                // NEW_PALETTE
                if parameters.len() != 16 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command NEW_PALETTE"));
                }
                let mut palette = Palette::new();
                for i in 0..16 {
                    let r = parameters[i] & 0xF;
                    let g = (parameters[i] >> 4) & 0xF;
                    let b = (parameters[i] >> 8) & 0xF;

                    palette.set_color(i as u32, amiga_color!(r, g, b));
                }
                return Ok(CallbackAction::NoUpdate);
            }

            12 => {
                // RESET_PALETTE
                *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);
                return Ok(CallbackAction::NoUpdate);
            }

            13 => {
                // FILLED_ELLIPSE
                if parameters.len() != 4 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command FILLED_ELLIPSE"));
                }
                let x1 = parameters[0];
                let y1 = parameters[1];
                let a = parameters[2];
                let b = parameters[3];
                self.bgi.fill_ellipse(buf, x1, y1, 0, 360, a, b);
                return Ok(CallbackAction::NoUpdate);
            }

            14 => {
                // DELAY
                if parameters.len() != 1 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command DELAY"));
                }
                let t = parameters[0];
                std::thread::sleep(std::time::Duration::from_millis((1000 * t as u64) / 60));
                return Ok(CallbackAction::NoUpdate);
            }

            15 => {
                // SET COLOUR OF PEN A
                if parameters.len() != 1 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command SET COLOUR OF PEN A"));
                }
                let col = parameters[0] as u8;
                self.bgi.set_color(col);
                buf.caret_mut().set_foreground(col as u32);
                return Ok(CallbackAction::NoUpdate);
            }

            16 => {
                // XMODEM TRANSFER
                if parameters.len() != 3 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command XMODEM_TRANSFER"));
                }
                let m = parameters[0];
                let a = parameters[1];
                let b = parameters[2];
                self.parameter.clear();
                self.parse_mode = SkypixParseMode::ParseXModemTransfer(m, a, b);
                return Ok(CallbackAction::NoUpdate);
            }

            17 => {
                // SELECT DISPLAY MODE
                if parameters.len() != 1 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command SELECT DISPLAY MODE"));
                }
                let m = parameters[0] as u8;
                match m {
                    1 => self.display_mode = DisplayMode::BitPlanes3,
                    2 => self.display_mode = DisplayMode::BitPlanes4,
                    _ => {
                        log::warn!("Unknown display mode: {}", m);
                    }
                }
                return Ok(CallbackAction::NoUpdate);
            }

            18 => {
                // SET COLOUR OF PEN B
                if parameters.len() != 1 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command SET COLOUR OF PEN B"));
                }
                let col = parameters[0] as u8;
                self.bgi.set_bk_color(col);
                buf.caret_mut().set_background(col as u32);
                return Ok(CallbackAction::NoUpdate);
            }

            19 => {
                // POSITION CURSOR
                if parameters.len() != 2 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command POSITION CURSOR"));
                }
                let x = (parameters[0] * 80) / SKYPIX_SCREEN_SIZE.width;
                let y = (parameters[1] * 25) / SKYPIX_SCREEN_SIZE.height;
                self.graphic_cursor = (parameters[0], parameters[1]).into();
                buf.caret_mut().set_position_xy(x + 1, y + 1);
                return Ok(CallbackAction::NoUpdate);
            }

            21 => {
                // CONTROLLER RETURN
                if parameters.len() != 3 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command POSITION CURSOR"));
                }
                let _c = parameters[0];
                let _x = parameters[1];
                let _y = parameters[2];
                log::warn!("todo: CONTROLLER RETURN");
                return Ok(CallbackAction::NoUpdate);
            }

            22 => {
                // DEFINE A SKYPIX GADGET
                if parameters.len() != 6 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command DEFINE A SKYPIX GADGET"));
                }
                let _n = parameters[0];
                let _c = parameters[1];
                let _x1 = parameters[2];
                let _y1 = parameters[3];
                let _x2 = parameters[4];
                let _y2 = parameters[5];
                log::warn!("todo: SKYPIX GADGET");
                return Ok(CallbackAction::NoUpdate);
            }

            99 => {
                // RESET_FONT?
                self.font = None;
                let x = (self.graphic_cursor.x * 80) / SKYPIX_SCREEN_SIZE.width;
                let y = (self.graphic_cursor.y * 25) / SKYPIX_SCREEN_SIZE.height;
                buf.caret_mut().set_position_xy(x + 1, y + 1);

                return Ok(CallbackAction::NoUpdate);
            }
            _ => {
                return Err(anyhow::Error::msg(format!("unknown skypix command {cmd}")));
            }
        }
    }

    fn load_font(&mut self, parameter: &str, size: i32) {
        let mut index = None;
        let mut old_size = 0;
        for (i, fonts) in self.fonts.iter().enumerate() {
            if fonts.0.eq_ignore_ascii_case(parameter) {
                if index.is_none() {
                    old_size = fonts.1;
                    index = Some(i);
                }
                if fonts.1 == size as usize {
                    self.font = Some(BitFont::from_str(fonts.2).unwrap());
                    return;
                }
                if fonts.1 > old_size && (fonts.1 < size as usize || old_size > size as usize) {
                    old_size = fonts.1;
                    index = Some(i);
                }
            }
        }
        if let Some(i) = index {
            log::warn!("can't load amiga font {parameter} with size {size}, fallback to {}", self.fonts[i].1);
            self.font = Some(BitFont::from_str(self.fonts[i].2).unwrap());
        } else {
            log::error!("unknown font: amiga_load_font {parameter} with size {size}");
            self.font = None;
        }
    }

    fn scroll_down(&mut self, buf: &mut dyn EditableScreen, lines: i32) {
        let img = self.bgi.get_image(buf, 0, lines, SKYPIX_SCREEN_SIZE.width, SKYPIX_SCREEN_SIZE.height - lines);
        self.bgi.clear_viewport(buf);
        self.bgi.put_image(buf, 0, 0, &img, WriteMode::Copy);
        self.cmd_counter += 1;
    }
}

impl Parser {}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        if buf.terminal_state().cleared_screen {
            self.font = None;
            buf.terminal_state_mut().cleared_screen = false;
            self.bgi.graph_defaults(buf);
            self.cmd_counter = 0;
            self.last_cmd_update = 0;
        }

        match self.parse_mode {
            SkypixParseMode::ParseFont(size) => {
                if ch == '!' || self.parameter.len() > 32 {
                    self.parse_mode = SkypixParseMode::Default;
                    self.load_font(&self.parameter.clone(), size);
                    return Ok(CallbackAction::NoUpdate);
                }
                self.parameter.push(ch);
                return Ok(CallbackAction::NoUpdate);
            }
            SkypixParseMode::ParseXModemTransfer(m, a, b) => {
                if ch == '!' || self.parameter.len() > 32 {
                    self.parse_mode = SkypixParseMode::Default;
                    log::warn!("initiate: SKYPIX_XMODEM_TRANSFER {m} {a} {b} {}", self.parameter);
                    return Ok(CallbackAction::XModemTransfer(self.parameter.clone()));
                }
                self.parameter.push(ch);
                return Ok(CallbackAction::NoUpdate);
            }
            _ => {
                if self.font.is_some() && self.fallback_parser.state == EngineState::Default && ch >= ' ' && ch <= '~' {
                    self.print_char(buf, ch);
                    return Ok(CallbackAction::NoUpdate);
                }

                match self.fallback_parser.print_char(buf, ch) {
                    Ok(CallbackAction::RunSkypixSequence(sequence)) => {
                        if sequence.len() == 0 {
                            return Err(anyhow::Error::msg("Empty sequence"));
                        }
                        return self.run_skypix_sequence(sequence[0], &sequence[1..], buf);
                    }
                    Ok(CallbackAction::ScrollDown(x)) => {
                        let lines = x * 8;
                        self.scroll_down(buf, lines);

                        return Ok(CallbackAction::Update);
                    }
                    x => x,
                }
            }
        }
    }
    /*
    fn get_picture_data(&mut self) -> Option<(Size, Vec<u8>)> {
        if self.last_cmd_update == self.cmd_counter {
            return None;
        }
        self.last_cmd_update = self.cmd_counter;
        let mut pixels = Vec::new();
        let pal = self.bgi.get_palette().clone();
        for i in &self.bgi.screen {
            if *i == 0 {
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
                continue;
            }
            let (r, g, b) = pal.get_rgb(*i as u32);
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(255);
        }
        Some((self.bgi.window, pixels))
    }*/
}
