#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::too_many_lines,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::must_use_candidate,
    clippy::struct_excessive_bools,
    clippy::return_self_not_must_use,
    clippy::field_reassign_with_default
)]
mod text_attribute;
use std::{
    cmp::min,
    ops::{Add, AddAssign, Sub, SubAssign},
};

pub use text_attribute::*;

mod attributed_char;
pub use attributed_char::*;

mod layer;
pub use layer::*;

mod line;
pub use line::*;

mod position;
pub use position::*;

mod buffers;
mod buffers_rendering;
pub use buffers::*;

#[macro_use]
mod palette_handling;
pub use palette_handling::*;

mod fonts;
pub use fonts::*;

pub mod parsers;
pub use parsers::*;

mod caret;
pub use caret::*;

pub mod formats;
pub use formats::*;

mod ansi_font;
pub use ansi_font::*;

mod crc;
pub use crc::*;

mod terminal_state;
pub use terminal_state::*;

mod sixel_mod;
pub use sixel_mod::*;

mod selection;
pub use selection::*;

mod selection_mask;
mod url_scanner;
pub use selection_mask::*;

pub type EngineResult<T> = anyhow::Result<T>;

pub mod editor;

pub mod overlay_mask;
pub mod paint;

use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;

pub mod screen;
pub use screen::*;

pub mod text_screen;
pub use text_screen::*;

#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use once_cell::sync::Lazy;
static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

#[derive(Copy, Clone, Debug, Default)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(width: {}, height: {})", self.width, self.height)
    }
}

impl PartialEq for Size {
    fn eq(&self, other: &Size) -> bool {
        self.width == other.width && self.height == other.height
    }
}

impl Size {
    pub fn new(width: i32, height: i32) -> Self {
        Size { width, height }
    }
}

impl From<(usize, usize)> for Size {
    fn from(value: (usize, usize)) -> Self {
        Size {
            width: value.0 as i32,
            height: value.1 as i32,
        }
    }
}
impl From<(i32, i32)> for Size {
    fn from(value: (i32, i32)) -> Self {
        Size {
            width: value.0,
            height: value.1,
        }
    }
}
impl From<(u32, u32)> for Size {
    fn from(value: (u32, u32)) -> Self {
        Size {
            width: value.0 as i32,
            height: value.1 as i32,
        }
    }
}

impl From<(u16, u16)> for Size {
    fn from(value: (u16, u16)) -> Self {
        Size {
            width: value.0 as i32,
            height: value.1 as i32,
        }
    }
}

impl From<(u8, u8)> for Size {
    fn from(value: (u8, u8)) -> Self {
        Size {
            width: value.0 as i32,
            height: value.1 as i32,
        }
    }
}

impl From<Position> for Size {
    fn from(value: Position) -> Self {
        Size {
            width: value.x,
            height: value.y,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Rectangle {
    pub start: Position,
    pub size: Size,
}
impl std::fmt::Display for Rectangle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(x:{}, y:{}, width: {}, height: {})",
            self.start.x, self.start.y, self.size.width, self.size.height
        )
    }
}

impl Rectangle {
    pub fn new(start: Position, size: Size) -> Self {
        Self { start, size }
    }

