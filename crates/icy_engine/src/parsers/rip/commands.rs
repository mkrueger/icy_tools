use std::{fs, io::Cursor, path, time::UNIX_EPOCH, vec};

use chrono::{DateTime, Datelike, Timelike};

use crate::{CallbackAction, EditableScreen, EngineResult, Position, Size, rip::to_base_36};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::prelude::*;

use super::{
    Command,
    bgi::{Bgi, ButtonStyle2, Direction, FontType, LabelOrientation, MouseField},
    parse_base_36,
};

#[derive(Default, Clone, Debug)]
pub struct TextWindow {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub wrap: bool,
    pub size: i32,
}

impl Command for TextWindow {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }
            6 | 7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            8 => {
                self.wrap = ch == '1';
                Ok(true)
            }

            9 => {
                self.size = ch.to_digit(36).unwrap() as i32;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let (x, y) = match self.size {
            1 => (7, 8),
            2 => (8, 14),
            3 => (7, 14),
            4 => (16, 14),
            _ => (8, 8),
        };
        if self.x0 == 0 && self.y0 == 0 && self.x1 == 0 && self.y1 == 0 && self.size == 0 && !self.wrap {
            bgi.suspend_text = !bgi.suspend_text;
        }
        buf.terminal_state_mut().set_text_window(self.x0, self.y0, self.x1, self.y1);
        bgi.set_text_window(self.x0 * x, self.y0 * y, self.x1 * x, self.y1 * y, self.wrap);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|w{}{}{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1),
            i32::from(self.wrap),
            to_base_36(1, self.size),
        )
    }
}

#[derive(Default, Clone)]
pub struct ViewPort {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Command for ViewPort {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }
            6 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_viewport(self.x0, self.y0, self.x1, self.y1);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|v{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1)
        )
    }
}

#[derive(Default, Clone)]
pub struct ResetWindows {}

impl Command for ResetWindows {
    fn to_rip_string(&self) -> String {
        "|*".to_string()
    }
    fn run(&self, buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        buf.terminal_state_mut().clear_text_window();
        buf.clear_screen();
        bgi.clear_text_window();

        bgi.graph_defaults();
        Ok(CallbackAction::NoUpdate)
    }
}

#[derive(Default, Clone)]
pub struct EraseWindow {}

impl Command for EraseWindow {
    fn to_rip_string(&self) -> String {
        "|e".to_string()
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.clear_text_window();
        Ok(CallbackAction::Update)
    }
}

#[derive(Default, Clone)]
pub struct EraseView {}

impl Command for EraseView {
    fn to_rip_string(&self) -> String {
        "|E".to_string()
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.clear_viewport();
        Ok(CallbackAction::Update)
    }
}

#[derive(Default, Clone)]
pub struct GotoXY {
    pub x: i32,
    pub y: i32,
}

impl Command for GotoXY {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }
            3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.move_to(self.x, self.y);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!("|g{}{}", to_base_36(2, self.x), to_base_36(2, self.y))
    }
}

#[derive(Default, Clone)]
pub struct Home {}

impl Command for Home {
    fn to_rip_string(&self) -> String {
        "|H".to_string()
    }
}

#[derive(Default, Clone)]
pub struct EraseEOL {}

impl Command for EraseEOL {
    fn to_rip_string(&self) -> String {
        "|>".to_string()
    }
}

#[derive(Default, Clone)]
pub struct Color {
    pub c: i32,
}

impl Command for Color {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 => {
                parse_base_36(&mut self.c, ch)?;
                Ok(true)
            }
            1 => {
                parse_base_36(&mut self.c, ch)?;
                Ok(false)
            }
            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_color(self.c as u8);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!("|c{}", to_base_36(2, self.c))
    }
}

#[derive(Default, Clone)]
pub struct SetPalette {
    pub palette: Vec<i32>,
}

impl Command for SetPalette {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        if *state % 2 == 0 {
            self.palette.push(0);
        }
        let mut c = self.palette.pop().unwrap();
        parse_base_36(&mut c, ch)?;
        self.palette.push(c);

        Ok(*state < 31)
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_palette(&self.palette);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        let mut res = String::from("|Q");
        for c in &self.palette {
            res.push_str(to_base_36(2, *c).as_str());
        }
        res
    }
}

#[derive(Default, Clone)]
pub struct OnePalette {
    pub color: i32,
    pub value: i32,
}

impl Command for OnePalette {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.color, ch)?;
                Ok(true)
            }
            2 => {
                parse_base_36(&mut self.value, ch)?;
                Ok(true)
            }
            3 => {
                parse_base_36(&mut self.value, ch)?;
                Ok(false)
            }
            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_palette_color(self.color, self.value as u8);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!("|a{}{}", to_base_36(2, self.color), to_base_36(2, self.value))
    }
}

#[derive(Default, Clone)]
pub struct WriteMode {
    pub mode: i32,
}

impl Command for WriteMode {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 => {
                parse_base_36(&mut self.mode, ch)?;
                Ok(true)
            }
            1 => {
                parse_base_36(&mut self.mode, ch)?;
                Ok(false)
            }
            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_write_mode(super::bgi::WriteMode::from(self.mode as u8));
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!("|W{}", to_base_36(2, self.mode))
    }
}

#[derive(Default, Clone)]
pub struct Move {
    pub x: i32,
    pub y: i32,
}

