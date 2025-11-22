use super::{Position, TextAttribute};
pub use icy_parser_core::CaretShape;

#[derive(Clone)]
pub struct Caret {
    pub x: i32,
    pub y: i32,
    pub attribute: TextAttribute,
    pub insert_mode: bool,
    pub visible: bool,
    pub blinking: bool,
    pub shape: CaretShape,
    pub use_pixel_positioning: bool,
}

impl Caret {
    pub fn from_xy(x: i32, y: i32) -> Self {
        Self { x, y, ..Default::default() }
    }

    pub fn position(&self) -> Position {
        Position::new(self.x, self.y)
    }

    pub fn set_position(&mut self, pos: Position) {
        self.x = pos.x;
        self.y = pos.y;
    }

    pub fn set_foreground(&mut self, color: u32) {
        self.attribute.set_foreground(color);
    }

    pub fn set_background(&mut self, color: u32) {
        self.attribute.set_background(color);
    }

    pub(crate) fn reset(&mut self) {
        self.x = 0;
        self.y = 0;
        self.attribute = TextAttribute::default();
        self.insert_mode = false;
        self.visible = true;
        self.blinking = true;
        self.shape = CaretShape::Block;
    }

    pub fn font_page(&self) -> usize {
        self.attribute.get_font_page()
    }

    pub fn set_font_page(&mut self, page: usize) {
        self.attribute.set_font_page(page);
    }
}

impl std::fmt::Debug for Caret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cursor")
            .field("pos", &Position::new(self.x, self.y))
            .field("attr", &self.attribute)
            .field("insert_mode", &self.insert_mode)
            .finish_non_exhaustive()
    }
}

impl Default for Caret {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            attribute: TextAttribute::default(),
            insert_mode: false,
            visible: true,
            blinking: true,
            shape: CaretShape::Block,
            use_pixel_positioning: false,
        }
    }
}

impl PartialEq for Caret {
    fn eq(&self, other: &Caret) -> bool {
        self.x == other.x && self.y == other.y && self.attribute == other.attribute
    }
}

impl From<Position> for Caret {
    fn from(pos: Position) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            ..Default::default()
        }
    }
}

impl From<(i32, i32)> for Caret {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x, y, ..Default::default() }
    }
}

impl From<Caret> for Position {
    fn from(caret: Caret) -> Self {
        Position::new(caret.x, caret.y)
    }
}

impl From<&Caret> for Position {
    fn from(caret: &Caret) -> Self {
        Position::new(caret.x, caret.y)
    }
}
