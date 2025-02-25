pub mod tdf;
pub use tdf::*;

use crate::{EngineResult, Position, editor::EditState};

pub mod figlet;

pub trait AnsiFont: Send {
    fn name(&self) -> &str;
    fn has_char(&self, ch: char) -> bool;
    fn render_next(&self, editor: &mut EditState, prev_char: char, ch: char) -> Position;

    fn font_type(&self) -> FontType;

    fn as_bytes(&self) -> EngineResult<Vec<u8>>;
}