impl Command for Move {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }
            3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(false)
            }
            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.move_to(self.x, self.y);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!("|m{}{}", to_base_36(2, self.x), to_base_36(2, self.y))
    }
}

#[derive(Default, Clone)]
pub struct Text {
    pub str: String,
}

impl Command for Text {
    fn parse(&mut self, _state: &mut i32, ch: char) -> EngineResult<bool> {
        self.str.push(ch);
        Ok(true)
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.out_text(&self.str);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!("|T{}", self.str)
    }
}

#[derive(Default, Clone)]
pub struct TextXY {
    pub x: i32,
    pub y: i32,
    pub str: String,
}

impl Command for TextXY {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }
            _ => {
                self.str.push(ch);
                Ok(true)
            }
        }
    }
    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.out_text_xy(self.x, self.y, &self.str);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!("|@{}{}{}", to_base_36(2, self.x), to_base_36(2, self.y), self.str)
    }
}

#[derive(Default, Clone)]
pub struct FontStyle {
    pub font: i32,
    pub direction: i32,
    pub size: i32,
    pub res: i32,
}

impl Command for FontStyle {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.font, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.direction, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.size, ch)?;
                Ok(true)
            }
            6 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }

            7 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_text_style(FontType::from(self.font as u8), Direction::from(self.direction as u8), self.size);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|Y{}{}{}{}",
            to_base_36(2, self.font),
            to_base_36(2, self.direction),
            to_base_36(2, self.size),
            to_base_36(2, self.res)
        )
    }
}

#[derive(Default, Clone)]
pub struct Pixel {
    pub x: i32,
    pub y: i32,
}

impl Command for Pixel {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }
            3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(false)
            }
            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.put_pixel(self.x, self.y, bgi.get_color());
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!("|X{}{}", to_base_36(2, self.x), to_base_36(2, self.y))
    }
}

#[derive(Default, Clone)]
pub struct Line {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Command for Line {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }
            6 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.line(self.x0, self.y0, self.x1, self.y1);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|L{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1)
        )
    }
}

#[derive(Default, Clone)]
pub struct Rectangle {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Command for Rectangle {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }
            6 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.rectangle(self.x0, self.y0, self.x1, self.y1);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|R{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1)
        )
    }
}

#[derive(Default, Clone)]
pub struct Bar {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Command for Bar {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }
            6 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let (left, right) = if self.x0 < self.x1 { (self.x0, self.x1) } else { (self.x1, self.x0) };

        let (top, bottom) = if self.y0 < self.y1 { (self.y0, self.y1) } else { (self.y1, self.y0) };

        bgi.bar(left, top, right, bottom);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|B{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1)
        )
    }
}

#[derive(Default, Clone)]
pub struct Circle {
    pub x_center: i32,
    pub y_center: i32,
    pub radius: i32,
}

impl Command for Circle {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x_center, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y_center, ch)?;
                Ok(true)
            }

            4 => {
                parse_base_36(&mut self.radius, ch)?;
                Ok(true)
            }

            5 => {
                parse_base_36(&mut self.radius, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.circle(self.x_center, self.y_center, self.radius);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|C{}{}{}",
            to_base_36(2, self.x_center),
            to_base_36(2, self.y_center),
            to_base_36(2, self.radius)
        )
    }
}

#[derive(Default, Clone)]
pub struct Oval {
    pub x: i32,
    pub y: i32,
    pub st_ang: i32,
    pub end_ang: i32,
    pub x_rad: i32,
    pub y_rad: i32,
}

impl Command for Oval {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.st_ang, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.end_ang, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.x_rad, ch)?;
                Ok(true)
            }

            10 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(true)
            }

            11 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.ellipse(self.x, self.y, self.st_ang, self.end_ang, self.x_rad, self.y_rad);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|O{}{}{}{}{}{}",
            to_base_36(2, self.x),
            to_base_36(2, self.y),
            to_base_36(2, self.st_ang),
            to_base_36(2, self.end_ang),
            to_base_36(2, self.x_rad),
            to_base_36(2, self.y_rad)
        )
    }
}

#[derive(Default, Clone)]
pub struct FilledOval {
    pub x_center: i32,
    pub y_center: i32,
    pub x_rad: i32,
    pub y_rad: i32,
}

impl Command for FilledOval {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x_center, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y_center, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x_rad, ch)?;
                Ok(true)
            }
            6 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(true)
            }

            7 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.fill_ellipse(self.x_center, self.y_center, 0, 360, self.x_rad, self.y_rad);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|o{}{}{}{}",
            to_base_36(2, self.x_center),
            to_base_36(2, self.y_center),
            to_base_36(2, self.x_rad),
            to_base_36(2, self.y_rad)
        )
    }
}

#[derive(Default, Clone)]
pub struct Arc {
    pub x: i32,
    pub y: i32,
    pub start_ang: i32,
    pub end_ang: i32,
    pub radius: i32,
}

impl Command for Arc {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.start_ang, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.end_ang, ch)?;
                Ok(true)
            }

            8 => {
                parse_base_36(&mut self.radius, ch)?;
                Ok(true)
            }

            9 => {
                parse_base_36(&mut self.radius, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.arc(self.x, self.y, self.start_ang, self.end_ang, self.radius);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|A{}{}{}{}{}",
            to_base_36(2, self.x),
            to_base_36(2, self.y),
            to_base_36(2, self.start_ang),
            to_base_36(2, self.end_ang),
            to_base_36(2, self.radius)
        )
    }
}

