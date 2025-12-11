use crate::{Position, Rectangle, Size};

#[derive(Default, Clone, PartialEq)]
pub struct OverlayMask {
    size: Size,
    lines: Vec<Vec<bool>>,
}

impl OverlayMask {
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn add_rectangle(&mut self, rect: Rectangle) {
        for y in rect.y_range_inclusive() {
            for x in rect.x_range_inclusive() {
                self.set_is_selected((x, y), true);
            }
        }
    }

    pub fn remove_rectangle(&mut self, rect: Rectangle) {
        for y in rect.y_range_inclusive() {
            for x in rect.x_range_inclusive() {
                self.set_is_selected((x, y), false);
            }
        }
    }

    pub fn is_selected(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();
        if !self.in_bounds(pos) {
            return false;
        }

        if pos.y < self.lines.len() as i32 {
            let line = &self.lines[pos.y as usize];
            if pos.x < line.len() as i32 {
                return line[pos.x as usize];
            }
        }
        false
    }

    fn in_bounds(&self, pos: Position) -> bool {
        pos.x >= 0 && pos.x < self.size.width && pos.y >= 0 && pos.y < self.size.height
    }

    pub fn set_is_selected(&mut self, pos: impl Into<Position>, selected: bool) {
        let pos = pos.into();
        if !self.in_bounds(pos) {
            return;
        }

        if self.lines.len() <= pos.y as usize {
            self.lines.resize(pos.y as usize + 1, Vec::new());
        }

        let line = &mut self.lines[pos.y as usize];
        if line.len() <= pos.x as usize {
            line.resize(pos.x as usize + 1, false);
        }
        line[pos.x as usize] = selected;
    }

    pub fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    pub fn is_empty(&self) -> bool {
        for l in &self.lines {
            if l.contains(&true) {
                return false;
            }
        }
        true
    }

    pub fn rectangle(&self) -> Rectangle {
        let mut y_min = usize::MAX;
        let mut x_min = usize::MAX;
        let mut y_max = 0;
        let mut x_max = 0;

        for (y, line) in self.lines.iter().enumerate() {
            for (x, b) in line.iter().enumerate() {
                if *b {
                    y_min = y_min.min(y);
                    x_min = x_min.min(x);
                    y_max = y_max.max(y);
                    x_max = x_max.max(x);
                }
            }
        }
        if x_max >= x_min && y_max >= y_min {
            Rectangle::from_min_size((x_min, y_min), (x_max - x_min + 1, y_max - y_min + 1))
        } else {
            Rectangle::default()
        }
    }

    pub fn width(&self) -> i32 {
        self.size.width
    }

    pub fn height(&self) -> i32 {
        self.size.height
    }
}
