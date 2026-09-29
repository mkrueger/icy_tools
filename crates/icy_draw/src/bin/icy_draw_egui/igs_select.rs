//! Selecting and reshaping IGS shapes on the canvas: hit tests, handles and the geometry that
//! handle drags change, independent of the UI.

use icy_parser_core::{BlitOperation, IgsCommand, IgsParameter, TerminalResolution};

pub type Point = (i32, i32);

/// The canvas a shape is edited on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub width: i32,
    pub height: i32,
    pub resolution: TerminalResolution,
}

impl Canvas {
    pub fn new(resolution: TerminalResolution) -> Self {
        let (width, height) = match resolution {
            TerminalResolution::Low => (320, 200),
            TerminalResolution::Medium => (640, 200),
            TerminalResolution::High => (640, 400),
        };
        Self { width, height, resolution }
    }

    /// The vertical radius VDI draws a circle of `radius` with; pixels are not square.
    pub fn circle_y_radius(&self, radius: i32) -> i32 {
        let (x, y) = match self.resolution {
            TerminalResolution::Low => (338, 372),
            TerminalResolution::Medium => (169, 372),
            TerminalResolution::High => (372, 372),
        };
        radius * x / y
    }

    /// The `y_radius` parameter of an elliptical pie slice drawn `ry` pixels tall, the inverse
    /// of [`Self::circle_y_radius`].
    pub fn circle_y_parameter(&self, ry: i32) -> i32 {
        let (x, y) = match self.resolution {
            TerminalResolution::Low => (338.0, 372.0),
            TerminalResolution::Medium => (169.0, 372.0),
            TerminalResolution::High => (372.0, 372.0),
        };
        (ry as f32 * y / x).round() as i32
    }

    /// The circle radius whose outline passes through `(dx, dy)` from the center.
    pub fn circle_radius(&self, dx: i32, dy: i32) -> i32 {
        let (x, y) = match self.resolution {
            TerminalResolution::Low => (338.0, 372.0),
            TerminalResolution::Medium => (169.0, 372.0),
            TerminalResolution::High => (372.0, 372.0),
        };
        (dx as f32).hypot(dy as f32 * y / x).round() as i32
    }
}