#[derive(Default, Clone)]
pub struct OvalArc {
    pub x: i32,
    pub y: i32,
    pub start_ang: i32,
    pub end_ang: i32,
    pub x_rad: i32,
    pub y_rad: i32,
}

impl Command for OvalArc {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.start_ang, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.end_ang, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.x_rad, ch)?;
                Ok(true)
            }

            10 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(true)
            }

            11 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.ellipse(self.x, self.y, self.start_ang, self.end_ang, self.x_rad, self.y_rad);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|V{}{}{}{}{}{}",
            to_base_36(2, self.x),
            to_base_36(2, self.y),
            to_base_36(2, self.start_ang),
            to_base_36(2, self.end_ang),
            to_base_36(2, self.x_rad),
            to_base_36(2, self.y_rad)
        )
    }
}

#[derive(Default, Clone)]
pub struct PieSlice {
    pub x: i32,
    pub y: i32,
    pub start_ang: i32,
    pub end_ang: i32,
    pub radius: i32,
}

impl Command for PieSlice {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.start_ang, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.end_ang, ch)?;
                Ok(true)
            }

            8 => {
                parse_base_36(&mut self.radius, ch)?;
                Ok(true)
            }

            9 => {
                parse_base_36(&mut self.radius, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.pie_slice(self.x, self.y, self.start_ang, self.end_ang, self.radius);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|I{}{}{}{}{}",
            to_base_36(2, self.x),
            to_base_36(2, self.y),
            to_base_36(2, self.start_ang),
            to_base_36(2, self.end_ang),
            to_base_36(2, self.radius)
        )
    }
}

#[derive(Default, Clone)]
pub struct OvalPieSlice {
    pub x: i32,
    pub y: i32,
    pub st_ang: i32,
    pub end_ang: i32,
    pub x_rad: i32,
    pub y_rad: i32,
}

impl Command for OvalPieSlice {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.st_ang, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.end_ang, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.x_rad, ch)?;
                Ok(true)
            }

            10 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(true)
            }

            11 => {
                parse_base_36(&mut self.y_rad, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.sector(self.x, self.y, self.st_ang, self.end_ang, self.x_rad, self.y_rad);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|i{}{}{}{}{}{}",
            to_base_36(2, self.x),
            to_base_36(2, self.y),
            to_base_36(2, self.st_ang),
            to_base_36(2, self.end_ang),
            to_base_36(2, self.x_rad),
            to_base_36(2, self.y_rad)
        )
    }
}

#[derive(Default, Clone)]
pub struct Bezier {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub x3: i32,
    pub y3: i32,
    pub x4: i32,
    pub y4: i32,
    pub cnt: i32,
}

impl Command for Bezier {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x2, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.y2, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.x3, ch)?;
                Ok(true)
            }

            10 | 11 => {
                parse_base_36(&mut self.y3, ch)?;
                Ok(true)
            }

            12 | 13 => {
                parse_base_36(&mut self.x4, ch)?;
                Ok(true)
            }

            14 | 15 => {
                parse_base_36(&mut self.y4, ch)?;
                Ok(true)
            }

            16 => {
                parse_base_36(&mut self.cnt, ch)?;
                Ok(true)
            }

            17 => {
                parse_base_36(&mut self.cnt, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.rip_bezier(self.x1, self.y1, self.x2, self.y2, self.x3, self.y3, self.x4, self.y4, self.cnt);
        /*
                let points = vec![
                    Position::new(self.x1, self.y1),
                    Position::new(self.x2, self.y2),
                    Position::new(self.x3, self.y3),
                    Position::new(self.x4, self.y4),
                ];
                bgi.draw_bezier(points.len() as i32, &points, self.cnt);
        */
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|Z{}{}{}{}{}{}{}{}{}",
            to_base_36(2, self.x1),
            to_base_36(2, self.y1),
            to_base_36(2, self.x2),
            to_base_36(2, self.y2),
            to_base_36(2, self.x3),
            to_base_36(2, self.y3),
            to_base_36(2, self.x4),
            to_base_36(2, self.y4),
            to_base_36(2, self.cnt)
        )
    }
}

#[derive(Default, Clone)]
pub struct Polygon {
    pub points: Vec<i32>,
    pub npoints: i32,
}

impl Command for Polygon {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.npoints, ch)?;
                Ok(true)
            }
            _ => {
                if *state % 2 == 0 {
                    self.points.push(0);
                }
                let mut p = self.points.pop().unwrap();
                parse_base_36(&mut p, ch)?;
                self.points.push(p);

                Ok(*state < (self.npoints + 1) * 4)
            }
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let mut points = Vec::new();
        for i in 0..self.points.len() / 2 {
            points.push(Position::new(self.points[i * 2], self.points[i * 2 + 1]));
        }
        bgi.draw_poly(&points);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        let mut res = String::from("|P");
        res.push_str(to_base_36(2, self.points.len() as i32 / 2).as_str());
        for p in &self.points {
            res.push_str(to_base_36(2, *p).as_str());
        }
        res
    }
}

#[derive(Default, Clone)]
pub struct FilledPolygon {
    pub points: Vec<i32>,
    pub npoints: i32,
}

