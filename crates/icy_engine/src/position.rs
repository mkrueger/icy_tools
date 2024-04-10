use std::{
    cmp::Ordering,
    hash::Hash,
    ops::{Add, AddAssign, Sub, SubAssign},
};

#[derive(Copy, Clone, Debug, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Hash for Position {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(x: {}, y: {})", self.x, self.y)
    }
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Position { x, y }
    }

    pub fn with_y(self, y: i32) -> Position {
        Position { x: self.x, y }
    }

    pub fn with_x(self, x: i32) -> Position {
        Position { x, y: self.y }
    }

    pub fn signum(&self) -> Position {
        Position {
            x: self.x.signum(),
            y: self.y.signum(),
        }
    }

    pub fn min(self, other: Self) -> Self {
        Position::new(self.x.min(other.x), self.y.min(other.y))
    }

    pub fn max(self, other: Self) -> Self {
        Position::new(self.x.max(other.x), self.y.max(other.y))
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl Add<Position> for Position {
    type Output = Position;

    fn add(self, rhs: Position) -> Position {
        Position {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign<Position> for Position {
    fn add_assign(&mut self, rhs: Position) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub<Position> for Position {
    type Output = Position;

    fn sub(self, rhs: Position) -> Position {
        Position {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl SubAssign<Position> for Position {
    fn sub_assign(&mut self, rhs: Position) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Position) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.y < other.y {
            return Some(Ordering::Less);
        }
        if self.y > other.y {
            return Some(Ordering::Greater);
        }
        if self.x < other.x {
            return Some(Ordering::Less);
        }
        if self.x > other.x {
            return Some(Ordering::Greater);
        }
        Some(Ordering::Equal)
    }
}

impl From<(usize, usize)> for Position {
    fn from(value: (usize, usize)) -> Self {
        Position {
            x: value.0 as i32,
            y: value.1 as i32,
        }
    }
}

impl From<(i32, i32)> for Position {
    fn from(value: (i32, i32)) -> Self {
        Position { x: value.0, y: value.1 }
    }
}

impl From<(u32, u32)> for Position {
    fn from(value: (u32, u32)) -> Self {
        Position {
            x: value.0 as i32,
            y: value.1 as i32,
        }
    }
}

impl From<(u8, u8)> for Position {
    fn from(value: (u8, u8)) -> Self {
        Position {
            x: value.0 as i32,
            y: value.1 as i32,
        }
    }
}

impl From<(f32, f32)> for Position {
    fn from(value: (f32, f32)) -> Self {
        Position {
            x: value.0 as i32,
            y: value.1 as i32,
        }
    }
}
