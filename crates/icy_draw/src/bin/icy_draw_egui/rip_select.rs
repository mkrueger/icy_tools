//! Selecting and reshaping RIP shapes on the canvas: hit tests, handles and the geometry that
//! handle drags change, independent of the UI.

use icy_parser_core::RipCommand;

pub type Point = (i32, i32);

const MAX_X: i32 = 639;
const MAX_Y: i32 = 349;

/// The editable geometry of a drawing command.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    /// Pixels, line ends and Bézier points.
    Points(Vec<Point>),
    Text {
        anchor: Point,
        width: i32,
        height: i32,
    },
    /// Normalized so that `x0 <= x1` and `y0 <= y1`.
    Rect {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
    },
    Circle {
        center: Point,
        radius: i32,
    },
    Ellipse {
        center: Point,
        rx: i32,
        ry: i32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Move,
    Point(usize),
    Rect { left: bool, top: bool, right: bool, bottom: bool },
    Radius,
    EllipseX,
    EllipseY,
}

impl Handle {
    pub fn cursor(self) -> eframe::egui::CursorIcon {
        use eframe::egui::CursorIcon;
        match self {
            Handle::Move => CursorIcon::Move,
            Handle::Rect { left, top, right, bottom } => match (left || right, top || bottom) {
                (true, false) => CursorIcon::ResizeHorizontal,
                (false, true) => CursorIcon::ResizeVertical,
                _ if (left && top) || (right && bottom) => CursorIcon::ResizeNwSe,
                _ => CursorIcon::ResizeNeSw,
            },
            Handle::EllipseX => CursorIcon::ResizeHorizontal,
            Handle::EllipseY => CursorIcon::ResizeVertical,
            Handle::Radius | Handle::Point(_) => CursorIcon::Crosshair,
        }
    }
}

fn rect(x0: u16, y0: u16, x1: u16, y1: u16) -> Geometry {
    let (x0, y0, x1, y1) = (i32::from(x0), i32::from(y0), i32::from(x1), i32::from(y1));
    Geometry::Rect {
        x0: x0.min(x1),
        y0: y0.min(y1),
        x1: x0.max(x1),
        y1: y0.max(y1),
    }
}

/// The geometry of a drawing command. `button_size` is the style size used by buttons placed
/// without an explicit rectangle.
pub fn geometry(command: &RipCommand, button_size: (u16, u16)) -> Option<Geometry> {
    let point = |x: u16, y: u16| (i32::from(x), i32::from(y));
    Some(match command {
        RipCommand::Pixel { x, y } => Geometry::Points(vec![point(*x, *y)]),
        RipCommand::TextXY { x, y, text } => Geometry::Text {
            anchor: point(*x, *y),
            width: (text.len() as i32 * 8).max(1),
            height: 8,
        },
        RipCommand::Line { x0, y0, x1, y1 } => Geometry::Points(vec![point(*x0, *y0), point(*x1, *y1)]),
        RipCommand::Bezier {
            x1,
            y1,
            x2,
            y2,
            x3,
            y3,
            x4,
            y4,
            ..
        } => Geometry::Points(vec![point(*x1, *y1), point(*x2, *y2), point(*x3, *y3), point(*x4, *y4)]),
        RipCommand::Polygon { points } | RipCommand::FilledPolygon { points } | RipCommand::PolyLine { points } => {
            Geometry::Points(points.chunks_exact(2).map(|pair| point(pair[0], pair[1])).collect())
        }
        RipCommand::Rectangle { x0, y0, x1, y1 } | RipCommand::Bar { x0, y0, x1, y1 } | RipCommand::Mouse { x0, y0, x1, y1, .. } => rect(*x0, *y0, *x1, *y1),
        RipCommand::Button { x0, y0, x1, y1, .. } => {
            if *x1 == 0 && *y1 == 0 {
                let (width, height) = (button_size.0.max(1), button_size.1.max(1));
                rect(*x0, *y0, x0.saturating_add(width - 1), y0.saturating_add(height - 1))
            } else {
                rect(*x0, *y0, *x1, *y1)
            }
        }
        RipCommand::Circle { x_center, y_center, radius } => Geometry::Circle {
            center: point(*x_center, *y_center),
            radius: i32::from(*radius),
        },
        RipCommand::Arc { x, y, radius, .. } | RipCommand::PieSlice { x, y, radius, .. } => Geometry::Circle {
            center: point(*x, *y),
            radius: i32::from(*radius),
        },
        RipCommand::Oval { x, y, x_rad, y_rad, .. }
        | RipCommand::OvalArc { x, y, x_rad, y_rad, .. }
        | RipCommand::OvalPieSlice { x, y, x_rad, y_rad, .. }
        | RipCommand::FilledOval { x, y, x_rad, y_rad } => Geometry::Ellipse {
            center: point(*x, *y),
            rx: i32::from(*x_rad),
            ry: i32::from(*y_rad),
        },
        _ => return None,
    })
}