impl Command for FilledPolygon {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.npoints, ch)?;
                Ok(true)
            }
            _ => {
                if *state % 2 == 0 {
                    self.points.push(0);
                }
                let mut p = self.points.pop().unwrap();
                parse_base_36(&mut p, ch)?;
                self.points.push(p);

                Ok(*state < (self.npoints + 1) * 4)
            }
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let mut points = Vec::new();
        for i in 0..self.points.len() / 2 {
            points.push(Position::new(self.points[i * 2], self.points[i * 2 + 1]));
        }
        bgi.fill_poly(&points);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        let mut res = String::from("|p");
        res.push_str(to_base_36(2, self.points.len() as i32 / 2).as_str());
        for p in &self.points {
            res.push_str(to_base_36(2, *p).as_str());
        }
        res
    }
}

#[derive(Default, Clone)]
pub struct PolyLine {
    pub points: Vec<i32>,
    pub npoints: i32,
}

impl Command for PolyLine {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.npoints, ch)?;
                Ok(true)
            }
            _ => {
                if *state % 2 == 0 {
                    self.points.push(0);
                }
                let mut p = self.points.pop().unwrap();
                parse_base_36(&mut p, ch)?;
                self.points.push(p);

                Ok(*state < (self.npoints + 1) * 4)
            }
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let mut points = Vec::new();
        for i in 0..self.points.len() / 2 {
            points.push(Position::new(self.points[i * 2], self.points[i * 2 + 1]));
        }
        bgi.draw_poly_line(&points);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        let mut res = String::from("|l");
        res.push_str(to_base_36(2, self.points.len() as i32 / 2).as_str());
        for p in &self.points {
            res.push_str(to_base_36(2, *p).as_str());
        }
        res
    }
}

#[derive(Default, Clone)]
pub struct Fill {
    pub x: i32,
    pub y: i32,
    pub border: i32,
}

impl Command for Fill {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 => {
                parse_base_36(&mut self.border, ch)?;
                Ok(true)
            }

            5 => {
                parse_base_36(&mut self.border, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.flood_fill(self.x, self.y, self.border as u8);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!("|F{}{}{}", to_base_36(2, self.x), to_base_36(2, self.y), to_base_36(2, self.border))
    }
}

#[derive(Default, Clone)]
pub struct LineStyle {
    pub style: i32,
    pub user_pat: i32,
    pub thick: i32,
}

impl Command for LineStyle {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.style, ch)?;
                Ok(true)
            }
            2..=5 => {
                parse_base_36(&mut self.user_pat, ch)?;
                Ok(true)
            }
            6 => {
                parse_base_36(&mut self.thick, ch)?;
                Ok(true)
            }

            7 => {
                parse_base_36(&mut self.thick, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_line_style(super::bgi::LineStyle::from(self.style as u8));
        //  If the <style> parameter is not 4, then the <user_pat> parameter is ignored.
        if self.style == 4 {
            bgi.set_line_pattern(self.user_pat);
        }
        bgi.set_line_thickness(self.thick);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!("|={}{}{}", to_base_36(2, self.style), to_base_36(4, self.user_pat), to_base_36(2, self.thick))
    }
}

#[derive(Default, Clone)]
pub struct FillStyle {
    pub pattern: i32,
    pub color: i32,
}

impl Command for FillStyle {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.pattern, ch)?;
                Ok(true)
            }
            2 => {
                parse_base_36(&mut self.color, ch)?;
                Ok(true)
            }
            3 => {
                parse_base_36(&mut self.color, ch)?;
                Ok(false)
            }
            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_fill_style(super::bgi::FillStyle::from(self.pattern as u8));
        bgi.set_fill_color(self.color as u8);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!("|S{}{}", to_base_36(2, self.pattern), to_base_36(2, self.color))
    }
}

#[derive(Default, Clone)]
pub struct FillPattern {
    pub c1: i32,
    pub c2: i32,
    pub c3: i32,
    pub c4: i32,
    pub c5: i32,
    pub c6: i32,
    pub c7: i32,
    pub c8: i32,
    pub col: i32,
}

impl Command for FillPattern {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.c1, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.c2, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.c3, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.c4, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.c5, ch)?;
                Ok(true)
            }

            10 | 11 => {
                parse_base_36(&mut self.c6, ch)?;
                Ok(true)
            }

            12 | 13 => {
                parse_base_36(&mut self.c7, ch)?;
                Ok(true)
            }

            14 | 15 => {
                parse_base_36(&mut self.c8, ch)?;
                Ok(true)
            }

            16 => {
                parse_base_36(&mut self.col, ch)?;
                Ok(true)
            }

            17 => {
                parse_base_36(&mut self.col, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_user_fill_pattern(&[
            self.c1 as u8,
            self.c2 as u8,
            self.c3 as u8,
            self.c4 as u8,
            self.c5 as u8,
            self.c6 as u8,
            self.c7 as u8,
            self.c8 as u8,
        ]);
        bgi.set_fill_style(super::bgi::FillStyle::User);
        bgi.set_fill_color(self.col as u8);
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|s{}{}{}{}{}{}{}{}{}",
            to_base_36(2, self.c1),
            to_base_36(2, self.c2),
            to_base_36(2, self.c3),
            to_base_36(2, self.c4),
            to_base_36(2, self.c5),
            to_base_36(2, self.c6),
            to_base_36(2, self.c7),
            to_base_36(2, self.c8),
            to_base_36(2, self.col)
        )
    }
}