/// The editable geometry of a drawing command.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    /// Markers, fill seeds, line ends and polygon vertices.
    Points(Vec<Point>),
    /// Normalized so that `x0 <= x1` and `y0 <= y1`.
    Rect {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
    },
    /// A circle; `ry` is the vertical radius it is drawn with.
    Circle {
        center: Point,
        radius: i32,
        ry: i32,
    },
    Ellipse {
        center: Point,
        rx: i32,
        ry: i32,
    },
    /// Text at its anchor, with the extent of its glyphs.
    Text {
        anchor: Point,
        width: i32,
        height: i32,
        top: i32,
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

/// A fixed parameter; random and loop parameters have no position to edit.
pub fn value(parameter: &IgsParameter) -> Option<i32> {
    match parameter {
        IgsParameter::Value(value) => Some(*value),
        _ => None,
    }
}

fn rect(x0: i32, y0: i32, x1: i32, y1: i32) -> Geometry {
    Geometry::Rect {
        x0: x0.min(x1),
        y0: y0.min(y1),
        x1: x0.max(x1),
        y1: y0.max(y1),
    }
}

fn points(parameters: &[IgsParameter]) -> Option<Vec<Point>> {
    parameters.chunks_exact(2).map(|pair| Some((value(&pair[0])?, value(&pair[1])?))).collect()
}

/// Width, height and top offset of `length` glyphs at IGS text size `size`.
pub fn text_extent(size: u8, length: usize) -> (i32, i32, i32) {
    let (metrics, font) = icy_engine::igs::load_atari_font(i32::from(size));
    let glyph = font.size();
    (glyph.width * metrics.scale * length as i32, glyph.height * metrics.scale, metrics.y_off)
}

/// The geometry of a drawing command. `text_size` is the text size in effect for text.
pub fn geometry(command: &IgsCommand, canvas: &Canvas, text_size: u8) -> Option<Geometry> {
    Some(match command {
        IgsCommand::PolymarkerPlot { x, y } | IgsCommand::FloodFill { x, y } | IgsCommand::LineDrawTo { x, y } => {
            Geometry::Points(vec![(value(x)?, value(y)?)])
        }
        IgsCommand::Line { x1, y1, x2, y2 } => Geometry::Points(vec![(value(x1)?, value(y1)?), (value(x2)?, value(y2)?)]),
        IgsCommand::PolyLine { points: parameters } | IgsCommand::PolyFill { points: parameters } => Geometry::Points(points(parameters)?),
        IgsCommand::Box { x1, y1, x2, y2, .. } | IgsCommand::FilledRectangle { x1, y1, x2, y2 } | IgsCommand::RoundedRectangles { x1, y1, x2, y2, .. } => {
            rect(value(x1)?, value(y1)?, value(x2)?, value(y2)?)
        }
        IgsCommand::Circle { x, y, radius } | IgsCommand::PieSlice { x, y, radius, .. } => {
            let radius = value(radius)?;
            Geometry::Circle {
                center: (value(x)?, value(y)?),
                radius,
                ry: canvas.circle_y_radius(radius),
            }
        }
        IgsCommand::Arc { x, y, radius, .. } => {
            let radius = value(radius)?;
            Geometry::Circle {
                center: (value(x)?, value(y)?),
                radius,
                ry: radius,
            }
        }
        IgsCommand::Ellipse { x, y, x_radius, y_radius } | IgsCommand::EllipticalArc { x, y, x_radius, y_radius, .. } => Geometry::Ellipse {
            center: (value(x)?, value(y)?),
            rx: value(x_radius)?,
            ry: value(y_radius)?,
        },
        // VDI corrects the vertical radius of elliptical pie slices for the pixel aspect.
        IgsCommand::EllipticalPieSlice { x, y, x_radius, y_radius, .. } => Geometry::Ellipse {
            center: (value(x)?, value(y)?),
            rx: value(x_radius)?,
            ry: canvas.circle_y_radius(value(y_radius)?),
        },
        IgsCommand::DefineZone { zone_id, x1, y1, x2, y2, .. } if !(9997..=9999).contains(zone_id) => rect(value(x1)?, value(y1)?, value(x2)?, value(y2)?),
        IgsCommand::SprayPaint { x, y, width, height, density } if value(density)? > 0 => {
            let (x, y) = (value(x)?, value(y)?);
            rect(x, y, x + value(width)?, y + value(height)?)
        }
        // A copied area is placed by its destination; its size follows the source.
        IgsCommand::GrabScreen {
            operation:
                BlitOperation::ScreenToScreen {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                },
            ..
        } => rect(*dest_x, *dest_y, dest_x + (src_x2 - src_x1).abs(), dest_y + (src_y2 - src_y1).abs()),
        IgsCommand::WriteText { x, y, text } => {
            let (width, height, top) = text_extent(text_size, text.len().max(1));
            Geometry::Text {
                anchor: (value(x)?, value(y)?),
                width,
                height,
                top,
            }
        }
        _ => return None,
    })
}

fn set(parameter: &mut IgsParameter, value: i32) {
    *parameter = IgsParameter::Value(value);
}

/// `command` with its coordinates taken from `geometry`.
pub fn apply(command: &IgsCommand, geometry: &Geometry, canvas: &Canvas) -> IgsCommand {
    let mut command = command.clone();
    match (&mut command, geometry) {
        (IgsCommand::PolymarkerPlot { x, y } | IgsCommand::FloodFill { x, y } | IgsCommand::LineDrawTo { x, y }, Geometry::Points(points))
            if !points.is_empty() =>
        {
            set(x, points[0].0);
            set(y, points[0].1);
        }
        (IgsCommand::WriteText { x, y, .. }, Geometry::Text { anchor, .. }) => {
            set(x, anchor.0);
            set(y, anchor.1);
        }
        (IgsCommand::DefineZone { x1, y1, x2, y2, .. }, Geometry::Rect { x0, y0, x1: r, y1: b }) => {
            set(x1, *x0);
            set(y1, *y0);
            set(x2, *r);
            set(y2, *b);
        }
        (IgsCommand::SprayPaint { x, y, width, height, .. }, Geometry::Rect { x0, y0, x1: r, y1: b }) => {
            set(x, *x0);
            set(y, *y0);
            set(width, (r - x0).min(255));
            set(height, (b - y0).min(255));
        }
        (
            IgsCommand::GrabScreen {
                operation: BlitOperation::ScreenToScreen { dest_x, dest_y, .. },
                ..
            },
            Geometry::Rect { x0, y0, .. },
        ) => {
            (*dest_x, *dest_y) = (*x0, *y0);
        }
        (IgsCommand::Line { x1, y1, x2, y2 }, Geometry::Points(points)) if points.len() == 2 => {
            set(x1, points[0].0);
            set(y1, points[0].1);
            set(x2, points[1].0);
            set(y2, points[1].1);
        }
        (IgsCommand::PolyLine { points: parameters } | IgsCommand::PolyFill { points: parameters }, Geometry::Points(points))
            if parameters.len() == points.len() * 2 =>
        {
            for (pair, point) in parameters.chunks_exact_mut(2).zip(points) {
                set(&mut pair[0], point.0);
                set(&mut pair[1], point.1);
            }
        }
        (
            IgsCommand::Box { x1, y1, x2, y2, .. } | IgsCommand::FilledRectangle { x1, y1, x2, y2 } | IgsCommand::RoundedRectangles { x1, y1, x2, y2, .. },
            Geometry::Rect { x0, y0, x1: r, y1: b },
        ) => {
            set(x1, *x0);
            set(y1, *y0);
            set(x2, *r);
            set(y2, *b);
        }
        (
            IgsCommand::Circle { x, y, radius } | IgsCommand::PieSlice { x, y, radius, .. } | IgsCommand::Arc { x, y, radius, .. },
            Geometry::Circle { center, radius: r, .. },
        ) => {
            set(x, center.0);
            set(y, center.1);
            set(radius, *r);
        }
        (
            IgsCommand::Ellipse { x, y, x_radius, y_radius } | IgsCommand::EllipticalArc { x, y, x_radius, y_radius, .. },
            Geometry::Ellipse { center, rx, ry },
        ) => {
            set(x, center.0);
            set(y, center.1);
            set(x_radius, *rx);
            set(y_radius, *ry);
        }
        (IgsCommand::EllipticalPieSlice { x, y, x_radius, y_radius, .. }, Geometry::Ellipse { center, rx, ry }) => {
            set(x, center.0);
            set(y, center.1);
            set(x_radius, *rx);
            // The aspect correction rounds, so an unchanged height keeps its parameter.
            if value(y_radius).is_none_or(|parameter| canvas.circle_y_radius(parameter) != *ry) {
                set(y_radius, canvas.circle_y_parameter(*ry));
            }
        }
        _ => {}
    }
    command
}

/// Whether the shape is shown by its points rather than a frame.
pub fn points_only(command: &IgsCommand) -> bool {
    matches!(command, IgsCommand::Line { .. } | IgsCommand::PolyLine { .. } | IgsCommand::PolyFill { .. })
}

/// The handles of a shape, in canvas pixels.
pub fn handles(command: &IgsCommand, geometry: &Geometry) -> Vec<(Handle, Point)> {
    if matches!(command, IgsCommand::GrabScreen { .. }) {
        return Vec::new();
    }
    match geometry {
        Geometry::Points(points) if points_only(command) => points.iter().enumerate().map(|(index, point)| (Handle::Point(index), *point)).collect(),
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
        Geometry::Circle { center, radius, .. } => vec![(Handle::Radius, (center.0 + radius, center.1))],
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
        Geometry::Circle { center, radius, ry } => (center.0 - radius, center.1 - ry, center.0 + radius, center.1 + ry),
        Geometry::Ellipse { center, rx, ry } => (center.0 - rx, center.1 - ry, center.0 + rx, center.1 + ry),
        Geometry::Text { anchor, width, height, top } => (anchor.0, anchor.1 - top, anchor.0 + width - 1, anchor.1 - top + height - 1),
    }
}

/// The shape moved by `dx`, `dy`, as far as it stays on the canvas.
pub fn translate(geometry: &Geometry, canvas: &Canvas, dx: i32, dy: i32) -> Geometry {
    let (x0, y0, x1, y1) = bounds(geometry);
    let dx = dx.clamp(-x0.max(0), (canvas.width - 1 - x1).max(0));
    let dy = dy.clamp(-y0.max(0), (canvas.height - 1 - y1).max(0));
    match geometry {
        Geometry::Points(points) => Geometry::Points(points.iter().map(|(x, y)| (x + dx, y + dy)).collect()),
        Geometry::Rect { x0, y0, x1, y1 } => Geometry::Rect {
            x0: x0 + dx,
            y0: y0 + dy,
            x1: x1 + dx,
            y1: y1 + dy,
        },
        Geometry::Circle { center, radius, ry } => Geometry::Circle {
            center: (center.0 + dx, center.1 + dy),
            radius: *radius,
            ry: *ry,
        },
        Geometry::Ellipse { center, rx, ry } => Geometry::Ellipse {
            center: (center.0 + dx, center.1 + dy),
            rx: *rx,
            ry: *ry,
        },
        Geometry::Text { anchor, width, height, top } => Geometry::Text {
            anchor: (anchor.0 + dx, anchor.1 + dy),
            width: *width,
            height: *height,
            top: *top,
        },
    }
}

/// The geometry after dragging `handle` from `start` to `now`, starting from `original`.
pub fn drag(original: &Geometry, canvas: &Canvas, handle: Handle, start: Point, now: Point) -> Geometry {
    let now = (now.0.clamp(0, canvas.width - 1), now.1.clamp(0, canvas.height - 1));
    match (original, handle) {
        (_, Handle::Move) => translate(original, canvas, now.0 - start.0, now.1 - start.1),
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
            rect(l, t, r, b)
        }
        (Geometry::Circle { center, radius, ry }, Handle::Radius) => {
            // Arcs are drawn round in pixels, circles and pie slices corrected for the aspect.
            let square = radius == ry;
            let (dx, dy) = (now.0 - center.0, now.1 - center.1);
            let radius = if square {
                (dx as f32).hypot(dy as f32).round() as i32
            } else {
                canvas.circle_radius(dx, dy)
            };
            Geometry::Circle {
                center: *center,
                radius,
                ry: if square { radius } else { canvas.circle_y_radius(radius) },
            }
        }
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

fn inside_polygon(point: (f32, f32), points: &[Point]) -> bool {
    let mut inside = false;
    let mut previous = match points.last() {
        Some(point) => *point,
        None => return false,
    };
    for &current in points {
        let (xi, yi, xj, yj) = (current.0 as f32, current.1 as f32, previous.0 as f32, previous.1 as f32);
        if (yi > point.1) != (yj > point.1) && point.0 < (xj - xi) * (point.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// Whether the direction `(dx, dy)` lies within the angles of an arc or slice. VDI angles are
/// parametric on the ellipse, so the offset is normalized by the radii first.
fn angle_hit(command: &IgsCommand, dx: f32, dy: f32, rx: f32, ry: f32) -> bool {
    let (start, end) = match command {
        IgsCommand::Arc { start_angle, end_angle, .. }
        | IgsCommand::PieSlice { start_angle, end_angle, .. }
        | IgsCommand::EllipticalArc { start_angle, end_angle, .. }
        | IgsCommand::EllipticalPieSlice { start_angle, end_angle, .. } => match (value(start_angle), value(end_angle)) {
            (Some(start), Some(end)) => (start, end),
            _ => return true,
        },
        _ => return true,
    };
    if (end - start).abs() >= 360 {
        return true;
    }
    let angle = (-dy / ry).atan2(dx / rx).to_degrees().rem_euclid(360.0);
    let (start, end) = (start.rem_euclid(360) as f32, end.rem_euclid(360) as f32);
    if start <= end {
        angle >= start && angle <= end
    } else {
        angle >= start || angle <= end
    }
}

/// Whether `point` (canvas pixels) is on the shape; outlines are hit within `tolerance`.
pub fn hit(command: &IgsCommand, geometry: &Geometry, point: (f32, f32), tolerance: f32) -> bool {
    let outline = matches!(command, IgsCommand::Arc { .. } | IgsCommand::EllipticalArc { .. });
    match geometry {
        Geometry::Points(points) => match command {
            IgsCommand::PolyFill { .. } => {
                inside_polygon(point, points)
                    || points
                        .iter()
                        .zip(points.iter().cycle().skip(1))
                        .any(|(a, b)| segment_distance(point, *a, *b) <= tolerance)
            }
            _ if points.len() == 1 => segment_distance(point, points[0], points[0]) <= tolerance.max(3.0),
            _ => points.windows(2).any(|pair| segment_distance(point, pair[0], pair[1]) <= tolerance),
        },
        Geometry::Rect { .. } | Geometry::Text { .. } => {
            let (x0, y0, x1, y1) = bounds(geometry);
            point.0 >= x0 as f32 - tolerance
                && point.0 <= x1 as f32 + 1.0 + tolerance
                && point.1 >= y0 as f32 - tolerance
                && point.1 <= y1 as f32 + 1.0 + tolerance
        }
        Geometry::Circle { center, radius, ry } | Geometry::Ellipse { center, rx: radius, ry } => {
            let (dx, dy) = (point.0 - center.0 as f32 - 0.5, point.1 - center.1 as f32 - 0.5);
            let (rx, ry) = ((*radius).max(1) as f32, (*ry).max(1) as f32);
            let distance = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
            let band = tolerance / rx.min(ry);
            if outline {
                (distance - 1.0).abs() <= band && angle_hit(command, dx, dy, rx, ry)
            } else {
                distance <= 1.0 + band && angle_hit(command, dx, dy, rx, ry)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> Canvas {
        Canvas::new(TerminalResolution::Medium)
    }

    #[test]
    fn rectangles_resize_and_move_within_the_canvas() {
        let command = IgsCommand::Box {
            x1: 10.into(),
            y1: 20.into(),
            x2: 50.into(),
            y2: 60.into(),
            rounded: false,
        };
        let geometry = geometry(&command, &canvas(), 9).unwrap();
        let handle = Handle::Rect {
            left: false,
            top: false,
            right: true,
            bottom: true,
        };
        let resized = apply(&command, &drag(&geometry, &canvas(), handle, (50, 60), (100, 150)), &canvas());
        assert!(matches!(
            resized,
            IgsCommand::Box {
                x2: IgsParameter::Value(100),
                y2: IgsParameter::Value(150),
                ..
            }
        ));
        let moved = translate(&geometry, &canvas(), 1000, 1000);
        assert_eq!(bounds(&moved), (599, 159, 639, 199));
    }

    #[test]
    fn circles_follow_the_pixel_aspect() {
        let command = IgsCommand::Circle {
            x: 100.into(),
            y: 100.into(),
            radius: 40.into(),
        };
        let geometry = geometry(&command, &canvas(), 9).unwrap();
        assert_eq!(
            geometry,
            Geometry::Circle {
                center: (100, 100),
                radius: 40,
                ry: 18
            }
        );
        assert!(hit(&command, &geometry, (100.0, 115.0), 1.0));
        assert!(!hit(&command, &geometry, (100.0, 130.0), 1.0));
        let dragged = drag(&geometry, &canvas(), Handle::Radius, (140, 100), (150, 100));
        assert!(matches!(dragged, Geometry::Circle { radius: 50, .. }));
    }

    #[test]
    fn copied_areas_move_by_their_destination_and_zones_resize() {
        let copy = IgsCommand::GrabScreen {
            operation: BlitOperation::ScreenToScreen {
                src_x1: 10,
                src_y1: 10,
                src_x2: 40,
                src_y2: 30,
                dest_x: 100,
                dest_y: 50,
            },
            mode: icy_parser_core::BlitMode::Replace,
        };
        let copied = geometry(&copy, &canvas(), 9).unwrap();
        assert_eq!(bounds(&copied), (100, 50, 130, 70));
        assert!(handles(&copy, &copied).is_empty());
        let moved = apply(&copy, &translate(&copied, &canvas(), 5, 5), &canvas());
        assert!(matches!(
            moved,
            IgsCommand::GrabScreen {
                operation: BlitOperation::ScreenToScreen {
                    dest_x: 105,
                    dest_y: 55,
                    src_x1: 10,
                    ..
                },
                ..
            }
        ));

        let zone = IgsCommand::DefineZone {
            zone_id: 1,
            x1: 10.into(),
            y1: 10.into(),
            x2: 50.into(),
            y2: 30.into(),
            length: 3,
            string: b"MSG".to_vec(),
        };
        let area = geometry(&zone, &canvas(), 9).unwrap();
        assert_eq!(handles(&zone, &area).len(), 8);
        assert!(hit(&zone, &area, (20.0, 20.0), 1.0));
    }

    #[test]
    fn random_parameters_are_not_editable() {
        let command = IgsCommand::Line {
            x1: IgsParameter::Random,
            y1: 0.into(),
            x2: 1.into(),
            y2: 1.into(),
        };
        assert!(geometry(&command, &canvas(), 9).is_none());
    }

    #[test]
    fn polygons_are_hit_inside_and_arcs_on_their_outline() {
        let polygon = IgsCommand::PolyFill {
            points: [0, 0, 100, 0, 50, 80].map(IgsParameter::Value).to_vec(),
        };
        let geometry_polygon = geometry(&polygon, &canvas(), 9).unwrap();
        assert!(hit(&polygon, &geometry_polygon, (50.0, 30.0), 1.0));
        assert!(!hit(&polygon, &geometry_polygon, (5.0, 70.0), 1.0));

        let arc = IgsCommand::Arc {
            x: 100.into(),
            y: 100.into(),
            radius: 50.into(),
            start_angle: 0.into(),
            end_angle: 90.into(),
        };
        let geometry_arc = geometry(&arc, &canvas(), 9).unwrap();
        assert!(hit(&arc, &geometry_arc, (135.8, 64.8), 2.0));
        assert!(!hit(&arc, &geometry_arc, (100.5, 100.5), 2.0));
        assert!(!hit(&arc, &geometry_arc, (64.8, 135.8), 2.0));
    }

    #[test]
    fn elliptical_pie_slices_follow_the_aspect_and_parametric_angles() {
        let slice = IgsCommand::EllipticalPieSlice {
            x: 200.into(),
            y: 100.into(),
            x_radius: 100.into(),
            y_radius: 88.into(),
            start_angle: 45.into(),
            end_angle: 90.into(),
        };
        let geometry = geometry(&slice, &canvas(), 9).unwrap();
        assert_eq!(
            geometry,
            Geometry::Ellipse {
                center: (200, 100),
                rx: 100,
                ry: 39
            }
        );
        assert_eq!(apply(&slice, &geometry, &canvas()), slice);
        // VDI angles are parametric: 60° lies at only about 35° on screen for this flat ellipse.
        let at = |degrees: f32| {
            let radians = degrees.to_radians();
            (200.5 + 100.0 * 0.6 * radians.cos(), 100.5 - 39.0 * 0.6 * radians.sin())
        };
        assert!(hit(&slice, &geometry, at(60.0), 1.0));
        assert!(!hit(&slice, &geometry, at(30.0), 1.0));
        let taller = drag(&geometry, &canvas(), Handle::EllipseY, (200, 61), (200, 50));
        assert!(matches!(
            apply(&slice, &taller, &canvas()),
            IgsCommand::EllipticalPieSlice {
                y_radius: IgsParameter::Value(110),
                ..
            }
        ));
    }
}
