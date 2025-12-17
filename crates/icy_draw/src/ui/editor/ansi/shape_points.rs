use icy_engine::Position;
use icy_engine_edit::brushes;
use icy_engine_edit::tools::Tool;

/// Generates shape points for the given tool between two positions.
///
/// Note: For HalfBlock mode, callers should pass half-block-space positions
/// (Y has 2x resolution) and interpret the returned points accordingly.
pub fn shape_points(tool: Tool, p0: Position, p1: Position) -> Vec<Position> {
    match tool {
        Tool::Line => brushes::get_line_points(p0, p1),
        Tool::RectangleOutline => brushes::get_rectangle_points(p0, p1).into_iter().map(|(p, _)| p).collect(),
        Tool::RectangleFilled => brushes::get_filled_rectangle_points(p0, p1).into_iter().map(|(p, _)| p).collect(),
        Tool::EllipseOutline => {
            use std::collections::HashSet;
            let points = brushes::get_ellipse_points_from_rect(p0, p1);
            let mut set: HashSet<(i32, i32)> = HashSet::new();
            for (p, _) in points {
                set.insert((p.x, p.y));
            }
            set.into_iter().map(|(x, y)| Position::new(x, y)).collect()
        }
        Tool::EllipseFilled => brushes::get_filled_ellipse_points_from_rect(p0, p1).into_iter().map(|(p, _)| p).collect(),
        _ => Vec::new(),
    }
}
