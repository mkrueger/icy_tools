#![allow(clippy::comparison_chain)]
use icy_engine::Position;
use icy_engine_gui::BufferView;

use super::{plot_point, BrushMode, ColorMode, PointRole};

fn get_line_points(from: Position, to: Position) -> Vec<Position> {
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

pub fn draw_line(buffer_view: &mut BufferView, from: impl Into<Position>, to: impl Into<Position>, mode: BrushMode, color_mode: ColorMode) {
    let mut from = from.into();
    let mut to = to.into();
    let mut y_mul = 1;
    if !matches!(mode, BrushMode::HalfBlock) {
        from.y /= 2;
        to.y /= 2;
        y_mul = 2;
    }

    let mut v = get_line_points(from, to);
    if v.is_empty() {
        return;
    }
    if !matches!(mode, BrushMode::Outline) {
        for point in get_line_points(from, to) {
            plot_point(buffer_view, (point.x, point.y * y_mul), mode.clone(), color_mode, PointRole::Line);
        }
        return;
    }
    let mut n = Vec::new();
    for pt in v {
        let p = Position::new(pt.x, 2 * (pt.y / 2));
        if let Some(l) = n.last() {
            if p == *l {
                continue;
            }
        }
        n.push(p);
    }
    v = n;
    let mut last = v[0];
    let mut i = 0;
    while i < v.len() {
        let is_last = i + 1 >= v.len();
        let next = if is_last {
            Position::new(v[i].x + v[i].x - last.x, v[i].y + v[i].y + last.y)
        } else {
            v[i + 1]
        };
        let point = v[i];
        last = point;
        let cy2 = point.y;
        let ny2 = next.y;

        let role = if point.x > next.x {
            // Left side
            if ny2 < cy2 {
                plot_point(buffer_view, point + Position::new(0, -1), mode.clone(), color_mode, PointRole::NECorner);
                PointRole::SWCorner
            } else if ny2 > cy2 {
                plot_point(buffer_view, point + Position::new(0, 2), mode.clone(), color_mode, PointRole::SECorner);
                PointRole::NWCorner
            } else {
                PointRole::TopSide
            }
        } else if point.x < next.x {
            // Right Side
            if ny2 < cy2 {
                plot_point(buffer_view, point + Position::new(0, -1), mode.clone(), color_mode, PointRole::NWCorner);
                PointRole::SECorner
            } else if ny2 > cy2 {
                plot_point(buffer_view, point + Position::new(0, 2), mode.clone(), color_mode, PointRole::SWCorner);
                PointRole::NECorner
            } else {
                // telel
                if point.x < next.x {
                    if point.y == next.y {
                        PointRole::TopSide
                    } else {
                        i += 1;
                        plot_point(buffer_view, point + Position::new(1, 0), mode.clone(), color_mode, PointRole::NECorner);
                        PointRole::SWCorner
                    }
                } else if point.x > next.x {
                    plot_point(buffer_view, point + Position::new(-1, 0), mode.clone(), color_mode, PointRole::NWCorner);
                    PointRole::SECorner
                } else {
                    // case 4
                    if last.y == point.y {
                        PointRole::TopSide
                    } else {
                        plot_point(buffer_view, point + Position::new(1, 0), mode.clone(), color_mode, PointRole::NECorner);
                        PointRole::SWCorner
                    }
                }
            }
        } else {
            PointRole::LeftSide
        };
        plot_point(buffer_view, point, mode.clone(), color_mode, role);
        i += 1;
    }
}
