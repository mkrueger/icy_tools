//! Ellipse drawing algorithms
//!
//! Implements the midpoint ellipse algorithm for drawing ellipses.

use icy_engine::Position;

use super::{DrawContext, DrawTarget, PointRole};

/// Generate all points on an ellipse outline using the midpoint algorithm
pub fn get_ellipse_points(center: Position, radius_x: i32, radius_y: i32) -> Vec<(Position, PointRole)> {
    if radius_x <= 0 || radius_y <= 0 {
        return vec![(center, PointRole::Fill)];
    }

    let mut points = Vec::new();

    let rx2 = (radius_x * radius_x) as i64;
    let ry2 = (radius_y * radius_y) as i64;
    let two_rx2 = 2 * rx2;
    let two_ry2 = 2 * ry2;

    // Region 1
    let mut x = 0i32;
    let mut y = radius_y;
    let mut px = 0i64;
    let mut py = two_rx2 * y as i64;

    // Initial point
    plot_ellipse_points(&mut points, center, x, y);

    // Region 1: |slope| < 1
    let mut p = (ry2 - rx2 * radius_y as i64) + rx2 / 4;

    while px < py {
        x += 1;
        px += two_ry2;

        if p < 0 {
            p += ry2 + px;
        } else {
            y -= 1;
            py -= two_rx2;
            p += ry2 + px - py;
        }

        plot_ellipse_points(&mut points, center, x, y);
    }

    // Region 2: |slope| >= 1
    p = (ry2 * (x as i64 * 2 + 1).pow(2)) / 4 + rx2 * ((y - 1) as i64).pow(2) - rx2 * ry2;

    while y > 0 {
        y -= 1;
        py -= two_rx2;

        if p > 0 {
            p += rx2 - py;
        } else {
            x += 1;
            px += two_ry2;
            p += rx2 - py + px;
        }

        plot_ellipse_points(&mut points, center, x, y);
    }

    points
}

fn plot_ellipse_points(points: &mut Vec<(Position, PointRole)>, center: Position, x: i32, y: i32) {
    // Determine roles based on position
    // Top and bottom extremes
    if x == 0 {
        points.push((Position::new(center.x, center.y - y), PointRole::TopSide));
        points.push((Position::new(center.x, center.y + y), PointRole::BottomSide));
    } else if y == 0 {
        points.push((Position::new(center.x + x, center.y), PointRole::RightSide));
        points.push((Position::new(center.x - x, center.y), PointRole::LeftSide));
    } else {
        // Four symmetric points
        // Quadrant 1 (top-right)
        points.push((Position::new(center.x + x, center.y - y), PointRole::TopSide));
        // Quadrant 2 (top-left)
        points.push((Position::new(center.x - x, center.y - y), PointRole::TopSide));
        // Quadrant 3 (bottom-left)
        points.push((Position::new(center.x - x, center.y + y), PointRole::BottomSide));
        // Quadrant 4 (bottom-right)
        points.push((Position::new(center.x + x, center.y + y), PointRole::BottomSide));
    }
}

/// Draw an ellipse outline
pub fn draw_ellipse<T: DrawTarget>(target: &mut T, ctx: &DrawContext, center: Position, radius_x: i32, radius_y: i32) {
    let points = get_ellipse_points(center, radius_x, radius_y);
    for (pt, role) in points {
        ctx.plot_point(target, pt, role);
    }
}

/// Generate all points of a filled ellipse with their roles
pub fn get_filled_ellipse_points(center: Position, radius_x: i32, radius_y: i32) -> Vec<(Position, PointRole)> {
    if radius_x <= 0 || radius_y <= 0 {
        return vec![(center, PointRole::Fill)];
    }

    let mut points = Vec::new();

    // Use a simple scanline fill approach
    for dy in -radius_y..=radius_y {
        let y = center.y + dy;

        // Calculate x extent at this y using ellipse equation
        // (x/rx)² + (y/ry)² = 1
        // x = rx * sqrt(1 - (dy/ry)²)
        let ry_f = radius_y as f64;
        let rx_f = radius_x as f64;
        let dy_f = dy as f64;

        let x_extent = (rx_f * (1.0 - (dy_f / ry_f).powi(2)).sqrt()).round() as i32;

        for dx in -x_extent..=x_extent {
            let x = center.x + dx;

            // Determine role
            let role = if dy == -radius_y || dy == radius_y {
                if dy < 0 {
                    PointRole::TopSide
                } else {
                    PointRole::BottomSide
                }
            } else if dx == -x_extent {
                PointRole::LeftSide
            } else if dx == x_extent {
                PointRole::RightSide
            } else {
                PointRole::Fill
            };

            points.push((Position::new(x, y), role));
        }
    }

    points
}

/// Generate all points on an ellipse outline from two corner points (bounding box)
pub fn get_ellipse_points_from_rect(p0: Position, p1: Position) -> Vec<(Position, PointRole)> {
    let min_x = p0.x.min(p1.x);
    let max_x = p0.x.max(p1.x);
    let min_y = p0.y.min(p1.y);
    let max_y = p0.y.max(p1.y);

    let center = Position::new((min_x + max_x) / 2, (min_y + max_y) / 2);
    let radius_x = (max_x - min_x) / 2;
    let radius_y = (max_y - min_y) / 2;

    get_ellipse_points(center, radius_x, radius_y)
}

/// Generate all points of a filled ellipse from two corner points (bounding box)
pub fn get_filled_ellipse_points_from_rect(p0: Position, p1: Position) -> Vec<(Position, PointRole)> {
    let min_x = p0.x.min(p1.x);
    let max_x = p0.x.max(p1.x);
    let min_y = p0.y.min(p1.y);
    let max_y = p0.y.max(p1.y);

    let center = Position::new((min_x + max_x) / 2, (min_y + max_y) / 2);
    let radius_x = (max_x - min_x) / 2;
    let radius_y = (max_y - min_y) / 2;

    get_filled_ellipse_points(center, radius_x, radius_y)
}

/// Draw a filled ellipse
pub fn fill_ellipse<T: DrawTarget>(target: &mut T, ctx: &DrawContext, center: Position, radius_x: i32, radius_y: i32) {
    let points = get_filled_ellipse_points(center, radius_x, radius_y);
    for (pt, role) in points {
        ctx.plot_point(target, pt, role);
    }
}

/// Draw an ellipse from two corner points (bounding box)
pub fn draw_ellipse_from_rect<T: DrawTarget>(target: &mut T, ctx: &DrawContext, p0: Position, p1: Position) {
    let min_x = p0.x.min(p1.x);
    let max_x = p0.x.max(p1.x);
    let min_y = p0.y.min(p1.y);
    let max_y = p0.y.max(p1.y);

    let center = Position::new((min_x + max_x) / 2, (min_y + max_y) / 2);
    let radius_x = (max_x - min_x) / 2;
    let radius_y = (max_y - min_y) / 2;

    draw_ellipse(target, ctx, center, radius_x, radius_y);
}

/// Fill an ellipse from two corner points (bounding box)
pub fn fill_ellipse_from_rect<T: DrawTarget>(target: &mut T, ctx: &DrawContext, p0: Position, p1: Position) {
    let min_x = p0.x.min(p1.x);
    let max_x = p0.x.max(p1.x);
    let min_y = p0.y.min(p1.y);
    let max_y = p0.y.max(p1.y);

    let center = Position::new((min_x + max_x) / 2, (min_y + max_y) / 2);
    let radius_x = (max_x - min_x) / 2;
    let radius_y = (max_y - min_y) / 2;

    fill_ellipse(target, ctx, center, radius_x, radius_y);
}