fn coordinate(value: i32) -> u16 {
    value.clamp(0, 1295) as u16
}

/// `command` with its coordinates taken from `geometry`.
pub fn apply(command: &RipCommand, geometry: &Geometry, button_size: (u16, u16)) -> RipCommand {
    let mut command = command.clone();
    match (&mut command, geometry) {
        (RipCommand::Pixel { x, y }, Geometry::Points(points)) if !points.is_empty() => {
            (*x, *y) = (coordinate(points[0].0), coordinate(points[0].1));
        }
        (RipCommand::TextXY { x, y, .. }, Geometry::Text { anchor, .. }) => {
            (*x, *y) = (coordinate(anchor.0), coordinate(anchor.1));
        }
        (RipCommand::Line { x0, y0, x1, y1 }, Geometry::Points(points)) if points.len() == 2 => {
            (*x0, *y0, *x1, *y1) = (
                coordinate(points[0].0),
                coordinate(points[0].1),
                coordinate(points[1].0),
                coordinate(points[1].1),
            );
        }
        (
            RipCommand::Polygon { points: coords } | RipCommand::FilledPolygon { points: coords } | RipCommand::PolyLine { points: coords },
            Geometry::Points(points),
        ) if coords.len() == points.len() * 2 => {
            for (pair, point) in coords.chunks_exact_mut(2).zip(points) {
                pair[0] = coordinate(point.0);
                pair[1] = coordinate(point.1);
            }
        }
        (
            RipCommand::Bezier {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                x4,
                y4,
                ..
            },
            Geometry::Points(points),
        ) if points.len() == 4 => {
            for ((x, y), point) in [(x1, y1), (x2, y2), (x3, y3), (x4, y4)].into_iter().zip(points) {
                (*x, *y) = (coordinate(point.0), coordinate(point.1));
            }
        }
        (
            RipCommand::Rectangle { x0, y0, x1, y1 } | RipCommand::Bar { x0, y0, x1, y1 } | RipCommand::Mouse { x0, y0, x1, y1, .. },
            Geometry::Rect { x0: l, y0: t, x1: r, y1: b },
        ) => {
            (*x0, *y0, *x1, *y1) = (coordinate(*l), coordinate(*t), coordinate(*r), coordinate(*b));
        }
        (RipCommand::Button { x0, y0, x1, y1, .. }, Geometry::Rect { x0: l, y0: t, x1: r, y1: b }) => {
            // A button that keeps the style's size stays sized by the style.
            let dynamic = *x1 == 0 && *y1 == 0 && r - l + 1 == i32::from(button_size.0.max(1)) && b - t + 1 == i32::from(button_size.1.max(1));
            (*x0, *y0) = (coordinate(*l), coordinate(*t));
            if !dynamic {
                (*x1, *y1) = (coordinate(*r), coordinate(*b));
            }
        }
        (RipCommand::Circle { x_center, y_center, radius }, Geometry::Circle { center, radius: r }) => {
            (*x_center, *y_center, *radius) = (coordinate(center.0), coordinate(center.1), coordinate(*r));
        }
        (RipCommand::Arc { x, y, radius, .. } | RipCommand::PieSlice { x, y, radius, .. }, Geometry::Circle { center, radius: r }) => {
            (*x, *y, *radius) = (coordinate(center.0), coordinate(center.1), coordinate(*r));
        }
        (
            RipCommand::Oval { x, y, x_rad, y_rad, .. }
            | RipCommand::OvalArc { x, y, x_rad, y_rad, .. }
            | RipCommand::OvalPieSlice { x, y, x_rad, y_rad, .. }
            | RipCommand::FilledOval { x, y, x_rad, y_rad },
            Geometry::Ellipse { center, rx, ry },
        ) => {
            (*x, *y, *x_rad, *y_rad) = (coordinate(center.0), coordinate(center.1), coordinate(*rx), coordinate(*ry));
        }
        _ => {}
    }
    command
}