#[derive(Default, Clone)]
pub struct Mouse {
    pub num: i32,
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub clk: i32,
    pub clr: i32,
    pub res: i32,
    pub text: String,
}

impl Command for Mouse {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.num, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            10 => {
                parse_base_36(&mut self.clk, ch)?;
                Ok(true)
            }

            11 => {
                parse_base_36(&mut self.clr, ch)?;
                Ok(true)
            }

            12..=16 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }

            _ => {
                self.text.push(ch);
                Ok(true)
            }
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let host_command = parse_host_command(&self.text);
        let mut style = ButtonStyle2::default();
        style.flags |= 1024;
        bgi.add_mouse_field(MouseField::new(self.x0, self.y0, self.x1, self.y1, Some(host_command), style));
        Ok(CallbackAction::NoUpdate)
    }
    fn to_rip_string(&self) -> String {
        format!(
            "|1M{}{}{}{}{}{}{}{}{}",
            to_base_36(2, self.num),
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1),
            to_base_36(1, self.clk),
            to_base_36(1, self.clr),
            to_base_36(5, self.res),
            self.text
        )
    }
}

#[derive(Default, Clone)]
pub struct MouseFields {}

impl Command for MouseFields {
    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.clear_mouse_fields();
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        "|1K".to_string()
    }
}

#[derive(Default, Clone)]
pub struct BeginText {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub res: i32,
}

impl Command for BeginText {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x2, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.y2, ch)?;
                Ok(true)
            }

            8 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }

            9 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, _bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        // Nothing?
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|1T{}{}{}{}{}",
            to_base_36(2, self.x1),
            to_base_36(2, self.y1),
            to_base_36(2, self.x2),
            to_base_36(2, self.y2),
            to_base_36(2, self.res)
        )
    }
}

#[derive(Default, Clone)]
pub struct RegionText {
    pub justify: bool,
    pub str: String,
}

impl Command for RegionText {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        if *state == 0 {
            self.justify = ch == '1';
        } else {
            self.str.push(ch);
        }
        Ok(true)
    }

    fn to_rip_string(&self) -> String {
        format!("|1t{}{}", i32::from(self.justify), self.str)
    }
}

#[derive(Default, Clone)]
pub struct EndText {}

impl Command for EndText {
    fn to_rip_string(&self) -> String {
        "|1E".to_string()
    }
    fn run(&self, _buf: &mut dyn EditableScreen, _bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        // Nothing
        Ok(CallbackAction::NoUpdate)
    }
}

#[derive(Default, Clone)]
pub struct GetImage {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub res: i32,
}

impl Command for GetImage {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            8 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.rip_image = Some(bgi.get_image(self.x0, self.y0, self.x1, self.y1));
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|1C{}{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1),
            to_base_36(1, self.res)
        )
    }
}

#[derive(Default, Clone)]
pub struct PutImage {
    pub x: i32,
    pub y: i32,
    pub mode: i32,
    pub res: i32,
}

impl Command for PutImage {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.mode, ch)?;
                Ok(true)
            }

            6 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.put_rip_image(self.x, self.y, super::bgi::WriteMode::from(self.mode as u8));
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|1P{}{}{}{}",
            to_base_36(2, self.x),
            to_base_36(2, self.y),
            to_base_36(2, self.mode),
            to_base_36(1, self.res)
        )
    }
}

#[derive(Default, Clone)]
pub struct WriteIcon {
    pub res: char,
    pub str: String,
}

impl Command for WriteIcon {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        if *state == 0 {
            self.res = ch;
        } else {
            self.str.push(ch);
        }
        Ok(true)
    }

    fn to_rip_string(&self) -> String {
        format!("|1W{}{}", self.res, self.str)
    }
}

#[derive(Default, Clone)]
pub struct LoadIcon {
    pub x: i32,
    pub y: i32,
    pub mode: i32,
    pub clipboard: i32,
    pub res: i32,
    pub file_name: String,
}

impl Command for LoadIcon {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.mode, ch)?;
                Ok(true)
            }

            6 => {
                parse_base_36(&mut self.clipboard, ch)?;
                Ok(true)
            }

            7 | 8 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }
            _ => {
                self.file_name.push(ch);
                Ok(true)
            }
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let file_name = lookup_cache_file(bgi, &self.file_name)?;
        if !file_name.exists() {
            log::error!("File not found: {}", self.file_name);
            return Ok(CallbackAction::NoUpdate);
        }
        let mut file = std::fs::File::open(file_name)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        let _len = buf.len();
        let mut br = Cursor::new(buf);

        let width = br.read_u16::<LittleEndian>()? as i32 + 1;
        let height = br.read_u16::<LittleEndian>()? as i32 + 1;

        // let _tmp = br.read_u16::<LittleEndian>()? + 1;

        /*
        00    Paste the image on-screen normally                   (COPY)
        01    Exclusive-OR  image with the one already on screen   (XOR)
        02    Logically OR  image with the one already on screen   (OR)
        03    Logically AND image with the one already on screen   (AND)
        04    Paste the inverse of the image on the screen         (NOT)
        */
        let mode = match self.mode {
            // 0 => bgi.set_write_mode(super::bgi::WriteMode::Copy),
            1 => bgi.set_write_mode(super::bgi::WriteMode::Xor),
            2 => bgi.set_write_mode(super::bgi::WriteMode::Or),
            3 => bgi.set_write_mode(super::bgi::WriteMode::And),
            4 => bgi.set_write_mode(super::bgi::WriteMode::Not),
            _ => bgi.set_write_mode(super::bgi::WriteMode::Copy),
        };
        for y in 0..height {
            if self.y + y >= bgi.window.height {
                break;
            }
            let row = (width / 8 + i32::from((width & 7) != 0)) as usize;
            let mut planes = vec![0u8; row * 4];
            br.read_exact(&mut planes)?;

            for x in 0..width as usize {
                if self.x + x as i32 >= bgi.window.width {
                    break;
                }
                let mut color = (planes[(row * 3) + (x / 8)] >> (7 - (x & 7))) & 1;
                color |= ((planes[(row * 2) + (x / 8)] >> (7 - (x & 7))) & 1) << 1;
                color |= ((planes[row + (x / 8)] >> (7 - (x & 7))) & 1) << 2;
                color |= ((planes[x / 8] >> (7 - (x & 7))) & 1) << 3;
                bgi.put_pixel(self.x + x as i32, self.y + y, color);
            }
        }
        bgi.set_write_mode(mode);
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|1I{}{}{}{}{}{}",
            to_base_36(2, self.x),
            to_base_36(2, self.y),
            to_base_36(2, self.mode),
            to_base_36(1, self.clipboard),
            to_base_36(2, self.res),
            self.file_name
        )
    }
}

