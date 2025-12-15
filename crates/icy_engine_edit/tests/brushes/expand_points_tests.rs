//! Tests for brush size expansion utilities

use icy_engine::Position;
use icy_engine_edit::brushes::{PointRole, expand_points_by_brush_size, expand_points_with_role_by_brush_size};

#[test]
fn test_expand_points_by_brush_size_1() {
    let points = vec![Position::new(5, 5), Position::new(10, 10)];
    let expanded = expand_points_by_brush_size(&points, 1);
    assert_eq!(expanded, points);
}

#[test]
fn test_expand_points_by_brush_size_0() {
    let points = vec![Position::new(5, 5)];
    let expanded = expand_points_by_brush_size(&points, 0);
    assert_eq!(expanded, points);
}

#[test]
fn test_expand_points_by_brush_size_2() {
    let points = vec![Position::new(5, 5)];
    let expanded = expand_points_by_brush_size(&points, 2);
    // 2x2 = 4 points, half = 1, so offsets are -1, 0 for both x and y
    assert_eq!(expanded.len(), 4);

    let expected = vec![Position::new(4, 4), Position::new(5, 4), Position::new(4, 5), Position::new(5, 5)];
    assert_eq!(expanded, expected);
}

#[test]
fn test_expand_points_by_brush_size_3() {
    let points = vec![Position::new(5, 5)];
    let expanded = expand_points_by_brush_size(&points, 3);
    // 3x3 = 9 points centered around (5,5)
    assert_eq!(expanded.len(), 9);

    // Half = 1, so offsets are -1, 0, 1 for both x and y
    let expected = vec![
        Position::new(4, 4),
        Position::new(5, 4),
        Position::new(6, 4),
        Position::new(4, 5),
        Position::new(5, 5),
        Position::new(6, 5),
        Position::new(4, 6),
        Position::new(5, 6),
        Position::new(6, 6),
    ];
    assert_eq!(expanded, expected);
}

#[test]
fn test_expand_points_multiple_points() {
    let points = vec![Position::new(0, 0), Position::new(10, 10)];
    let expanded = expand_points_by_brush_size(&points, 2);
    // 2 points * 4 expansion = 8 points
    assert_eq!(expanded.len(), 8);
}

#[test]
fn test_expand_points_with_role_by_brush_size_1() {
    let points = vec![(Position::new(5, 5), PointRole::TopSide)];
    let expanded = expand_points_with_role_by_brush_size(&points, 1);
    assert_eq!(expanded, points);
}

#[test]
fn test_expand_points_with_role_by_brush_size_2() {
    let points = vec![(Position::new(5, 5), PointRole::TopSide)];
    let expanded = expand_points_with_role_by_brush_size(&points, 2);
    // 2x2 = 4 points, half = 1, so offsets are -1, 0 for both x and y
    assert_eq!(expanded.len(), 4);
    for (_, role) in &expanded {
        assert_eq!(*role, PointRole::TopSide);
    }
}

#[test]
fn test_expand_points_with_role_preserves_different_roles() {
    let points = vec![(Position::new(0, 0), PointRole::NWCorner), (Position::new(5, 0), PointRole::NECorner)];
    let expanded = expand_points_with_role_by_brush_size(&points, 2);
    // 2 points * 4 expansion = 8 points
    assert_eq!(expanded.len(), 8);

    // First 4 points should have NWCorner role
    for (_, role) in &expanded[0..4] {
        assert_eq!(*role, PointRole::NWCorner);
    }
    // Last 4 points should have NECorner role
    for (_, role) in &expanded[4..8] {
        assert_eq!(*role, PointRole::NECorner);
    }
}
