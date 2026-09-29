//! Box-drawing lines for the line tool: the line is drawn with CP437's single and double line
//! characters, and where it meets lines already on the canvas the matching junction is chosen,
//! like TheDraw's line drawing.

use icy_engine::Position;

/// How the line looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BoxStyle {
    /// `─ │`
    #[default]
    Single,
    /// `═ ║`
    Double,
    /// `═` across, `│` down
    DoubleHorizontal,
    /// `─` across, `║` down
    DoubleVertical,
}

impl BoxStyle {
    pub const ALL: [BoxStyle; 4] = [BoxStyle::Single, BoxStyle::Double, BoxStyle::DoubleHorizontal, BoxStyle::DoubleVertical];

    /// Weights of the horizontal and the vertical strokes: 1 single, 2 double.
    fn weights(self) -> (u8, u8) {
        match self {
            BoxStyle::Single => (1, 1),
            BoxStyle::Double => (2, 2),
            BoxStyle::DoubleHorizontal => (2, 1),
            BoxStyle::DoubleVertical => (1, 2),
        }
    }

    /// A sample character for buttons: the cross of the style.
    pub fn sample(self) -> char {
        char_for([self.weights().1, self.weights().0, self.weights().1, self.weights().0])
    }
}

/// The strokes leaving a cell up, right, down and left: 0 none, 1 single, 2 double.
pub type Arms = [u8; 4];

const UP: usize = 0;
const RIGHT: usize = 1;
const DOWN: usize = 2;
const LEFT: usize = 3;

/// Every CP437 box-drawing character with its arms.
const BOX_CHARS: [(char, Arms); 40] = [
    ('─', [0, 1, 0, 1]),
    ('│', [1, 0, 1, 0]),
    ('┌', [0, 1, 1, 0]),
    ('┐', [0, 0, 1, 1]),
    ('└', [1, 1, 0, 0]),
    ('┘', [1, 0, 0, 1]),
    ('├', [1, 1, 1, 0]),
    ('┤', [1, 0, 1, 1]),
    ('┬', [0, 1, 1, 1]),
    ('┴', [1, 1, 0, 1]),
    ('┼', [1, 1, 1, 1]),
    ('═', [0, 2, 0, 2]),
    ('║', [2, 0, 2, 0]),
    ('╔', [0, 2, 2, 0]),
    ('╗', [0, 0, 2, 2]),
    ('╚', [2, 2, 0, 0]),
    ('╝', [2, 0, 0, 2]),
    ('╠', [2, 2, 2, 0]),
    ('╣', [2, 0, 2, 2]),
    ('╦', [0, 2, 2, 2]),
    ('╩', [2, 2, 0, 2]),
    ('╬', [2, 2, 2, 2]),
    ('╒', [0, 2, 1, 0]),
    ('╕', [0, 0, 1, 2]),
    ('╘', [1, 2, 0, 0]),
    ('╛', [1, 0, 0, 2]),
    ('╞', [1, 2, 1, 0]),
    ('╡', [1, 0, 1, 2]),
    ('╤', [0, 2, 1, 2]),
    ('╧', [1, 2, 0, 2]),
    ('╪', [1, 2, 1, 2]),
    ('╓', [0, 1, 2, 0]),
    ('╖', [0, 0, 2, 1]),
    ('╙', [2, 1, 0, 0]),
    ('╜', [2, 0, 0, 1]),
    ('╟', [2, 1, 2, 0]),
    ('╢', [2, 0, 2, 1]),
    ('╥', [0, 1, 2, 1]),
    ('╨', [2, 1, 0, 1]),
    ('╫', [2, 1, 2, 1]),
];

/// The arms of a box-drawing character (as Unicode), `None` for any other character.
pub fn arms_of(ch: char) -> Option<Arms> {
    BOX_CHARS.iter().find(|(candidate, _)| *candidate == ch).map(|(_, arms)| *arms)
}

/// The box-drawing character for a set of arms. CP437 draws both horizontal arms of a cell with
/// the same weight and both vertical ones too, so the heavier one wins; a cell with strokes in one
/// direction only becomes a straight line.
pub fn char_for(arms: Arms) -> char {
    let horizontal = arms[LEFT].max(arms[RIGHT]);
    let vertical = arms[UP].max(arms[DOWN]);
    let normalized = [
        if arms[UP] > 0 { vertical } else { 0 },
        if arms[RIGHT] > 0 { horizontal } else { 0 },
        if arms[DOWN] > 0 { vertical } else { 0 },
        if arms[LEFT] > 0 { horizontal } else { 0 },
    ];
    let normalized = match (horizontal > 0, vertical > 0) {
        (true, false) | (false, false) => [0, horizontal.max(1), 0, horizontal.max(1)],
        (false, true) => [vertical, 0, vertical, 0],
        (true, true) => normalized,
    };
    BOX_CHARS.iter().find(|(_, candidate)| *candidate == normalized).map_or('┼', |(ch, _)| *ch)
}

/// The cells from `start` to `end`: a straight line, or an elbow when the two are not in one row
/// or column, going along the longer direction first.
pub fn box_path(start: Position, end: Position) -> Vec<Position> {
    let corner = if (end.x - start.x).abs() >= (end.y - start.y).abs() {
        Position::new(end.x, start.y)
    } else {
        Position::new(start.x, end.y)
    };
    let mut path = vec![start];
    for target in [corner, end] {
        let mut current = *path.last().unwrap();
        while current != target {
            current = current + Position::new((target.x - current.x).signum(), (target.y - current.y).signum());
            path.push(current);
        }
    }
    path
}