#[derive(Default, Clone)]
pub struct ButtonStyle {
    pub wid: i32,
    pub hgt: i32,
    pub orient: i32,
    pub flags: i32,
    pub bevsize: i32,
    pub dfore: i32,
    pub dback: i32,
    pub bright: i32,
    pub dark: i32,

    pub surface: i32,
    pub grp_no: i32,
    pub flags2: i32,
    pub uline_col: i32,
    pub corner_col: i32,
    pub res: i32,
}

impl Command for ButtonStyle {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.wid, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.hgt, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.orient, ch)?;
                Ok(true)
            }

            6..=9 => {
                parse_base_36(&mut self.flags, ch)?;
                Ok(true)
            }

            10 | 11 => {
                parse_base_36(&mut self.bevsize, ch)?;
                Ok(true)
            }

            12 | 13 => {
                parse_base_36(&mut self.dfore, ch)?;
                Ok(true)
            }

            14 | 15 => {
                parse_base_36(&mut self.dback, ch)?;
                Ok(true)
            }

            16 | 17 => {
                parse_base_36(&mut self.bright, ch)?;
                Ok(true)
            }

            18 | 19 => {
                parse_base_36(&mut self.dark, ch)?;
                Ok(true)
            }

            20 | 21 => {
                parse_base_36(&mut self.surface, ch)?;
                Ok(true)
            }

            22 | 23 => {
                parse_base_36(&mut self.grp_no, ch)?;
                Ok(true)
            }

            24 | 25 => {
                parse_base_36(&mut self.flags2, ch)?;
                Ok(true)
            }

            26 | 27 => {
                parse_base_36(&mut self.uline_col, ch)?;
                Ok(true)
            }

            28 | 29 => {
                parse_base_36(&mut self.corner_col, ch)?;
                Ok(true)
            }
            30..=36 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(*state < 36)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        bgi.set_button_style(ButtonStyle2 {
            size: Size::new(self.wid, self.hgt),
            orientation: LabelOrientation::from(self.orient as u8),
            flags: self.flags,
            bevel_size: self.bevsize,
            label_color: self.dfore,
            drop_shadow_color: self.dback,
            bright: self.bright,
            dark: self.dark,
            surface_color: self.surface,
            group: self.grp_no,
            flags2: self.flags2,
            underline_color: self.uline_col,
            corner_color: self.corner_col,
        });
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|1B{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            to_base_36(2, self.wid),
            to_base_36(2, self.hgt),
            to_base_36(2, self.orient),
            to_base_36(4, self.flags),
            to_base_36(2, self.bevsize),
            to_base_36(2, self.dfore),
            to_base_36(2, self.dback),
            to_base_36(2, self.bright),
            to_base_36(2, self.dark),
            to_base_36(2, self.surface),
            to_base_36(2, self.grp_no),
            to_base_36(2, self.flags2),
            to_base_36(2, self.uline_col),
            to_base_36(2, self.corner_col),
            to_base_36(6, self.res)
        )
    }
}

#[derive(Default, Clone)]
pub struct Button {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub hotkey: i32,
    pub flags: i32,
    pub res: i32,
    pub text: String,
}

impl Command for Button {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.hotkey, ch)?;
                Ok(true)
            }

            10 => {
                parse_base_36(&mut self.flags, ch)?;
                Ok(true)
            }

            11 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }

            _ => {
                self.text.push(ch);
                Ok(true)
            }
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let split: Vec<&str> = self.text.split("<>").collect();

        if split.len() == 4 {
            bgi.add_button(
                self.x0,
                self.y0,
                self.x1,
                self.y1,
                self.hotkey as u8,
                self.flags,
                Some(split[0]),
                split[1],
                Some(parse_host_command(split[2])),
                false,
            );
        } else if split.len() == 3 {
            bgi.add_button(
                self.x0,
                self.y0,
                self.x1,
                self.y1,
                self.hotkey as u8,
                self.flags,
                None,
                split[1],
                Some(parse_host_command(split[2])),
                false,
            );
        } else if split.len() == 2 {
            bgi.add_button(self.x0, self.y0, self.x1, self.y1, self.hotkey as u8, self.flags, None, split[1], None, false);
        } else {
            bgi.add_button(
                self.x0,
                self.y0,
                self.x1,
                self.y1,
                self.hotkey as u8,
                self.flags,
                None,
                &format!("error in text {}", split.len()),
                None,
                false,
            );
        }

        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|1U{}{}{}{}{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1),
            to_base_36(2, self.hotkey),
            to_base_36(1, self.flags),
            to_base_36(1, self.res),
            self.text
        )
    }
}