/// The handles of a shape, in scene pixels.
pub fn handles(command: &RipCommand, geometry: &Geometry) -> Vec<(Handle, Point)> {
    match geometry {
        Geometry::Points(points)
            if matches!(
                command,
                RipCommand::Line { .. }
                    | RipCommand::Bezier { .. }
                    | RipCommand::Polygon { .. }
                    | RipCommand::FilledPolygon { .. }
                    | RipCommand::PolyLine { .. }
            ) =>
        {
            points.iter().enumerate().map(|(index, point)| (Handle::Point(index), *point)).collect()
        }
        Geometry::Points(_) | Geometry::Text { .. } => Vec::new(),
        Geometry::Rect { x0, y0, x1, y1 } => {
            let (cx, cy) = ((x0 + x1) / 2, (y0 + y1) / 2);
            let handle = |left, top, right, bottom| Handle::Rect { left, top, right, bottom };
            vec![
                (handle(true, true, false, false), (*x0, *y0)),
                (handle(false, true, false, false), (cx, *y0)),
                (handle(false, true, true, false), (*x1, *y0)),
                (handle(false, false, true, false), (*x1, cy)),
                (handle(false, false, true, true), (*x1, *y1)),
                (handle(false, false, false, true), (cx, *y1)),
                (handle(true, false, false, true), (*x0, *y1)),
                (handle(true, false, false, false), (*x0, cy)),
            ]
        }
        Geometry::Circle { center, radius } => vec![(Handle::Radius, (center.0 + radius, center.1))],
        Geometry::Ellipse { center, rx, ry } => vec![
            (Handle::EllipseX, (center.0 + rx, center.1)),
            (Handle::EllipseX, (center.0 - rx, center.1)),
            (Handle::EllipseY, (center.0, center.1 + ry)),
            (Handle::EllipseY, (center.0, center.1 - ry)),
        ],
    }
}

/// The frame drawn around the selection.
pub fn bounds(geometry: &Geometry) -> (i32, i32, i32, i32) {
    match geometry {
        Geometry::Points(points) => {
            let xs = points.iter().map(|point| point.0);
            let ys = points.iter().map(|point| point.1);
            (
                xs.clone().min().unwrap_or(0),
                ys.clone().min().unwrap_or(0),
                xs.max().unwrap_or(0),
                ys.max().unwrap_or(0),
            )
        }
        Geometry::Rect { x0, y0, x1, y1 } => (*x0, *y0, *x1, *y1),
        Geometry::Text { anchor, width, height } => (anchor.0, anchor.1, anchor.0 + width - 1, anchor.1 + height - 1),
        Geometry::Circle { center, radius } => (center.0 - radius, center.1 - radius, center.0 + radius, center.1 + radius),
        Geometry::Ellipse { center, rx, ry } => (center.0 - rx, center.1 - ry, center.0 + rx, center.1 + ry),
    }
}

/// The shape moved by `dx`, `dy`, as far as it stays on the canvas.
pub fn translate(geometry: &Geometry, dx: i32, dy: i32) -> Geometry {
    // The whole shape stays on the canvas, so moving never distorts it.
    let (x0, y0, x1, y1) = bounds(geometry);
    let dx = dx.clamp(-x0.max(0), (MAX_X - x1).max(0));
    let dy = dy.clamp(-y0.max(0), (MAX_Y - y1).max(0));
    match geometry {
        Geometry::Points(points) => Geometry::Points(points.iter().map(|(x, y)| (x + dx, y + dy)).collect()),
        Geometry::Text { anchor, width, height } => Geometry::Text {
            anchor: (anchor.0 + dx, anchor.1 + dy),
            width: *width,
            height: *height,
        },
        Geometry::Rect { x0, y0, x1, y1 } => Geometry::Rect {
            x0: x0 + dx,
            y0: y0 + dy,
            x1: x1 + dx,
            y1: y1 + dy,
        },
        Geometry::Circle { center, radius } => Geometry::Circle {
            center: (center.0 + dx, center.1 + dy),
            radius: *radius,
        },
        Geometry::Ellipse { center, rx, ry } => Geometry::Ellipse {
            center: (center.0 + dx, center.1 + dy),
            rx: *rx,
            ry: *ry,
        },
    }
}