    pub fn from(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            start: Position::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn from_coords(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Rectangle {
            start: Position::new(x1.min(x2), y1.min(y2)),
            size: Size::new((x2 - x1).abs(), (y2 - y1).abs()),
        }
    }

    pub fn from_pt(p1: Position, p2: Position) -> Self {
        let start = Position::new(min(p1.x, p2.x), min(p1.y, p2.y));

        Rectangle {
            start,
            size: Size::new((p1.x - p2.x).abs(), (p1.y - p2.y).abs()),
        }
    }

    pub fn top_left(&self) -> Position {
        self.start
    }

    pub fn bottom_right(&self) -> Position {
        Position {
            x: self.start.x + self.size.width,
            y: self.start.y + self.size.height,
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.start.x <= x && x <= self.start.x + self.size.width && self.start.y <= y && y <= self.start.y + self.size.height
    }

    pub fn contains_pt(&self, point: Position) -> bool {
        self.start.x <= point.x && point.x <= self.start.x + self.size.width && self.start.y <= point.y && point.y <= self.start.y + self.size.height
    }

    pub fn contains_rect(&self, other: &Rectangle) -> bool {
        self.contains_pt(other.start) && self.contains_pt(other.bottom_right())
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

    pub fn from_min_size(pos: impl Into<Position>, size: impl Into<Size>) -> Rectangle {
        Rectangle {
            start: pos.into(),
            size: size.into(),
        }
    }

    pub fn intersect(&self, other: &Rectangle) -> Rectangle {
        let min = self.start.max(other.start);
        let max = self.bottom_right().min(other.bottom_right());
        Rectangle {
            start: min,
            size: (max - min).into(),
        }
    }

    pub fn union(&self, other: &Rectangle) -> Rectangle {
        if self.is_empty() {
            return *other;
        }

        if other.is_empty() {
            return *self;
        }

        let min = self.start.min(other.start);
        let max = self.bottom_right().max(other.bottom_right());
        Rectangle {
            start: min,
            size: (max - min).into(),
        }
    }

    pub fn y_range(&self) -> std::ops::Range<i32> {
        self.start.y..self.bottom_right().y
    }

    pub fn x_range(&self) -> std::ops::Range<i32> {
        self.start.x..self.bottom_right().x
    }

    pub fn y_range_inclusive(&self) -> std::ops::RangeInclusive<i32> {
        self.start.y..=self.bottom_right().y
    }

    pub fn x_range_inclusive(&self) -> std::ops::RangeInclusive<i32> {
        self.start.x..=self.bottom_right().x
    }

    pub fn left(&self) -> i32 {
        self.start.x
    }

    pub fn right(&self) -> i32 {
        self.bottom_right().x
    }

    pub fn top(&self) -> i32 {
        self.start.y
    }

    pub fn bottom(&self) -> i32 {
        self.bottom_right().y
    }

    pub fn is_empty(&self) -> bool {
        self.size.width <= 0 || self.size.height <= 0
    }

    pub fn is_inside(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();

        self.start.x <= pos.x && self.start.y <= pos.y && pos.x < self.start.x + self.size.width && pos.y < self.start.y + self.size.height
    }

    pub fn is_inside_inclusive(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();
        self.start.x <= pos.x && self.start.y <= pos.y && pos.x <= self.start.x + self.size.width && pos.y <= self.start.y + self.size.height
    }
}

impl Add<Position> for Rectangle {
    type Output = Rectangle;

    fn add(self, rhs: Position) -> Rectangle {
        Rectangle {
            start: self.start + rhs,
            size: self.size,
        }
    }
}

impl AddAssign<Position> for Rectangle {
    fn add_assign(&mut self, rhs: Position) {
        self.start += rhs;
    }
}

impl Sub<Position> for Rectangle {
    type Output = Rectangle;

    fn sub(self, rhs: Position) -> Rectangle {
        Rectangle {
            start: self.start - rhs,
            size: self.size,
        }
    }
}

impl SubAssign<Position> for Rectangle {
    fn sub_assign(&mut self, rhs: Position) {
        self.start -= rhs;
    }
}

pub trait TextPane {
    fn get_char(&self, pos: Position) -> AttributedChar;
    fn get_line_count(&self) -> i32;
    fn get_width(&self) -> i32;
    fn get_height(&self) -> i32;
    fn get_size(&self) -> Size;
    fn get_line_length(&self, line: i32) -> i32;
    fn get_rectangle(&self) -> Rectangle;

    fn get_string(&self, pos: Position, size: usize) -> String {
        let pos = pos.into();
        let mut result = String::new();
        let mut pos = pos;
        for _ in 0..size {
            result.push(self.get_char(pos).ch);
            pos.x += 1;
            if pos.x >= self.get_width() {
                pos.x = 0;
                pos.y += 1;
            }
        }
        result
    }

    fn is_position_in_range(&self, pos: Position, from: Position, size: i32) -> bool {
        match pos.y.cmp(&from.y) {
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => from.x <= pos.x && pos.x < from.x + size,
            std::cmp::Ordering::Greater => {
                let remainder = size.saturating_sub(self.get_width() + from.x);
                let lines = remainder / self.get_width();
                let mut y = from.y + lines;
                let x = if remainder > 0 {
                    y += 1; // remainder > 1 wraps 1 extra line
                    remainder - lines * self.get_width()
                } else {
                    remainder
                };
                pos.y < y || pos.y == y && pos.x < x
            }
        }
    }
}
