//! Tests for rectangle drawing algorithms

use icy_engine::{AttributedChar, Position};
use icy_engine_edit::brushes::{DrawContext, DrawTarget, draw_rectangle, get_filled_rectangle_points, get_rectangle_points};

/// Mock target for rectangle tests
struct MockTarget {
    width: i32,
    height: i32,
}

impl MockTarget {
    fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

impl DrawTarget for MockTarget {
    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn char_at(&self, _pos: Position) -> Option<AttributedChar> {
        None
    }

    fn set_char(&mut self, _pos: Position, _ch: AttributedChar) {
        // We track via plot_point
    }
}

#[test]
fn test_single_point_rectangle() {
    let ctx = DrawContext::default();
    let mut target = MockTarget::new(80, 25);

    // For a single point, we can't easily track without modifying the mock
    // This test just ensures no panic
    draw_rectangle(&mut target, &ctx, Position::new(5, 5), Position::new(5, 5));
}

#[test]
fn test_vertical_line_rectangle() {
    let ctx = DrawContext::default();
    let mut target = MockTarget::new(80, 25);

    draw_rectangle(&mut target, &ctx, Position::new(5, 0), Position::new(5, 3));
    // Should draw 4 points vertically
}

#[test]
fn test_horizontal_line_rectangle() {
    let ctx = DrawContext::default();
    let mut target = MockTarget::new(80, 25);

    draw_rectangle(&mut target, &ctx, Position::new(0, 5), Position::new(3, 5));
    // Should draw 4 points horizontally
}

#[test]
fn test_rectangle_outline_count() {
    // A 4x3 rectangle (from (0,0) to (3,2)) should have:
    // - Top row: 4 points (x=0,1,2,3 at y=0)
    // - Bottom row: 4 points (x=0,1,2,3 at y=2)
    // - Left side (excluding corners): 1 point (x=0 at y=1)
    // - Right side (excluding corners): 1 point (x=3 at y=1)
    // Total: 10 points
    let points = get_rectangle_points(Position::new(0, 0), Position::new(3, 2));
    assert_eq!(points.len(), 10);
}

#[test]
fn test_filled_rectangle_count() {
    // A 4x3 filled rectangle should have 12 points (4 * 3)
    let points = get_filled_rectangle_points(Position::new(0, 0), Position::new(3, 2));
    assert_eq!(points.len(), 12);
}

#[test]
fn test_rectangle_points_contains_corners() {
    let points = get_rectangle_points(Position::new(0, 0), Position::new(5, 3));

    let positions: Vec<_> = points.iter().map(|(p, _)| *p).collect();

    // Check corners exist
    assert!(positions.contains(&Position::new(0, 0)), "Should have top-left corner");
    assert!(positions.contains(&Position::new(5, 0)), "Should have top-right corner");
    assert!(positions.contains(&Position::new(0, 3)), "Should have bottom-left corner");
    assert!(positions.contains(&Position::new(5, 3)), "Should have bottom-right corner");
}
