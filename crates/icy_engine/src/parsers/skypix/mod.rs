use std::path::PathBuf;

use super::{ansi, BufferParser};
use crate::{
    rip::bgi::{Bgi, Image, WriteMode},
    Buffer, CallbackAction, Caret, Color, EngineResult, Palette, Size, SKYPIX_PALETTE,
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
}

impl Parser {
    pub fn new(fallback_parser: Box<ansi::Parser>, file_path: PathBuf) -> Self {
        Self {
            fallback_parser,
            bgi: Bgi::new(SKYPIX_SCREEN_SIZE, file_path),
            display_mode: DisplayMode::BitPlanes4,
            parse_mode: SkypixParseMode::Default,
            parameter: String::new(),
            last_cmd_update: -1,
            cmd_counter: 0,
            brush: Image {
                width: 0,
                height: 0,
                data: Vec::new(),
            },
        }
    }

    fn run_skypix_sequence(&mut self, cmd: i32, parameters: &[i32], buf: &mut Buffer, caret: &mut Caret) -> EngineResult<CallbackAction> {
        self.cmd_counter += 1;
        println!(" run cmd {cmd} = par {parameters:?}");
        match cmd {
            1 => {
                // SET_PIXEL
                if parameters.len() != 2 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command SET_PIXEL"));
                }
                let x = parameters[0];
                let y = parameters[1];
                self.bgi.put_pixel(x, y, self.bgi.get_color());
                return Ok(CallbackAction::NoUpdate);
            }
            2 => {
                // DRAW_LINE
                if parameters.len() != 2 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command DRAW_LINE"));
                }
                let x = parameters[0];
                let y = parameters[1];
                self.bgi.line_to(x, y);
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
                self.bgi.flood_fill(x, y, self.bgi.get_color());
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
                self.bgi.bar(x1, y1, x2, y2);
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
                self.bgi.ellipse(x1, y1, 0, 360, a, b);
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
                self.brush = self.bgi.get_image(x1, y1, x2, y2);
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
                self.bgi.put_image2(src_x, src_y, width, height, dst_x, dst_y, &self.brush, WriteMode::Copy);
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

                log::warn!("todo: SET_FONT");
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
                buf.palette = Palette::from_slice(&SKYPIX_PALETTE);
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
                self.bgi.fill_ellipse(x1, y1, 0, 360, a, b);
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
                caret.set_foreground(col as u32);
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
                caret.set_background(col as u32);
                return Ok(CallbackAction::NoUpdate);
            }

            19 => {
                // POSITION CURSOR
                if parameters.len() != 2 {
                    return Err(anyhow::Error::msg("Invalid number of parameters for skypix command POSITION CURSOR"));
                }
                let x = (parameters[0] * 80) / SKYPIX_SCREEN_SIZE.width;
                let y = (parameters[1] * 25) / SKYPIX_SCREEN_SIZE.height;
                caret.set_position_xy(x, y);
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
            _ => {
                return Err(anyhow::Error::msg(format!("unknown skypix command {cmd}")));
            }
        }
    }

    fn load_font(&self, parameter: &str, size: i32) {
        log::warn!("todo: load_font {parameter} with size {size}");
    }
}

impl Parser {}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        match self.parse_mode {
            SkypixParseMode::ParseFont(size) => {
                if ch == '!' || self.parameter.len() > 32 {
                    self.load_font(&self.parameter, size);
                    self.parse_mode = SkypixParseMode::Default;
                    return Ok(CallbackAction::NoUpdate);
                }
                self.parameter.push(ch);
                return Ok(CallbackAction::NoUpdate);
            }
            SkypixParseMode::ParseXModemTransfer(m, a, b) => {
                if ch == '!' || self.parameter.len() > 32 {
                    self.parse_mode = SkypixParseMode::Default;
                    // For what exactly is this?
                    // Skypix looks like a half baked standard
                    // As long as it's not defined/unclear I'm not implementing transferring unknown files to the users system
                    log::warn!("todo: SKYPIX_XMODEM_TRANSFER {m} {a} {b} {}", self.parameter);
                    return Ok(CallbackAction::NoUpdate);
                }
                self.parameter.push(ch);
                return Ok(CallbackAction::NoUpdate);
            }
            _ => match self.fallback_parser.print_char(buf, current_layer, caret, ch) {
                Ok(CallbackAction::RunSkypixSequence(sequence)) => {
                    if sequence.len() == 0 {
                        return Err(anyhow::Error::msg("Empty sequence"));
                    }
                    return self.run_skypix_sequence(sequence[0], &sequence[1..], buf, caret);
                }
                x => x,
            },
        }
    }

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
    }
}
