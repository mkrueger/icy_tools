use byteorder::ReadBytesExt;
use std::io::Cursor;

use crate::EngineResult;

use super::font::Font;

pub enum StrokeType {
    End,
    MoveTo,
    LineTo,
}

pub struct Stroke {
    pub stype: StrokeType,
    pub x: i32,
    pub y: i32,
}

impl Stroke {
    pub fn load(br: &mut Cursor<&[u8]>) -> EngineResult<Self> {
        let byte1 = br.read_u8()?;
        let byte2 = br.read_u8()?;

        let flag1 = (byte1 & 0x80) != 0;
        let flag2 = (byte2 & 0x80) != 0;

        let x = if (byte1 & 0x40) != 0 {
            -((!byte1 & 0x3F) as i32) - 1
        } else {
            (byte1 & 0x3F) as i32
        };

        let y = if (byte2 & 0x40) != 0 {
            -((!byte2 & 0x3F) as i32) - 1
        } else {
            (byte2 & 0x3F) as i32
        };

        let stype = if flag1 && flag2 {
            StrokeType::LineTo
        } else if flag1 && !flag2 {
            StrokeType::MoveTo
        } else {
            StrokeType::End
        };

        Ok(Self { stype, x, y })
    }
}

pub const SCALE_UP: [i32; 11] = [1, 6, 2, 3, 1, 4, 5, 2, 5, 3, 4];
pub const SCALE_DOWN: [i32; 11] = [1, 10, 3, 4, 1, 3, 3, 1, 2, 1, 1];

pub struct Character {
    pub strokes: Vec<Stroke>,
    pub width: i32,
}

impl Character {
    /* pub fn new() -> Character {
        Character { strokes: Vec:new(), width: 0 }
    }*/

    pub fn get_width(&self, scale_factor: i32) -> f32 {
        (self.width * SCALE_UP[scale_factor as usize]) as f32 / SCALE_DOWN[scale_factor as usize] as f32
    }

    pub fn draw(&self, bgi: &mut super::Bgi, font: &Font, x: i32, y: i32, dir: super::Direction, size: i32) {
        let size = size as usize;
        let height = font.get_height() * SCALE_UP[size] / SCALE_DOWN[size];
        if matches!(dir, super::Direction::Horizontal) {
            for stroke in &self.strokes {
                let curx = x + (stroke.x * SCALE_UP[size] / SCALE_DOWN[size]);
                let cur_y = y + height - (stroke.y * SCALE_UP[size] / SCALE_DOWN[size]);

                if matches!(stroke.stype, StrokeType::MoveTo) {
                    bgi.move_to(curx, cur_y);
                } else if matches!(stroke.stype, StrokeType::LineTo) {
                    bgi.line_to(curx, cur_y);
                }
            }
        } else {
            for stroke in &self.strokes {
                let curx = x + height - (stroke.y * SCALE_UP[size] / SCALE_DOWN[size]);
                let cur_y = y - (stroke.x * SCALE_UP[size] / SCALE_DOWN[size]);

                if matches!(stroke.stype, StrokeType::MoveTo) {
                    bgi.move_to(curx, cur_y);
                } else if matches!(stroke.stype, StrokeType::LineTo) {
                    bgi.line_to(curx, cur_y);
                }
            }
        }
    }

    pub fn load(br: &mut Cursor<&[u8]>, width: i32) -> EngineResult<Self> {
        let mut strokes = Vec::new();

        loop {
            let stroke = Stroke::load(br)?;
            if matches!(stroke.stype, StrokeType::End) {
                break;
            }
            strokes.push(stroke);
        }

        Ok(Self { strokes, width })
    }
}