fn parse_host_command(split: &str) -> String {
    let mut res = String::new();
    let mut got_caret = false;
    for c in split.chars() {
        if got_caret {
            match c {
                // Null (ASCII 0)
                '@' => res.push('\x00'),
                // Beep
                'G' => res.push('\x07'),
                // Clear Screen (Top of Form)
                'L' => res.push('\x0C'),
                // Carriage Return
                'M' => res.push('\x0D'),
                // Break (sometimes)
                'C' => res.push('\x18'),
                // Backspace
                'H' => res.push('\x08'),
                // Escape character
                '[' => res.push('\x1B'),
                // Pause data transmission
                'S' => res.push('1'),
                // Resume data transmission
                'Q' => res.push('2'),
                _ => {
                    log::error!("Invalid character after ^ in button command: {}", c);
                }
            }
            got_caret = false;
            continue;
        }
        if c == '^' {
            got_caret = true;
            continue;
        }
        res.push(c);
    }
    res
}

#[derive(Default, Clone)]
pub struct Define {
    pub flags: i32,
    pub res: i32,
    pub text: String,
}

impl Command for Define {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0..=2 => {
                parse_base_36(&mut self.flags, ch)?;
                Ok(true)
            }
            3 | 4 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }
            _ => {
                self.text.push(ch);
                Ok(true)
            }
        }
    }

    fn to_rip_string(&self) -> String {
        format!("|1D{}{}{}", to_base_36(3, self.flags), to_base_36(2, self.res), self.text)
    }
}

#[derive(Default, Clone)]
pub struct Query {
    pub mode: i32,
    pub res: i32,
    pub text: String,
}

impl Command for Query {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 => {
                parse_base_36(&mut self.mode, ch)?;
                Ok(true)
            }
            1..=3 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }
            _ => {
                self.text.push(ch);
                Ok(true)
            }
        }
    }

    fn to_rip_string(&self) -> String {
        format!("|1\x1B{}{}{}", to_base_36(1, self.mode), to_base_36(3, self.res), self.text)
    }
}

#[derive(Default, Clone)]
pub struct CopyRegion {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub res: i32,
    pub dest_line: i32,
}

impl Command for CopyRegion {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.x0, ch)?;
                Ok(true)
            }
            2 | 3 => {
                parse_base_36(&mut self.y0, ch)?;
                Ok(true)
            }

            4 | 5 => {
                parse_base_36(&mut self.x1, ch)?;
                Ok(true)
            }

            6 | 7 => {
                parse_base_36(&mut self.y1, ch)?;
                Ok(true)
            }

            8 | 9 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }

            10 => {
                parse_base_36(&mut self.dest_line, ch)?;
                Ok(true)
            }

            11 => {
                parse_base_36(&mut self.dest_line, ch)?;
                Ok(false)
            }

            _ => Err(anyhow::Error::msg("Invalid state")),
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let image = bgi.get_image(self.x0, self.y0, self.x1, self.y1 + 1);
        bgi.put_image(self.x0, self.dest_line, &image, bgi.get_write_mode());
        Ok(CallbackAction::Update)
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|1G{}{}{}{}{}{}",
            to_base_36(2, self.x0),
            to_base_36(2, self.y0),
            to_base_36(2, self.x1),
            to_base_36(2, self.y1),
            to_base_36(2, self.res),
            to_base_36(2, self.dest_line)
        )
    }
}

#[derive(Default, Clone)]
pub struct ReadScene {
    pub res: String,
    pub str: String,
}

impl Command for ReadScene {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        if (0..=7).contains(state) {
            self.res.push(ch);
        } else {
            self.str.push(ch);
        }
        Ok(true)
    }

    fn to_rip_string(&self) -> String {
        format!("|1R{}{}", self.res, self.str)
    }
}
#[derive(Default, Clone)]
pub struct FileQuery {
    pub mode: i32,
    pub res: i32,
    pub file_name: String,
}

