//! Tests for line drawing algorithms

use icy_engine::Position;
use icy_engine_edit::brushes::get_line_points;

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
