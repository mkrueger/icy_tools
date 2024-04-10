use crate::Position;

pub mod half_block;
pub use half_block::*;

pub fn get_line_points(from: Position, to: Position) -> Vec<Position> {
    let dx = (to.x - from.x).abs();
    let sx = if from.x < to.x { 1 } else { -1 };
    let dy = (to.y - from.y).abs();
    let sy = if from.y < to.y { 1 } else { -1 };

    let mut err = if dx > dy { dx } else { -dy } / 2;

    let mut result = Vec::new();
    let mut cur = from;
    loop {
        result.push(cur);
        if cur == to {
            break;
        }

        let e2 = err;
        if e2 > -dx {
            err -= dy;
            cur.x += sx;
        }
        if e2 < dy {
            err += dx;
            cur.y += sy;
        }
    }
    result
}
