//! Tests for ellipse drawing algorithms

use icy_engine::Position;
use icy_engine_edit::brushes::get_ellipse_points;

#[test]
fn test_single_point_ellipse() {
    let points = get_ellipse_points(Position::new(10, 10), 0, 0);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].0, Position::new(10, 10));
}

#[test]
fn test_horizontal_line_ellipse() {
    // An ellipse with radius_y = 0 is essentially a horizontal line
    let points = get_ellipse_points(Position::new(10, 10), 5, 0);
    assert_eq!(points.len(), 1);
}

#[test]
fn test_small_ellipse() {
    let points = get_ellipse_points(Position::new(10, 10), 3, 2);

    // Check that we have points in all four quadrants
    let has_top = points.iter().any(|(p, _)| p.y < 10);
    let has_bottom = points.iter().any(|(p, _)| p.y > 10);
    let has_left = points.iter().any(|(p, _)| p.x < 10);
    let has_right = points.iter().any(|(p, _)| p.x > 10);

    assert!(has_top, "Ellipse should have top points");
    assert!(has_bottom, "Ellipse should have bottom points");
    assert!(has_left, "Ellipse should have left points");
    assert!(has_right, "Ellipse should have right points");
}

#[test]
fn test_circle() {
    // Equal radii should produce a circle
    let points = get_ellipse_points(Position::new(10, 10), 5, 5);

    // All points should be approximately equidistant from center
    for (pt, _) in &points {
        let dx = (pt.x - 10) as f64;
        let dy = (pt.y - 10) as f64;
        let dist = (dx * dx + dy * dy).sqrt();

        // Allow for some rasterization error
        assert!(dist >= 4.0 && dist <= 6.0, "Point {:?} has distance {} from center", pt, dist);
    }
}

#[test]
fn test_ellipse_symmetry() {
    let points = get_ellipse_points(Position::new(0, 0), 4, 3);

    // For each point (x, y), there should be points at (-x, y), (x, -y), (-x, -y)
    for (pt, _) in &points {
        if pt.x != 0 && pt.y != 0 {
            // Check for symmetric points
            let has_neg_x = points.iter().any(|(p, _)| p.x == -pt.x && p.y == pt.y);
            let has_neg_y = points.iter().any(|(p, _)| p.x == pt.x && p.y == -pt.y);
            let has_neg_both = points.iter().any(|(p, _)| p.x == -pt.x && p.y == -pt.y);

            assert!(has_neg_x, "Missing symmetric point for {:?}", pt);
            assert!(has_neg_y, "Missing symmetric point for {:?}", pt);
            assert!(has_neg_both, "Missing symmetric point for {:?}", pt);
        }
    }
}
