//! Rectangle drawing algorithms

use icy_engine::Position;

use super::{DrawContext, DrawTarget, PointRole};

/// Generate all points on a rectangle outline with their roles
pub fn get_rectangle_points(p0: Position, p1: Position) -> Vec<(Position, PointRole)> {
    let min_x = p0.x.min(p1.x);
    let max_x = p0.x.max(p1.x);
    let min_y = p0.y.min(p1.y);
    let max_y = p0.y.max(p1.y);

    let mut points = Vec::new();

    // Handle degenerate cases
    if min_x == max_x && min_y == max_y {
        // Single point
        points.push((Position::new(min_x, min_y), PointRole::NWCorner));
        return points;
    }

    if min_x == max_x {
        // Vertical line
        for y in min_y..=max_y {
            let role = if y == min_y {
                PointRole::TopSide
            } else if y == max_y {
                PointRole::BottomSide
            } else {
                PointRole::LeftSide
            };
            points.push((Position::new(min_x, y), role));
        }
        return points;
    }

    if min_y == max_y {
        // Horizontal line
        for x in min_x..=max_x {
            let role = if x == min_x {
                PointRole::LeftSide
            } else if x == max_x {
                PointRole::RightSide
            } else {
                PointRole::TopSide
            };
            points.push((Position::new(x, min_y), role));
        }
        return points;
    }

    // Full rectangle outline
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let is_top = y == min_y;
            let is_bottom = y == max_y;
            let is_left = x == min_x;
            let is_right = x == max_x;

            // Only draw border
            if !is_top && !is_bottom && !is_left && !is_right {
                continue;
            }

            let role = match (is_top, is_bottom, is_left, is_right) {
                (true, _, true, _) => PointRole::NWCorner,
                (true, _, _, true) => PointRole::NECorner,
                (_, true, true, _) => PointRole::SWCorner,
                (_, true, _, true) => PointRole::SECorner,
                (true, _, _, _) => PointRole::TopSide,
                (_, true, _, _) => PointRole::BottomSide,
                (_, _, true, _) => PointRole::LeftSide,
                (_, _, _, true) => PointRole::RightSide,
                _ => unreachable!(),
            };

            points.push((Position::new(x, y), role));
        }
    }

    points
}

/// Generate all points of a filled rectangle with their roles
pub fn get_filled_rectangle_points(p0: Position, p1: Position) -> Vec<(Position, PointRole)> {
    let min_x = p0.x.min(p1.x);
    let max_x = p0.x.max(p1.x);
    let min_y = p0.y.min(p1.y);
    let max_y = p0.y.max(p1.y);

    let mut points = Vec::new();

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let is_top = y == min_y;
            let is_bottom = y == max_y;
            let is_left = x == min_x;
            let is_right = x == max_x;

            let role = match (is_top, is_bottom, is_left, is_right) {
                (true, _, true, _) => PointRole::NWCorner,
                (true, _, _, true) => PointRole::NECorner,
                (_, true, true, _) => PointRole::SWCorner,
                (_, true, _, true) => PointRole::SECorner,
                (true, _, _, _) => PointRole::TopSide,
                (_, true, _, _) => PointRole::BottomSide,
                (_, _, true, _) => PointRole::LeftSide,
                (_, _, _, true) => PointRole::RightSide,
                _ => PointRole::Fill,
            };

            points.push((Position::new(x, y), role));
        }
    }

    points
}

/// Draw a rectangle outline
pub fn draw_rectangle<T: DrawTarget>(target: &mut T, ctx: &DrawContext, p0: Position, p1: Position) {
    let points = get_rectangle_points(p0, p1);
    for (pt, role) in points {
        ctx.plot_point(target, pt, role);
    }
}

/// Draw a filled rectangle
pub fn fill_rectangle<T: DrawTarget>(target: &mut T, ctx: &DrawContext, p0: Position, p1: Position) {
    let points = get_filled_rectangle_points(p0, p1);
    for (pt, role) in points {
        ctx.plot_point(target, pt, role);
    }
}