/// The geometry after dragging `handle` from `start` to `now`, starting from `original`.
pub fn drag(original: &Geometry, handle: Handle, start: Point, now: Point) -> Geometry {
    let now = (now.0.clamp(0, MAX_X), now.1.clamp(0, MAX_Y));
    match (original, handle) {
        (_, Handle::Move) => translate(original, now.0 - start.0, now.1 - start.1),
        (Geometry::Points(points), Handle::Point(index)) => {
            let mut points = points.clone();
            if let Some(point) = points.get_mut(index) {
                *point = now;
            }
            Geometry::Points(points)
        }
        (Geometry::Rect { x0, y0, x1, y1 }, Handle::Rect { left, top, right, bottom }) => {
            let (mut l, mut t, mut r, mut b) = (*x0, *y0, *x1, *y1);
            if left {
                l = now.0;
            }
            if right {
                r = now.0;
            }
            if top {
                t = now.1;
            }
            if bottom {
                b = now.1;
            }
            Geometry::Rect {
                x0: l.min(r),
                y0: t.min(b),
                x1: l.max(r),
                y1: t.max(b),
            }
        }
        (Geometry::Circle { center, .. }, Handle::Radius) => Geometry::Circle {
            center: *center,
            radius: (((now.0 - center.0).pow(2) + (now.1 - center.1).pow(2)) as f32).sqrt().round() as i32,
        },
        (Geometry::Ellipse { center, ry, .. }, Handle::EllipseX) => Geometry::Ellipse {
            center: *center,
            rx: (now.0 - center.0).abs(),
            ry: *ry,
        },
        (Geometry::Ellipse { center, rx, .. }, Handle::EllipseY) => Geometry::Ellipse {
            center: *center,
            rx: *rx,
            ry: (now.1 - center.1).abs(),
        },
        _ => original.clone(),
    }
}

fn segment_distance(point: (f32, f32), a: Point, b: Point) -> f32 {
    let (ax, ay, bx, by) = (a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32);
    let (dx, dy) = (bx - ax, by - ay);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((point.0 - ax) * dx + (point.1 - ay) * dy) / length).clamp(0.0, 1.0)
    };
    ((point.0 - ax - t * dx).powi(2) + (point.1 - ay - t * dy).powi(2)).sqrt()
}

fn bezier_points(points: &[Point]) -> Vec<Point> {
    (0..=48)
        .map(|step| {
            let t = step as f32 / 48.0;
            let u = 1.0 - t;
            let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            let x: f32 = points.iter().zip(weights).map(|(point, weight)| point.0 as f32 * weight).sum();
            let y: f32 = points.iter().zip(weights).map(|(point, weight)| point.1 as f32 * weight).sum();
            (x.round() as i32, y.round() as i32)
        })
        .collect()
}

fn angle_hit(command: &RipCommand, dx: f32, dy: f32) -> bool {
    let (start, end) = match command {
        RipCommand::Arc { st_ang, end_ang, .. }
        | RipCommand::OvalArc { st_ang, end_ang, .. }
        | RipCommand::PieSlice { st_ang, end_ang, .. }
        | RipCommand::OvalPieSlice { st_ang, end_ang, .. } => (*st_ang, *end_ang),
        _ => return true,
    };
    if end.abs_diff(start) >= 360 {
        return true;
    }
    let angle = (-dy).atan2(dx).to_degrees().rem_euclid(360.0);
    let (start, end) = ((start % 360) as f32, (end % 360) as f32);
    if start <= end {
        angle >= start && angle <= end
    } else {
        angle >= start || angle <= end
    }
}

