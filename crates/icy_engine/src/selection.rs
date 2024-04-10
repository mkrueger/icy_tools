use crate::{Position, Rectangle, Size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rectangle,
    Lines,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AddType {
    Default,
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    pub anchor: Position,
    pub lead: Position,

    pub locked: bool,
    pub shape: Shape,
    pub add_type: AddType,
}

impl Default for Selection {
    fn default() -> Self {
        Selection::new((0, 0))
    }
}

impl Selection {
    pub fn new(pos: impl Into<Position>) -> Self {
        let pos = pos.into();
        Self {
            anchor: pos,
            lead: pos,
            locked: false,
            shape: Shape::Lines,
            add_type: AddType::Default,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.lead
    }

    pub fn is_inside(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();
        self.as_rectangle().is_inside(pos)
    }

    pub fn min(&self) -> Position {
        self.anchor.min(self.lead)
    }

    pub fn max(&self) -> Position {
        self.anchor.max(self.lead)
    }

    pub fn size(&self) -> Size {
        Size::new((self.anchor.x - self.lead.x).abs(), (self.anchor.y - self.lead.y).abs())
    }

    pub fn as_rectangle(&self) -> Rectangle {
        Rectangle::from_min_size(self.min(), self.size())
    }
}

impl From<Rectangle> for Selection {
    fn from(value: Rectangle) -> Self {
        Selection {
            anchor: value.top_left(),
            lead: value.bottom_right(),
            locked: false,
            shape: Shape::Rectangle,
            add_type: AddType::Default,
        }
    }
}

impl From<(f32, f32, f32, f32)> for Selection {
    fn from(value: (f32, f32, f32, f32)) -> Self {
        Selection {
            anchor: (value.0, value.1).into(),
            lead: (value.2, value.3).into(),
            locked: false,
            shape: Shape::Rectangle,
            add_type: AddType::Default,
        }
    }
}

impl From<(i32, i32, i32, i32)> for Selection {
    fn from(value: (i32, i32, i32, i32)) -> Self {
        Selection {
            anchor: (value.0, value.1).into(),
            lead: (value.2, value.3).into(),
            locked: false,
            shape: Shape::Rectangle,
            add_type: AddType::Default,
        }
    }
}
