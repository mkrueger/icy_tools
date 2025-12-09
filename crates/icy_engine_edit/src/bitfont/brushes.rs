//! Brush/shape drawing algorithms for BitFont editor
//!
//! Contains algorithms for drawing shapes:
//! - Lines (Bresenham's algorithm)
//! - Rectangles (outline and filled)
//! - Flood fill (4-connected BFS)

use std::collections::{HashSet, VecDeque};

// ═══════════════════════════════════════════════════════════════════════════
// Bresenham Line Algorithm
// ═══════════════════════════════════════════════════════════════════════════

/// Bresenham's line algorithm - returns points along the line
///
/// This is a classic algorithm for drawing lines on a pixel grid.
/// It produces a connected series of pixels from (x0, y0) to (x1, y1).
///
/// # Arguments
/// * `x0`, `y0` - Starting point coordinates
/// * `x1`, `y1` - Ending point coordinates
///
/// # Returns
/// A vector of (x, y) coordinates representing all pixels on the line
pub fn bresenham_line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut x = x0;
    let mut y = y0;

    loop {
        points.push((x, y));

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 {
                break;
            }
            err += dx;
            y += sy;
        }
    }

    points
}

// ═══════════════════════════════════════════════════════════════════════════
// Rectangle Algorithm
// ═══════════════════════════════════════════════════════════════════════════

/// Get points for a rectangle (outline or filled)
///
/// Works regardless of coordinate order (handles inverted rectangles).
///
/// # Arguments
/// * `x0`, `y0` - First corner coordinates
/// * `x1`, `y1` - Second corner coordinates (opposite corner)
/// * `filled` - If true, returns all interior points; if false, only the outline
///
/// # Returns
/// A vector of (x, y) coordinates representing the rectangle
pub fn rectangle_points(x0: i32, y0: i32, x1: i32, y1: i32, filled: bool) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    if filled {
        // Filled rectangle - all points inside
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                points.push((x, y));
            }
        }
    } else {
        // Outline only - top, bottom, left, right edges
        for x in min_x..=max_x {
            points.push((x, min_y)); // top
            points.push((x, max_y)); // bottom
        }
        for y in (min_y + 1)..max_y {
            points.push((min_x, y)); // left
            points.push((max_x, y)); // right
        }
    }

    points
}

// ═══════════════════════════════════════════════════════════════════════════
// Flood Fill Algorithm
// ═══════════════════════════════════════════════════════════════════════════

/// Compute flood fill points using 4-connected BFS
///
/// Returns the set of points that should be filled, without modifying any state.
/// The caller is responsible for actually setting the pixels.
///
/// # Arguments
/// * `start_x`, `start_y` - Starting point coordinates
/// * `width`, `height` - Bounds of the pixel grid
/// * `get_pixel` - Function to get the current pixel value at (x, y)
///
/// # Returns
/// A set of (x, y) coordinates that should be filled
pub fn flood_fill_points<F>(start_x: i32, start_y: i32, width: i32, height: i32, get_pixel: F) -> HashSet<(i32, i32)>
where
    F: Fn(i32, i32) -> bool,
{
    let mut result = HashSet::new();

    // Bounds check
    if start_x < 0 || start_x >= width || start_y < 0 || start_y >= height {
        return result;
    }

    // Get target value at start position
    let target_value = get_pixel(start_x, start_y);

    // BFS flood fill
    let mut queue = VecDeque::new();
    queue.push_back((start_x, start_y));
    result.insert((start_x, start_y));

    while let Some((x, y)) = queue.pop_front() {
        // Check 4-connected neighbors
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let nx = x + dx;
            let ny = y + dy;

            // Bounds check
            if nx < 0 || nx >= width || ny < 0 || ny >= height {
                continue;
            }

            // Already visited check
            if result.contains(&(nx, ny)) {
                continue;
            }

            // Only fill if neighbor has the target value
            if get_pixel(nx, ny) == target_value {
                result.insert((nx, ny));
                queue.push_back((nx, ny));
            }
        }
    }

    result
}