/// Whether `point` (scene pixels) is on the shape; outlines are hit within `tolerance`.
pub fn hit(command: &RipCommand, geometry: &Geometry, point: (f32, f32), tolerance: f32) -> bool {
    let filled = matches!(
        command,
        RipCommand::Bar { .. }
            | RipCommand::FilledOval { .. }
            | RipCommand::Button { .. }
            | RipCommand::Mouse { .. }
            | RipCommand::FilledPolygon { .. }
            | RipCommand::PieSlice { .. }
            | RipCommand::OvalPieSlice { .. }
    );
    match geometry {
        Geometry::Points(points) => match command {
            RipCommand::Bezier { .. } => bezier_points(points)
                .windows(2)
                .any(|pair| segment_distance(point, pair[0], pair[1]) <= tolerance),
            RipCommand::Polygon { .. } | RipCommand::FilledPolygon { .. } | RipCommand::PolyLine { .. } => {
                let edge = points.windows(2).any(|pair| segment_distance(point, pair[0], pair[1]) <= tolerance);
                let closed = !matches!(command, RipCommand::PolyLine { .. });
                edge || (closed && points.len() > 2 && segment_distance(point, *points.last().unwrap(), points[0]) <= tolerance)
                    || (filled
                        && points
                            .iter()
                            .zip(points.iter().cycle().skip(1))
                            .take(points.len())
                            .fold(false, |inside, (a, b)| {
                                let (ay, by) = (a.1 as f32, b.1 as f32);
                                inside ^ ((ay > point.1) != (by > point.1) && point.0 < (b.0 - a.0) as f32 * (point.1 - ay) / (by - ay) + a.0 as f32)
                            }))
            }
            _ if points.len() == 1 => segment_distance(point, points[0], points[0]) <= tolerance,
            _ => points.windows(2).any(|pair| segment_distance(point, pair[0], pair[1]) <= tolerance),
        },
        Geometry::Text { .. } => {
            let (x0, y0, x1, y1) = bounds(geometry);
            point.0 >= x0 as f32 - tolerance
                && point.0 <= x1 as f32 + 1.0 + tolerance
                && point.1 >= y0 as f32 - tolerance
                && point.1 <= y1 as f32 + 1.0 + tolerance
        }
        Geometry::Rect { x0, y0, x1, y1 } => {
            let (x0, y0, x1, y1) = (*x0 as f32, *y0 as f32, *x1 as f32, *y1 as f32);
            let inside = |margin: f32| point.0 >= x0 - margin && point.0 <= x1 + margin && point.1 >= y0 - margin && point.1 <= y1 + margin;
            if filled {
                inside(tolerance)
            } else {
                inside(tolerance) && !(point.0 > x0 + tolerance && point.0 < x1 - tolerance && point.1 > y0 + tolerance && point.1 < y1 - tolerance)
            }
        }
        Geometry::Circle { center, radius } => {
            let (dx, dy) = (point.0 - center.0 as f32, point.1 - center.1 as f32);
            let distance = dx.hypot(dy);
            (distance - *radius as f32).abs() <= tolerance && angle_hit(command, dx, dy) || (filled && distance <= *radius as f32 && angle_hit(command, dx, dy))
        }
        Geometry::Ellipse { center, rx, ry } => {
            let (rx, ry) = ((*rx).max(1) as f32, (*ry).max(1) as f32);
            let (dx, dy) = (point.0 - center.0 as f32, point.1 - center.1 as f32);
            let normalized = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
            if filled {
                normalized <= 1.0 + tolerance / rx.min(ry) && angle_hit(command, dx / rx, dy / ry)
            } else {
                (normalized - 1.0).abs() * rx.min(ry) <= tolerance && angle_hit(command, dx / rx, dy / ry)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_moves_as_a_whole_and_stays_within_the_canvas() {
        let command = RipCommand::TextXY {
            x: 100,
            y: 100,
            text: "Hello".into(),
        };
        for (width, height) in [(120, 24), (24, 120)] {
            let geometry = Geometry::Text {
                anchor: (100, 100),
                width,
                height,
            };
            let moved = drag(&geometry, Handle::Move, (100, 100), (639, 349));
            assert_eq!(bounds(&moved), (640 - width, 350 - height, 639, 349));
            assert_eq!(
                apply(&command, &moved, (0, 0)),
                RipCommand::TextXY {
                    x: (640 - width) as u16,
                    y: (350 - height) as u16,
                    text: "Hello".into(),
                }
            );
            assert_eq!(translate(&moved, -1000, -1000), Geometry::Text { anchor: (0, 0), width, height });
            assert!(handles(&command, &geometry).is_empty());
        }
    }

    #[test]
    fn new_shapes_select_resize_and_respect_arc_angles() {
        let polygon = RipCommand::FilledPolygon {
            points: vec![20, 20, 100, 20, 80, 100],
        };
        let poly = geometry(&polygon, (0, 0)).unwrap();
        assert_eq!(handles(&polygon, &poly).len(), 3);
        assert!(hit(&polygon, &poly, (60.0, 40.0), 2.0));
        let changed = drag(&poly, Handle::Point(1), (100, 20), (110, 25));
        assert_eq!(
            apply(&polygon, &changed, (0, 0)),
            RipCommand::FilledPolygon {
                points: vec![20, 20, 110, 25, 80, 100]
            }
        );

        let line = RipCommand::PolyLine {
            points: vec![20, 20, 100, 20, 80, 100],
        };
        assert!(!hit(&line, &geometry(&line, (0, 0)).unwrap(), (60.0, 40.0), 2.0));
        let arc = RipCommand::Arc {
            x: 100,
            y: 100,
            st_ang: 0,
            end_ang: 90,
            radius: 30,
        };
        let circle = geometry(&arc, (0, 0)).unwrap();
        assert!(hit(&arc, &circle, (130.0, 100.0), 2.0));
        assert!(!hit(&arc, &circle, (70.0, 100.0), 2.0));
        let expanded = drag(&circle, Handle::Radius, (130, 100), (140, 100));
        assert_eq!(
            apply(&arc, &expanded, (0, 0)),
            RipCommand::Arc {
                x: 100,
                y: 100,
                st_ang: 0,
                end_ang: 90,
                radius: 40
            }
        );
    }

    #[test]
    fn rectangles_resize_from_every_handle_and_move_as_a_whole() {
        let command = RipCommand::Rectangle {
            x0: 10,
            y0: 20,
            x1: 50,
            y1: 60,
        };
        let geometry = geometry(&command, (0, 0)).unwrap();
        let handles = handles(&command, &geometry);
        assert_eq!(handles.len(), 8);
        let bottom_right = handles.iter().find(|(_, point)| *point == (50, 60)).unwrap().0;
        let resized = drag(&geometry, bottom_right, (50, 60), (80, 90));
        assert_eq!(
            apply(&command, &resized, (0, 0)),
            RipCommand::Rectangle {
                x0: 10,
                y0: 20,
                x1: 80,
                y1: 90
            }
        );
        // Dragging past the opposite edge flips the rectangle instead of inverting it.
        let top = handles.iter().find(|(_, point)| *point == (30, 20)).unwrap().0;
        assert_eq!(
            drag(&geometry, top, (30, 20), (30, 70)),
            Geometry::Rect {
                x0: 10,
                y0: 60,
                x1: 50,
                y1: 70
            }
        );
        let moved = drag(&geometry, Handle::Move, (30, 30), (35, 25));
        assert_eq!(
            moved,
            Geometry::Rect {
                x0: 15,
                y0: 15,
                x1: 55,
                y1: 55
            }
        );
        let clamped = drag(&geometry, Handle::Move, (30, 30), (0, 0));
        assert_eq!(bounds(&clamped).0, 0, "moving stops at the canvas edge without distorting");
        assert_eq!(bounds(&clamped).2 - bounds(&clamped).0, 40);
    }

    #[test]
    fn outlines_are_hit_near_their_line_and_filled_shapes_inside() {
        let outline = RipCommand::Rectangle {
            x0: 10,
            y0: 10,
            x1: 100,
            y1: 100,
        };
        let bar = RipCommand::Bar {
            x0: 10,
            y0: 10,
            x1: 100,
            y1: 100,
        };
        for (command, center) in [(&outline, false), (&bar, true)] {
            let geometry = geometry(command, (0, 0)).unwrap();
            assert!(hit(command, &geometry, (11.0, 50.0), 3.0));
            assert_eq!(hit(command, &geometry, (55.0, 55.0), 3.0), center);
            assert!(!hit(command, &geometry, (150.0, 50.0), 3.0));
        }
        let line = RipCommand::Line {
            x0: 0,
            y0: 0,
            x1: 100,
            y1: 100,
        };
        let geometry = geometry(&line, (0, 0)).unwrap();
        assert!(hit(&line, &geometry, (50.0, 52.0), 3.0));
        assert!(!hit(&line, &geometry, (50.0, 60.0), 3.0));
        let curve = RipCommand::Bezier {
            x1: 0,
            y1: 100,
            x2: 0,
            y2: 0,
            x3: 100,
            y3: 0,
            x4: 100,
            y4: 100,
            cnt: 32,
        };
        let geometry = super::geometry(&curve, (0, 0)).unwrap();
        assert!(hit(&curve, &geometry, (50.0, 25.0), 3.0), "the curve, not its control polygon");
        assert!(!hit(&curve, &geometry, (50.0, 2.0), 3.0));
        let ellipse = RipCommand::Oval {
            x: 100,
            y: 100,
            st_ang: 0,
            end_ang: 360,
            x_rad: 50,
            y_rad: 20,
        };
        let geometry = super::geometry(&ellipse, (0, 0)).unwrap();
        assert!(hit(&ellipse, &geometry, (150.0, 100.0), 3.0));
        assert!(!hit(&ellipse, &geometry, (100.0, 100.0), 3.0));
    }

    #[test]
    fn circles_ellipses_lines_and_curves_have_their_own_handles() {
        let circle = RipCommand::Circle {
            x_center: 100,
            y_center: 100,
            radius: 20,
        };
        let geometry = geometry(&circle, (0, 0)).unwrap();
        assert_eq!(handles(&circle, &geometry), vec![(Handle::Radius, (120, 100))]);
        let bigger = drag(&geometry, Handle::Radius, (120, 100), (100, 150));
        assert!(matches!(apply(&circle, &bigger, (0, 0)), RipCommand::Circle { radius: 50, .. }));

        let oval = RipCommand::FilledOval {
            x: 100,
            y: 100,
            x_rad: 30,
            y_rad: 10,
        };
        let geometry = super::geometry(&oval, (0, 0)).unwrap();
        let wider = drag(&geometry, Handle::EllipseX, (130, 100), (60, 100));
        assert!(matches!(apply(&oval, &wider, (0, 0)), RipCommand::FilledOval { x_rad: 40, y_rad: 10, .. }));

        let line = RipCommand::Line { x0: 1, y0: 2, x1: 3, y1: 4 };
        let geometry = super::geometry(&line, (0, 0)).unwrap();
        assert_eq!(handles(&line, &geometry).len(), 2);
        let moved = drag(&geometry, Handle::Point(1), (3, 4), (30, 40));
        assert_eq!(apply(&line, &moved, (0, 0)), RipCommand::Line { x0: 1, y0: 2, x1: 30, y1: 40 });

        let pixel = RipCommand::Pixel { x: 5, y: 5 };
        assert!(handles(&pixel, &super::geometry(&pixel, (0, 0)).unwrap()).is_empty(), "pixels only move");
    }

    #[test]
    fn buttons_keep_the_style_size_until_resized() {
        let button = RipCommand::Button {
            x0: 10,
            y0: 10,
            x1: 0,
            y1: 0,
            hotkey: 0,
            flags: 0,
            res: 0,
            text: "<>OK<>".into(),
        };
        let geometry = geometry(&button, (80, 30)).unwrap();
        assert_eq!(
            geometry,
            Geometry::Rect {
                x0: 10,
                y0: 10,
                x1: 89,
                y1: 39
            }
        );
        let moved = drag(&geometry, Handle::Move, (20, 20), (60, 30));
        assert!(matches!(
            apply(&button, &moved, (80, 30)),
            RipCommand::Button {
                x0: 50,
                y0: 20,
                x1: 0,
                y1: 0,
                ..
            }
        ));
        let resized = drag(
            &geometry,
            Handle::Rect {
                left: false,
                top: false,
                right: true,
                bottom: true,
            },
            (89, 39),
            (120, 60),
        );
        assert!(matches!(
            apply(&button, &resized, (80, 30)),
            RipCommand::Button {
                x0: 10,
                y0: 10,
                x1: 120,
                y1: 60,
                ..
            }
        ));
    }
}
