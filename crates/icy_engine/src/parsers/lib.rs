#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::too_many_lines, clippy::cast_lossless, clippy::cast_precision_loss)]
mod text_attribute;
use std::error::Error;

pub use text_attribute::*;

mod attributed_char;
pub use attributed_char::*;

mod layer;
pub use layer::*;

mod line;
pub use line::*;

mod position;
pub use position::*;

mod buffer;
pub use  buffer::*;

mod palette_handling;
pub use palette_handling::*;

mod fonts;
pub use fonts::*;

mod parser;
pub use parser::*;

mod caret;
pub use caret::*;

mod formats;
pub use formats::*;

mod tdf_font;
pub use tdf_font::*;

mod undo_stack;
pub use undo_stack::*;

mod crc;
pub use crc::*;

mod terminal_state;
pub use terminal_state::*;

pub type EngineResult<T: Send> = Result<T, Box<dyn Error + Send>>;

#[derive(Copy, Clone, Debug, Default)]
pub struct Size<T> 
{
    pub width: T,
    pub height: T
}

impl<T> PartialEq for Size<T>
where T: PartialEq {
    fn eq(&self, other: &Size<T>) -> bool {
        self.width == other.width && self.height == other.height
    }
}

impl<T> Size<T> 
where T: Default
{
    pub fn from(width: T, height: T) -> Self
    {
        Size { width, height }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Rectangle
{
    pub start: Position,
    pub size: Size<i32>
}