/// The characters of a box line from `start` to `end`. Each cell gets arms towards its neighbors
/// on the line, joined with the arms of box characters already there (`existing`), so crossing or
/// touching a line makes a junction. An existing arm that leads nowhere, like the loose end of a
/// `│`, is dropped, so a line leaving the end of another turns it into a corner. A click without
/// dragging draws nothing.
pub fn box_line(start: Position, end: Position, style: BoxStyle, existing: impl Fn(Position) -> Option<Arms>) -> Vec<(Position, char)> {
    let path = box_path(start, end);
    if path.len() < 2 {
        return Vec::new();
    }
    let (horizontal, vertical) = style.weights();
    let arm_towards = |from: Position, to: Position| -> (usize, u8) {
        match (to.x - from.x, to.y - from.y) {
            (1, _) => (RIGHT, horizontal),
            (-1, _) => (LEFT, horizontal),
            (_, 1) => (DOWN, vertical),
            _ => (UP, vertical),
        }
    };
    let offsets = [Position::new(0, -1), Position::new(1, 0), Position::new(0, 1), Position::new(-1, 0)];
    path.iter()
        .enumerate()
        .map(|(index, &point)| {
            let mut arms = existing(point).unwrap_or_default();
            for (arm, offset) in offsets.iter().enumerate() {
                let connected = existing(point + *offset).is_some_and(|neighbor| neighbor[(arm + 2) % 4] > 0);
                if arms[arm] > 0 && !connected {
                    arms[arm] = 0;
                }
            }
            for neighbor in [index.checked_sub(1).map(|previous| path[previous]), path.get(index + 1).copied()]
                .into_iter()
                .flatten()
            {
                let (arm, weight) = arm_towards(point, neighbor);
                arms[arm] = arms[arm].max(weight);
            }
            (point, char_for(arms))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(start: (i32, i32), end: (i32, i32), style: BoxStyle) -> String {
        box_line(Position::new(start.0, start.1), Position::new(end.0, end.1), style, |_| None)
            .into_iter()
            .map(|(_, ch)| ch)
            .collect()
    }

    #[test]
    fn straight_lines_and_elbows() {
        assert_eq!(draw((0, 0), (3, 0), BoxStyle::Single), "────");
        assert_eq!(draw((2, 3), (2, 0), BoxStyle::Double), "║║║║");
        assert_eq!(draw((0, 0), (3, 2), BoxStyle::Single), "───┐││", "along the longer direction, then down");
        assert_eq!(draw((0, 0), (1, 3), BoxStyle::Single), "│││└─", "along the longer direction, then across");
        assert_eq!(draw((0, 0), (2, 1), BoxStyle::DoubleHorizontal), "══╕│");
        assert!(draw((4, 4), (4, 4), BoxStyle::Single).is_empty(), "a click draws nothing");
    }

    #[test]
    fn lines_join_the_lines_they_meet() {
        // A horizontal line through the middle of a vertical one makes a cross, ending on it a T.
        let vertical = |point: Position| (point.x == 2).then(|| arms_of('│').unwrap());
        let cross = box_line(Position::new(0, 1), Position::new(4, 1), BoxStyle::Single, vertical);
        assert_eq!(cross.iter().map(|(_, ch)| *ch).collect::<String>(), "──┼──");
        let tee = box_line(Position::new(0, 1), Position::new(2, 1), BoxStyle::Single, vertical);
        assert_eq!(tee.last().unwrap().1, '┤');
        // Mixed weights pick the matching mixed junction.
        let double_vertical = |point: Position| (point.x == 2).then(|| arms_of('║').unwrap());
        let mixed = box_line(Position::new(0, 1), Position::new(4, 1), BoxStyle::Single, double_vertical);
        assert_eq!(mixed[2].1, '╫');
        let corner = |point: Position| match (point.x, point.y) {
            (0, 0) => arms_of('─'),
            (1, 0) => arms_of('─'),
            _ => None,
        };
        let extended = box_line(Position::new(0, 0), Position::new(0, 2), BoxStyle::Double, corner);
        assert_eq!(extended[0].1, '╓', "a double line down from the loose end of a single one");
        // A vertical line's loose lower end becomes a corner, not a T.
        let stub = |point: Position| (point.x == 0 && point.y <= 2).then(|| arms_of('│').unwrap());
        let turn = box_line(Position::new(0, 2), Position::new(3, 2), BoxStyle::Single, stub);
        assert_eq!(turn[0].1, '└');
        let crossing_end = box_line(Position::new(-2, 2), Position::new(2, 2), BoxStyle::Single, stub);
        assert_eq!(crossing_end[2].1, '┴', "a line across the end of another makes a T");
    }

    #[test]
    fn every_arm_combination_has_a_character() {
        for up in 0..3u8 {
            for right in 0..3u8 {
                for down in 0..3u8 {
                    for left in 0..3u8 {
                        let arms = [up, right, down, left];
                        let ch = char_for(arms);
                        assert!(arms_of(ch).is_some(), "{arms:?} -> {ch}");
                    }
                }
            }
        }
        for (ch, arms) in BOX_CHARS {
            assert_eq!(char_for(arms), ch, "{ch} keeps its shape");
        }
        assert_eq!(BoxStyle::DoubleVertical.sample(), '╫');
    }
}
