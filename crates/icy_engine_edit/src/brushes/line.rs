//! Line drawing algorithms
//!
//! Implements Bresenham's line algorithm for drawing lines between two points.

use icy_engine::Position;

use super::{DrawContext, DrawTarget, PointRole};

/// Generate all points on a line from p0 to p1 using Bresenham's algorithm
pub fn get_line_points(p0: Position, p1: Position) -> Vec<Position> {
    let dx = (p1.x - p0.x).abs();
    let dy = -(p1.y - p0.y).abs();
    let sx = if p0.x < p1.x { 1 } else { -1 };
    let sy = if p0.y < p1.y { 1 } else { -1 };
    let mut err = dx + dy;

    let mut x = p0.x;
    let mut y = p0.y;
    let mut points = Vec::new();

    loop {
        points.push(Position::new(x, y));

        if x == p1.x && y == p1.y {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            if x == p1.x {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == p1.y {
                break;
            }
            err += dx;
            y += sy;
        }
    }

    points
}

/// Draw a line from p0 to p1
pub fn draw_line<T: DrawTarget>(target: &mut T, ctx: &DrawContext, p0: Position, p1: Position) {
    let points = get_line_points(p0, p1);
    for pt in points {
        ctx.plot_point(target, pt, PointRole::Line);
    }
}

/// Draw a line with outline characters (for TheDraw fonts)
pub fn draw_line_outline<T: DrawTarget>(
    target: &mut T, 
    ctx: &DrawContext, 
    p0: Position, 
    p1: Position,
) {
    let points = get_line_points(p0, p1);
    let len = points.len();
    
    for (i, pt) in points.iter().enumerate() {
        let role = if len == 1 {
            // Single point - use NW corner as default
            PointRole::NWCorner
        } else if i == 0 {
            // First point
            determine_start_role(p0, p1)
        } else if i == len - 1 {
            // Last point
            determine_end_role(p0, p1)
        } else {
            // Middle points
            PointRole::Line
        };
        
        ctx.plot_point(target, *pt, role);
    }
}

fn determine_start_role(p0: Position, p1: Position) -> PointRole {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    
    if dx.abs() > dy.abs() {
        // More horizontal
        if dx > 0 {
            PointRole::LeftSide
        } else {
            PointRole::RightSide
        }
    } else {
        // More vertical
        if dy > 0 {
            PointRole::TopSide
        } else {
            PointRole::BottomSide
        }
    }
}

fn determine_end_role(p0: Position, p1: Position) -> PointRole {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    
    if dx.abs() > dy.abs() {
        // More horizontal
        if dx > 0 {
            PointRole::RightSide
        } else {
            PointRole::LeftSide
        }
    } else {
        // More vertical
        if dy > 0 {
            PointRole::BottomSide
        } else {
            PointRole::TopSide
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizontal_line() {
        let points = get_line_points(Position::new(0, 0), Position::new(5, 0));
        assert_eq!(points.len(), 6);
        assert_eq!(points[0], Position::new(0, 0));
        assert_eq!(points[5], Position::new(5, 0));
        
        // All points should have same y
        for pt in &points {
            assert_eq!(pt.y, 0);
        }
    }

    #[test]
    fn test_vertical_line() {
        let points = get_line_points(Position::new(0, 0), Position::new(0, 5));
        assert_eq!(points.len(), 6);
        assert_eq!(points[0], Position::new(0, 0));
        assert_eq!(points[5], Position::new(0, 5));
        
        // All points should have same x
        for pt in &points {
            assert_eq!(pt.x, 0);
        }
    }

    #[test]
    fn test_diagonal_line() {
        let points = get_line_points(Position::new(0, 0), Position::new(5, 5));
        assert_eq!(points.len(), 6);
        assert_eq!(points[0], Position::new(0, 0));
        assert_eq!(points[5], Position::new(5, 5));
    }

    #[test]
    fn test_single_point() {
        let points = get_line_points(Position::new(3, 3), Position::new(3, 3));
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], Position::new(3, 3));
    }

    #[test]
    fn test_negative_direction() {
        let points = get_line_points(Position::new(5, 5), Position::new(0, 0));
        assert_eq!(points.len(), 6);
        assert_eq!(points[0], Position::new(5, 5));
        assert_eq!(points[5], Position::new(0, 0));
    }

    #[test]
    fn test_steep_line() {
        // Line with slope > 1
        let points = get_line_points(Position::new(0, 0), Position::new(2, 6));
        assert_eq!(points[0], Position::new(0, 0));
        assert_eq!(points[points.len() - 1], Position::new(2, 6));
        
        // Check that y values are consecutive (no gaps)
        let mut last_y = points[0].y;
        for pt in points.iter().skip(1) {
            assert!(pt.y == last_y || pt.y == last_y + 1);
            last_y = pt.y;
        }
    }
}