impl Command for FileQuery {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 | 1 => {
                parse_base_36(&mut self.mode, ch)?;
                Ok(true)
            }
            2..=5 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }
            _ => {
                self.file_name.push(ch);
                Ok(true)
            }
        }
    }

    fn run(&self, _buf: &mut dyn EditableScreen, bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        let file_name = lookup_cache_file(bgi, &self.file_name)?;
        match self.mode {
            // Simply query the existence of the file.  If it exists, a "1" is
            // returned.  Otherwise a "0" is returned to the Host (without a
            // carriage return).
            0 => {
                if file_name.exists() {
                    return Ok(CallbackAction::SendString("1".to_string()));
                }
                return Ok(CallbackAction::SendString("0".to_string()));
            }

            // Same as 0, except a carriage return is added after the response.
            1 => {
                if file_name.exists() {
                    return Ok(CallbackAction::SendString("1\r\n".to_string()));
                }
                return Ok(CallbackAction::SendString("0\r\n".to_string()));
            }

            // Queries the existence of a file.  If it does not exist, a "0" is
            // returned to the Host followed by a carriage return.  If it does
            // exist, the returned text is a "1." followed by the file size (in
            // decimal).  The return sequence is terminated by a carriage
            // return.  An example of the returned text could be "1.20345".
            2 => {
                if let Ok(data) = fs::metadata(file_name) {
                    return Ok(CallbackAction::SendString(format!("1.{}\r\n", data.len())));
                }
                return Ok(CallbackAction::SendString("0\r\n".to_string()));
            }
            // Queries extended return information.  If the file does not
            // exist, a "0" is returned followed by a carriage return.  If it
            // does exist, the text returned to the Host is in the Format:
            // 1.size.date.time <cr>.  An example of a return statement could
            // be "1.20345.01/02/93.03:04:30<cr>"
            3 => {
                if let Ok(data) = fs::metadata(file_name) {
                    let time = data.modified().unwrap().duration_since(UNIX_EPOCH).unwrap();
                    if let Some(time) = DateTime::from_timestamp(time.as_secs() as i64, 0) {
                        return Ok(CallbackAction::SendString(format!(
                            "1.{}.{:02}.{:02}.{:02}.{:02}:{:02}:{:02}\r\n",
                            data.len(),
                            time.month(),
                            time.day(),
                            time.year(),
                            time.hour(),
                            time.minute(),
                            time.second(),
                        )));
                    }
                    return Ok(CallbackAction::SendString(format!("1.{}.\r\n", data.len())));
                }
                return Ok(CallbackAction::SendString("0\r\n".to_string()));
            }
            // Queries extended return information.  If the file does not
            // exist, a "0" is returned followed by a carriage return.  If it
            // does exist, the text returned to the Host is in the Format:
            // 1.filename.size.date.time <cr>. An example of a return statement
            // could be "1.MYFILE.RIP.20345.01/02/93.03:04:30 <cr>".  Note that
            // the file extension adds another period into the return text.
            4 => {
                if let Ok(data) = fs::metadata(file_name) {
                    let time = data.modified().unwrap().duration_since(UNIX_EPOCH).unwrap();
                    if let Some(time) = DateTime::from_timestamp(time.as_secs() as i64, 0) {
                        return Ok(CallbackAction::SendString(format!(
                            "1.{}.{}.{:02}.{:02}.{:02}.{:02}:{:02}:{:02}\r\n",
                            self.file_name,
                            data.len(),
                            time.month(),
                            time.day(),
                            time.year(),
                            time.hour(),
                            time.minute(),
                            time.second(),
                        )));
                    }
                    return Ok(CallbackAction::SendString(format!("1.{}.{}.\r\n", self.file_name, data.len())));
                }
                return Ok(CallbackAction::SendString("0\r\n".to_string()));
            }
            _ => {
                log::error!("Invalid mode for FileQuery: {}", self.mode);
            }
        }
        Ok(CallbackAction::NoUpdate)
    }

    fn to_rip_string(&self) -> String {
        format!("|1F{}{}{}", to_base_36(2, self.mode), to_base_36(4, self.res), self.file_name)
    }
}

fn lookup_cache_file(bgi: &mut Bgi, search_file: &str) -> EngineResult<path::PathBuf> {
    let mut search_file = search_file.to_uppercase();
    let has_extension = search_file.contains('.');
    if !has_extension {
        search_file.push('.');
    }

    for path in fs::read_dir(&bgi.file_path)?.flatten() {
        if let Some(file_name) = path.file_name().to_str() {
            if has_extension && file_name.to_uppercase() == search_file {
                return Ok(path.path());
            }
            if !has_extension && file_name.to_uppercase().starts_with(&search_file) {
                return Ok(path.path());
            }
        }
    }
    Ok(bgi.file_path.join(&search_file))
}

#[derive(Default, Clone)]
pub struct EnterBlockMode {
    pub mode: i32,
    pub proto: i32,
    pub file_type: i32,
    pub res: i32,
    pub file_name: String,
}

impl Command for EnterBlockMode {
    fn parse(&mut self, state: &mut i32, ch: char) -> EngineResult<bool> {
        match state {
            0 => {
                parse_base_36(&mut self.mode, ch)?;
                Ok(true)
            }
            1 => {
                parse_base_36(&mut self.proto, ch)?;
                Ok(true)
            }

            2 | 3 => {
                parse_base_36(&mut self.file_type, ch)?;
                Ok(true)
            }

            4..=7 => {
                parse_base_36(&mut self.res, ch)?;
                Ok(true)
            }

            _ => {
                self.file_name.push(ch);
                Ok(true)
            }
        }
    }

    fn to_rip_string(&self) -> String {
        format!(
            "|9\x1B{}{}{}{}{}",
            to_base_36(1, self.mode),
            to_base_36(1, self.proto),
            to_base_36(2, self.file_type),
            to_base_36(4, self.res),
            self.file_name
        )
    }
}

#[derive(Default, Clone)]
pub struct TextVariable {
    pub text: String,
}

impl Command for TextVariable {
    fn parse(&mut self, _state: &mut i32, ch: char) -> EngineResult<bool> {
        if ch == '$' {
            return Ok(false);
        }
        self.text.push(ch);
        Ok(true)
    }

    fn to_rip_string(&self) -> String {
        format!("|${}$", self.text)
    }
}
