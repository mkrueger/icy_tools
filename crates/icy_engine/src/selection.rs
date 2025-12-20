use serde::{Deserialize, Serialize};

use crate::{Position, Rectangle, Size};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Rectangle,
    Lines,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AddType {
    Default,
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
        match self.shape {
            Shape::Rectangle => {
                let pos = pos.into();
                self.as_rectangle().contains_pt(pos)
            }
            Shape::Lines => {
                let pos = pos.into();
                // Same-line selection: straight min/max span
                if self.anchor.y == self.lead.y {
                    let left = self.anchor.x.min(self.lead.x);
                    let right = self.anchor.x.max(self.lead.x);
                    return pos.y == self.anchor.y && pos.x >= left && pos.x <= right;
                }

                // Directional multi-line selection
                // Downward drag
                if self.anchor.y < self.lead.y {
                    if pos.y < self.anchor.y || pos.y > self.lead.y {
                        return false;
                    }
                    if pos.y == self.anchor.y {
                        // First line: from anchor.x to end-of-line (≥ anchor.x)
                        return pos.x >= self.anchor.x;
                    }
                    if pos.y == self.lead.y {
                        // Last line: from start-of-line to lead.x (≤ lead.x)
                        return pos.x <= self.lead.x;
                    }
                    // Intermediate line: whole line selected
                    return true;
                } else {
                    // Upward drag (anchor.y > lead.y)
                    if pos.y < self.lead.y || pos.y > self.anchor.y {
                        return false;
                    }
                    if pos.y == self.lead.y {
                        // First line in visual order (where drag ended): from lead.x to end-of-line
                        return pos.x >= self.lead.x;
                    }
                    if pos.y == self.anchor.y {
                        // Last line in visual order (where drag began): from start-of-line to anchor.x
                        return pos.x <= self.anchor.x;
                    }
                    // Intermediate line: whole line selected
                    return true;
                }
            }
        }
    }

    pub fn min(&self) -> Position {
        self.anchor.min(self.lead)
    }

    pub fn max(&self) -> Position {
        self.anchor.max(self.lead)
    }

    pub fn size(&self) -> Size {
        // Selection is inclusive on both ends, so add 1 to each dimension
        Size::new((self.anchor.x - self.lead.x).abs() + 1, (self.anchor.y - self.lead.y).abs() + 1)
    }

    pub fn as_rectangle(&self) -> Rectangle {
        Rectangle::from_min_size(self.min(), self.size())
    }

    pub fn as_rectangle_with_width(&self, buffer_width: i32) -> Rectangle {
        match self.shape {
            Shape::Rectangle => Rectangle::from_min_size(self.min(), self.size()),
            Shape::Lines => {
                if self.anchor.y == self.lead.y {
                    // Single line selection
                    let left = self.anchor.x.min(self.lead.x);
                    let right = self.anchor.x.max(self.lead.x) + 1;
                    Rectangle::from_coords(left, self.anchor.y, right, self.anchor.y + 1)
                } else {
                    // Multi-line selection uses full buffer width
                    let min_y = self.anchor.y.min(self.lead.y);
                    let max_y = self.anchor.y.max(self.lead.y) + 1;
                    Rectangle::from_coords(0, min_y, buffer_width - 1, max_y)
                }
            }
        }
    }
}

impl From<Rectangle> for Selection {
    fn from(value: Rectangle) -> Self {
        // Rectangle uses exclusive bounds (start + size), Selection uses inclusive (anchor, lead)
        // So lead should be bottom_right - 1 in each dimension
        let br = value.bottom_right();
        Selection {
            anchor: value.top_left(),
            lead: Position::new(br.x.saturating_sub(1), br.y.saturating_sub(1)),
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
