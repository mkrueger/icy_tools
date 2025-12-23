//! Tests for brush algorithms (pure functions, no state)

use icy_engine_edit::bitfont::brushes::{bresenham_line, flood_fill_points, rectangle_points};

// ═══════════════════════════════════════════════════════════════════════════
// Bresenham Line Algorithm Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_bresenham_horizontal_line() {
    let points = bresenham_line(0, 0, 5, 0);
    assert_eq!(points, vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)]);
}

#[test]
fn test_bresenham_vertical_line() {
    let points = bresenham_line(0, 0, 0, 5);
    assert_eq!(points, vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (0, 5)]);
}

#[test]
fn test_bresenham_diagonal_line() {
    let points = bresenham_line(0, 0, 3, 3);
    assert_eq!(points, vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
}

#[test]
fn test_bresenham_reverse_horizontal() {
    let points = bresenham_line(5, 0, 0, 0);
    assert_eq!(points, vec![(5, 0), (4, 0), (3, 0), (2, 0), (1, 0), (0, 0)]);
}

#[test]
fn test_bresenham_reverse_vertical() {
    let points = bresenham_line(0, 5, 0, 0);
    assert_eq!(points, vec![(0, 5), (0, 4), (0, 3), (0, 2), (0, 1), (0, 0)]);
}

#[test]
fn test_bresenham_steep_line() {
    // Line with dy > dx (steep)
    let points = bresenham_line(0, 0, 2, 5);
    assert_eq!(points.len(), 6); // Should have 6 points
    assert_eq!(points[0], (0, 0));
    assert_eq!(points[points.len() - 1], (2, 5));
}

#[test]
fn test_bresenham_shallow_line() {
    // Line with dx > dy (shallow)
    let points = bresenham_line(0, 0, 5, 2);
    assert_eq!(points.len(), 6); // Should have 6 points
    assert_eq!(points[0], (0, 0));
    assert_eq!(points[points.len() - 1], (5, 2));
}

#[test]
fn test_bresenham_single_point() {
    let points = bresenham_line(3, 3, 3, 3);
    assert_eq!(points, vec![(3, 3)]);
}

#[test]
fn test_bresenham_negative_slope() {
    let points = bresenham_line(0, 3, 3, 0);
    assert_eq!(points.len(), 4);
    assert_eq!(points[0], (0, 3));
    assert_eq!(points[points.len() - 1], (3, 0));
}

#[test]
fn test_line_continuity() {
    // Verify that lines are continuous (no gaps)
    let points = bresenham_line(0, 0, 10, 7);

    for i in 1..points.len() {
        let (x1, y1) = points[i - 1];
        let (x2, y2) = points[i];
        let dx = (x2 - x1).abs();
        let dy = (y2 - y1).abs();
        // Each step should move at most 1 in each direction
        assert!(dx <= 1 && dy <= 1, "Gap detected between {:?} and {:?}", points[i - 1], points[i]);
    }
}

#[test]
fn test_line_endpoints() {
    // Verify lines always include both endpoints
    for (x0, y0, x1, y1) in [(0, 0, 5, 5), (5, 5, 0, 0), (0, 0, 10, 3), (3, 7, 8, 2)] {
        let points = bresenham_line(x0, y0, x1, y1);
        assert_eq!(points[0], (x0, y0), "First point should be start");
        assert_eq!(points[points.len() - 1], (x1, y1), "Last point should be end");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Rectangle Points Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_rectangle_outline_small() {
    let points = rectangle_points(0, 0, 2, 2, false);
    // Should contain outline only: top row, bottom row, left col (middle), right col (middle)
    assert!(points.contains(&(0, 0))); // top-left
    assert!(points.contains(&(1, 0))); // top-middle
    assert!(points.contains(&(2, 0))); // top-right
    assert!(points.contains(&(0, 2))); // bottom-left
    assert!(points.contains(&(1, 2))); // bottom-middle
    assert!(points.contains(&(2, 2))); // bottom-right
    assert!(points.contains(&(0, 1))); // left-middle
    assert!(points.contains(&(2, 1))); // right-middle
                                       // Should NOT contain interior
    assert!(!points.contains(&(1, 1))); // center should NOT be in outline
}

#[test]
fn test_rectangle_filled_small() {
    let points = rectangle_points(0, 0, 2, 2, true);
    // Should contain all 9 points (3x3)
    assert_eq!(points.len(), 9);
    for y in 0..=2 {
        for x in 0..=2 {
            assert!(points.contains(&(x, y)), "Missing point ({}, {})", x, y);
        }
    }
}

#[test]
fn test_rectangle_reverse_coords() {
    // Test that rectangle works regardless of coordinate order
    let points1 = rectangle_points(0, 0, 3, 3, true);
    let points2 = rectangle_points(3, 3, 0, 0, true);

    // Both should have same number of points
    assert_eq!(points1.len(), points2.len());

    // Both should contain all the same points (order may differ)
    for p in &points1 {
        assert!(points2.contains(p));
    }
}

#[test]
fn test_rectangle_single_point() {
    let points = rectangle_points(5, 5, 5, 5, false);
    assert_eq!(points.len(), 2); // top and bottom at same point (duplicated)

    let points_filled = rectangle_points(5, 5, 5, 5, true);
    assert_eq!(points_filled.len(), 1);
    assert_eq!(points_filled[0], (5, 5));
}

#[test]
fn test_rectangle_horizontal_line() {
    let points = rectangle_points(0, 0, 4, 0, false);
    // Should be a horizontal line of 5 points (duplicated for top/bottom)
    assert_eq!(points.len(), 10); // 5 top + 5 bottom (same row)
}

#[test]
fn test_rectangle_vertical_line() {
    let points = rectangle_points(0, 0, 0, 4, false);
    // 2 points for top (0,0), 2 for bottom (0,4), and 3 for middle rows (left+right at same x)
    // This gives 2 + 2 + 3*2 = 10 points total
    assert!(points.len() >= 5);
}

#[test]
fn test_rectangle_outline_no_interior() {
    let points = rectangle_points(0, 0, 4, 4, false);

    // Interior points (not on edges) should not be present
    assert!(!points.contains(&(1, 1)));
    assert!(!points.contains(&(2, 2)));
    assert!(!points.contains(&(3, 3)));
    assert!(!points.contains(&(1, 2)));
    assert!(!points.contains(&(2, 1)));
}

#[test]
fn test_rectangle_filled_has_interior() {
    let points = rectangle_points(0, 0, 4, 4, true);

    // All 25 points (5x5) should be present
    assert_eq!(points.len(), 25);

    // Including interior
    assert!(points.contains(&(1, 1)));
    assert!(points.contains(&(2, 2)));
    assert!(points.contains(&(3, 3)));
}

// ═══════════════════════════════════════════════════════════════════════════
// Flood Fill Algorithm Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_flood_fill_empty_grid() {
    // 4x4 grid, all false
    let grid = vec![vec![false; 4]; 4];

    let points = flood_fill_points(0, 0, 4, 4, |x, y| grid[y as usize][x as usize]);

    // Should fill entire grid (all 16 points)
    assert_eq!(points.len(), 16);
}

#[test]
fn test_flood_fill_with_boundary() {
    // 4x4 grid with vertical line at x=2
    let mut grid = vec![vec![false; 4]; 4];
    for y in 0..4 {
        grid[y][2] = true; // vertical wall
    }

    // Fill from (0,0) - should only fill left side (x=0,1)
    let points = flood_fill_points(0, 0, 4, 4, |x, y| grid[y as usize][x as usize]);

    // Should fill 8 points (4 rows * 2 columns)
    assert_eq!(points.len(), 8);

    // All points should have x < 2
    for (x, _y) in &points {
        assert!(*x < 2, "Point ({}, _) should be in left region", x);
    }
}

#[test]
fn test_flood_fill_single_pixel() {
    // 4x4 grid with single pixel filled
    let mut grid = vec![vec![false; 4]; 4];
    grid[1][1] = true;

    // Fill from (1,1) - should only fill that one pixel
    let points = flood_fill_points(1, 1, 4, 4, |x, y| grid[y as usize][x as usize]);

    assert_eq!(points.len(), 1);
    assert!(points.contains(&(1, 1)));
}

#[test]
fn test_flood_fill_bounds() {
    // Test out of bounds start position
    let grid = vec![vec![false; 4]; 4];

    let points = flood_fill_points(-1, 0, 4, 4, |x, y| grid[y as usize][x as usize]);
    assert!(points.is_empty());

    let points = flood_fill_points(4, 0, 4, 4, |x, y| grid[y as usize][x as usize]);
    assert!(points.is_empty());

    let points = flood_fill_points(0, -1, 4, 4, |x, y| grid[y as usize][x as usize]);
    assert!(points.is_empty());

    let points = flood_fill_points(0, 4, 4, 4, |x, y| grid[y as usize][x as usize]);
    assert!(points.is_empty());
}

#[test]
fn test_flood_fill_l_shape() {
    // 4x4 grid with L-shaped region of true values
    // . . . .
    // X . . .
    // X . . .
    // X X X .
    let mut grid = vec![vec![false; 4]; 4];
    grid[1][0] = true;
    grid[2][0] = true;
    grid[3][0] = true;
    grid[3][1] = true;
    grid[3][2] = true;

    // Fill from (0,1) - should get the L shape
    let points = flood_fill_points(0, 1, 4, 4, |x, y| grid[y as usize][x as usize]);

    assert_eq!(points.len(), 5);
    assert!(points.contains(&(0, 1)));
    assert!(points.contains(&(0, 2)));
    assert!(points.contains(&(0, 3)));
    assert!(points.contains(&(1, 3)));
    assert!(points.contains(&(2, 3)));
}
