use egui::ahash::HashSet;
use icy_engine::Position;
use icy_engine_gui::BufferView;

use super::{plot_point, BrushMode, ColorMode, PointRole};

fn get_ellipse_points(from: Position, to: Position) -> Vec<Position> {
    let mut result = Vec::new();

    let rx = (from.x - to.x).abs() / 2;
    let ry = (from.y - to.y).abs() / 2;

    let xc = (from.x + to.x) / 2;
    let yc = (from.y + to.y) / 2;

    let mut x = 0;
    let mut y = ry;

    let mut d1 = (ry * ry) - (rx * rx * ry) + (rx * rx) / 4;
    let mut dx = 2 * ry * ry * x;
    let mut dy = 2 * rx * rx * y;

    while dx < dy {
        result.push(Position::new(-x + xc, y + yc));
        result.push(Position::new(x + xc, y + yc));
        result.push(Position::new(-x + xc, -y + yc));
        result.push(Position::new(x + xc, -y + yc));

        if d1 < 0 {
            x += 1;
            dx += 2 * ry * ry;
            d1 += dx + (ry * ry);
        } else {
            x += 1;
            y -= 1;
            dx += 2 * ry * ry;
            dy -= 2 * rx * rx;
            d1 += dx - dy + (ry * ry);
        }
    }

    let mut d2 = ((ry * ry) * ((x/*+ 0.5f*/) * (x/*+ 0.5f*/))) + ((rx * rx) * ((y - 1) * (y - 1))) - (rx * rx * ry * ry);

    while y >= 0 {
        result.push(Position::new(-x + xc, y + yc));
        result.push(Position::new(x + xc, y + yc));
        result.push(Position::new(-x + xc, -y + yc));
        result.push(Position::new(x + xc, -y + yc));
        if d2 > 0 {
            y -= 1;
            dy -= 2 * rx * rx;
            d2 += (rx * rx) - dy;
        } else {
            y -= 1;
            x += 1;
            dx += 2 * ry * ry;
            dy -= 2 * rx * rx;
            d2 += dx - dy + (rx * rx);
        }
    }
    result
}

pub fn draw_ellipse(buffer_view: &mut BufferView, from: impl Into<Position>, to: impl Into<Position>, mode: BrushMode, color_mode: ColorMode) {
    let mut from = from.into();
    let mut to = to.into();
    let mut y_mul = 1;
    if !matches!(mode, BrushMode::HalfBlock) {
        from.y /= 2;
        to.y /= 2;
        y_mul = 2;
    }
    let mut visited = HashSet::default();
    for point in get_ellipse_points(from, to) {
        let pos = (point.x, point.y * y_mul);
        if visited.insert(pos) {
            plot_point(buffer_view, pos, mode.clone(), color_mode, PointRole::Line);
        }
    }
}

pub fn fill_ellipse(buffer_view: &mut BufferView, from: impl Into<Position>, to: impl Into<Position>, mode: BrushMode, color_mode: ColorMode) {
    let mut from = from.into();
    let mut to = to.into();
    let mut y_mul = 1;
    if !matches!(mode, BrushMode::HalfBlock) {
        from.y /= 2;
        to.y /= 2;
        y_mul = 2;
    }
    let points = get_ellipse_points(from, to);
    let mut visited = HashSet::default();

    for i in 0..points.len() / 2 {
        let mut x1 = points[i * 2];
        let x2 = points[i * 2 + 1];
        if !visited.insert(x1.y) {
            continue;
        }

        while x1.x < x2.x {
            plot_point(buffer_view, (x1.x, x1.y * y_mul), mode.clone(), color_mode, PointRole::Line);

            x1.x += 1;
        }
    }
}
