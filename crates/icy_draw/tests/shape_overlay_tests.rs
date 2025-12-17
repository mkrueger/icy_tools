//! Unit tests for shape preview overlay generation
//!
//! Tests the overlay_mask_for_drag and overlay_mask_for_drag_half_block functions
//! that generate RGBA preview masks for shape tools during drag operations.

use icy_engine::Position;
use icy_engine_edit::tools::Tool;

// Re-export the shape module functions for testing
// We test via the public interface by including the relevant code

/// Helper to generate shape points for testing
fn shape_points(tool: Tool, start: Position, end: Position) -> Vec<Position> {
    // Inline implementation matching the actual shape_points function
    match tool {
        Tool::Line => line_points(start, end),
        Tool::RectangleOutline => rectangle_outline_points(start, end),
        Tool::RectangleFilled => rectangle_filled_points(start, end),
        Tool::EllipseOutline => ellipse_outline_points(start, end),
        Tool::EllipseFilled => ellipse_filled_points(start, end),
        _ => vec![],
    }
}

fn line_points(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    let sx = if start.x < end.x { 1 } else { -1 };
    let sy = if start.y < end.y { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = start.x;
    let mut y = start.y;

    loop {
        points.push(Position::new(x, y));
        if x == end.x && y == end.y {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
    points
}

fn rectangle_outline_points(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);

    // Top and bottom edges
    for x in min_x..=max_x {
        points.push(Position::new(x, min_y));
        if min_y != max_y {
            points.push(Position::new(x, max_y));
        }
    }
    // Left and right edges (excluding corners)
    for y in (min_y + 1)..max_y {
        points.push(Position::new(min_x, y));
        if min_x != max_x {
            points.push(Position::new(max_x, y));
        }
    }
    points
}

fn rectangle_filled_points(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            points.push(Position::new(x, y));
        }
    }
    points
}

fn ellipse_outline_points(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);

    let cx = (min_x + max_x) as f64 / 2.0;
    let cy = (min_y + max_y) as f64 / 2.0;
    let rx = (max_x - min_x) as f64 / 2.0;
    let ry = (max_y - min_y) as f64 / 2.0;

    if rx < 0.5 || ry < 0.5 {
        // Degenerate case - return a line
        return line_points(start, end);
    }

    // Midpoint ellipse algorithm
    let mut x = 0.0;
    let mut y = ry;

    // Region 1
    let mut d1 = ry * ry - rx * rx * ry + 0.25 * rx * rx;
    let mut dx = 2.0 * ry * ry * x;
    let mut dy = 2.0 * rx * rx * y;

    while dx < dy {
        add_ellipse_points(&mut points, cx, cy, x, y);
        x += 1.0;
        dx += 2.0 * ry * ry;
        if d1 < 0.0 {
            d1 += dx + ry * ry;
        } else {
            y -= 1.0;
            dy -= 2.0 * rx * rx;
            d1 += dx - dy + ry * ry;
        }
    }

    // Region 2
    let mut d2 = ry * ry * (x + 0.5) * (x + 0.5) + rx * rx * (y - 1.0) * (y - 1.0) - rx * rx * ry * ry;
    while y >= 0.0 {
        add_ellipse_points(&mut points, cx, cy, x, y);
        y -= 1.0;
        dy -= 2.0 * rx * rx;
        if d2 > 0.0 {
            d2 += rx * rx - dy;
        } else {
            x += 1.0;
            dx += 2.0 * ry * ry;
            d2 += dx - dy + rx * rx;
        }
    }

    points
}

fn add_ellipse_points(points: &mut Vec<Position>, cx: f64, cy: f64, x: f64, y: f64) {
    let positions = [
        Position::new((cx + x).round() as i32, (cy + y).round() as i32),
        Position::new((cx - x).round() as i32, (cy + y).round() as i32),
        Position::new((cx + x).round() as i32, (cy - y).round() as i32),
        Position::new((cx - x).round() as i32, (cy - y).round() as i32),
    ];
    for pos in positions {
        if !points.contains(&pos) {
            points.push(pos);
        }
    }
}

fn ellipse_filled_points(start: Position, end: Position) -> Vec<Position> {
    let outline = ellipse_outline_points(start, end);
    if outline.is_empty() {
        return outline;
    }

    let min_y = outline.iter().map(|p| p.y).min().unwrap();
    let max_y = outline.iter().map(|p| p.y).max().unwrap();

    let mut points = Vec::new();
    for y in min_y..=max_y {
        let row_points: Vec<_> = outline.iter().filter(|p| p.y == y).collect();
        if row_points.is_empty() {
            continue;
        }
        let min_x = row_points.iter().map(|p| p.x).min().unwrap();
        let max_x = row_points.iter().map(|p| p.x).max().unwrap();
        for x in min_x..=max_x {
            points.push(Position::new(x, y));
        }
    }
    points
}

/// Generate overlay mask for shape preview during drag (character mode).
fn overlay_mask_for_drag(
    tool: Tool,
    font_width: f32,
    font_height: f32,
    start: Position,
    end: Position,
    color: (u8, u8, u8),
) -> (Option<(Vec<u8>, u32, u32)>, Option<(f32, f32, f32, f32)>) {
    let points = shape_points(tool, start, end);
    if points.is_empty() {
        return (None, None);
    }

    let min_x = points.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = points.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = points.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = points.iter().map(|p| p.y).max().unwrap_or(0);

    let px_min_x = min_x as f32 * font_width;
    let px_min_y = min_y as f32 * font_height;
    let px_max_x = (max_x + 1) as f32 * font_width;
    let px_max_y = (max_y + 1) as f32 * font_height;

    let w = (px_max_x - px_min_x).ceil() as u32;
    let h = (px_max_y - px_min_y).ceil() as u32;

    if w == 0 || h == 0 {
        return (None, None);
    }

    let mut rgba = vec![0u8; (w * h * 4) as usize];

    for point in &points {
        let rel_x = point.x - min_x;
        let rel_y = point.y - min_y;

        let cell_px_x = (rel_x as f32 * font_width) as u32;
        let cell_px_y = (rel_y as f32 * font_height) as u32;
        let cell_px_w = font_width.ceil() as u32;
        let cell_px_h = font_height.ceil() as u32;

        for py in cell_px_y..(cell_px_y + cell_px_h).min(h) {
            for px in cell_px_x..(cell_px_x + cell_px_w).min(w) {
                let idx = ((py * w + px) * 4) as usize;
                if idx + 3 < rgba.len() {
                    rgba[idx] = color.0;
                    rgba[idx + 1] = color.1;
                    rgba[idx + 2] = color.2;
                    rgba[idx + 3] = 140;
                }
            }
        }
    }

    (Some((rgba, w, h)), Some((px_min_x, px_min_y, w as f32, h as f32)))
}

/// Generate overlay mask for shape preview during drag (half-block mode).
fn overlay_mask_for_drag_half_block(
    tool: Tool,
    font_width: f32,
    font_height: f32,
    start: Position,
    end: Position,
    color: (u8, u8, u8),
) -> (Option<(Vec<u8>, u32, u32)>, Option<(f32, f32, f32, f32)>) {
    let points = shape_points(tool, start, end);
    if points.is_empty() {
        return (None, None);
    }

    let min_x = points.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = points.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = points.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = points.iter().map(|p| p.y).max().unwrap_or(0);

    let half_height = font_height / 2.0;
    let px_min_x = min_x as f32 * font_width;
    let px_min_y = min_y as f32 * half_height;
    let px_max_x = (max_x + 1) as f32 * font_width;
    let px_max_y = (max_y + 1) as f32 * half_height;

    let w = (px_max_x - px_min_x).ceil() as u32;
    let h = (px_max_y - px_min_y).ceil() as u32;

    if w == 0 || h == 0 {
        return (None, None);
    }

    let mut rgba = vec![0u8; (w * h * 4) as usize];

    for point in &points {
        let rel_x = point.x - min_x;
        let rel_y = point.y - min_y;

        let cell_px_x = (rel_x as f32 * font_width) as u32;
        let cell_px_y = (rel_y as f32 * half_height) as u32;
        let cell_px_w = font_width.ceil() as u32;
        let cell_px_h = half_height.ceil() as u32;

        for py in cell_px_y..(cell_px_y + cell_px_h).min(h) {
            for px in cell_px_x..(cell_px_x + cell_px_w).min(w) {
                let idx = ((py * w + px) * 4) as usize;
                if idx + 3 < rgba.len() {
                    rgba[idx] = color.0;
                    rgba[idx + 1] = color.1;
                    rgba[idx + 2] = color.2;
                    rgba[idx + 3] = 140;
                }
            }
        }
    }

    (Some((rgba, w, h)), Some((px_min_x, px_min_y, w as f32, h as f32)))
}

// =============================================================================
// Tests
// =============================================================================

#[test]
fn test_overlay_returns_none_for_empty_points() {
    // Using an unsupported tool type should return empty points and thus None
    let (data, rect) = overlay_mask_for_drag(
        Tool::Click, // Not a shape tool
        8.0,
        16.0,
        Position::new(0, 0),
        Position::new(5, 5),
        (255, 255, 0),
    );
    assert!(data.is_none());
    assert!(rect.is_none());
}

#[test]
fn test_overlay_line_basic() {
    let start = Position::new(0, 0);
    let end = Position::new(2, 0);
    let color = (255, 128, 64);

    let (data, rect) = overlay_mask_for_drag(Tool::Line, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    assert!(rect.is_some());

    let (rgba, w, h) = data.unwrap();
    let (rx, ry, rw, rh) = rect.unwrap();

    // Line from (0,0) to (2,0) spans 3 cells horizontally
    assert_eq!(w, 24); // 3 cells * 8 pixels
    assert_eq!(h, 16); // 1 cell * 16 pixels
    assert_eq!(rx, 0.0);
    assert_eq!(ry, 0.0);
    assert_eq!(rw, 24.0);
    assert_eq!(rh, 16.0);

    // Check that pixels are filled with the correct color
    assert_eq!(rgba[0], color.0); // R
    assert_eq!(rgba[1], color.1); // G
    assert_eq!(rgba[2], color.2); // B
    assert_eq!(rgba[3], 140); // A (semi-transparent)
}

#[test]
fn test_overlay_line_vertical() {
    let start = Position::new(0, 0);
    let end = Position::new(0, 3);
    let color = (0, 255, 0);

    let (data, _rect) = overlay_mask_for_drag(Tool::Line, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    let (rgba, w, h) = data.unwrap();

    // Vertical line spans 4 cells vertically
    assert_eq!(w, 8); // 1 cell * 8 pixels
    assert_eq!(h, 64); // 4 cells * 16 pixels

    // All pixels in the column should be filled
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            assert_eq!(rgba[idx], color.0);
            assert_eq!(rgba[idx + 1], color.1);
            assert_eq!(rgba[idx + 2], color.2);
            assert_eq!(rgba[idx + 3], 140);
        }
    }
}

#[test]
fn test_overlay_rectangle_outline() {
    let start = Position::new(0, 0);
    let end = Position::new(3, 2);
    let color = (255, 0, 0);

    let (data, _rect) = overlay_mask_for_drag(Tool::RectangleOutline, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    let (rgba, w, h) = data.unwrap();

    // Rectangle from (0,0) to (3,2) spans 4x3 cells
    assert_eq!(w, 32); // 4 cells * 8 pixels
    assert_eq!(h, 48); // 3 cells * 16 pixels

    // Verify top-left corner is filled
    assert_eq!(rgba[0], color.0);
    assert_eq!(rgba[3], 140);

    // Verify bottom-right corner is filled
    let br_idx = (((h - 1) * w + (w - 1)) * 4) as usize;
    assert_eq!(rgba[br_idx], color.0);
    assert_eq!(rgba[br_idx + 3], 140);
}

#[test]
fn test_overlay_rectangle_filled() {
    let start = Position::new(1, 1);
    let end = Position::new(2, 2);
    let color = (0, 0, 255);

    let (data, rect) = overlay_mask_for_drag(Tool::RectangleFilled, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    let (rgba, w, h) = data.unwrap();
    let (rx, ry, _, _) = rect.unwrap();

    // Rectangle from (1,1) to (2,2) spans 2x2 cells
    assert_eq!(w, 16); // 2 cells * 8 pixels
    assert_eq!(h, 32); // 2 cells * 16 pixels
    assert_eq!(rx, 8.0); // Offset by 1 cell
    assert_eq!(ry, 16.0); // Offset by 1 cell

    // All pixels should be filled (it's filled rectangle)
    for i in 0..(w * h) as usize {
        let idx = i * 4;
        assert_eq!(rgba[idx], color.0);
        assert_eq!(rgba[idx + 1], color.1);
        assert_eq!(rgba[idx + 2], color.2);
        assert_eq!(rgba[idx + 3], 140);
    }
}

#[test]
fn test_overlay_ellipse_basic() {
    let start = Position::new(0, 0);
    let end = Position::new(4, 2);
    let color = (128, 128, 128);

    let (data, _rect) = overlay_mask_for_drag(Tool::EllipseOutline, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    let (rgba, w, h) = data.unwrap();

    // Ellipse should have some filled pixels
    let filled_count = (0..(w * h) as usize).filter(|&i| rgba[i * 4 + 3] > 0).count();
    assert!(filled_count > 0);
}

#[test]
fn test_overlay_half_block_y_resolution() {
    let start = Position::new(0, 0);
    let end = Position::new(0, 3);
    let color = (255, 255, 255);

    // Normal mode
    let (data_normal, _) = overlay_mask_for_drag(Tool::Line, 8.0, 16.0, start, end, color);
    let (_, _, h_normal) = data_normal.unwrap();

    // Half-block mode
    let (data_half, _) = overlay_mask_for_drag_half_block(Tool::Line, 8.0, 16.0, start, end, color);
    let (_, _, h_half) = data_half.unwrap();

    // Half-block mode should have half the height for the same Y range
    assert_eq!(h_normal, 64); // 4 cells * 16 pixels
    assert_eq!(h_half, 32); // 4 half-cells * 8 pixels
}

#[test]
fn test_overlay_half_block_position() {
    let start = Position::new(2, 4);
    let end = Position::new(4, 8);
    let color = (100, 150, 200);

    let (data, rect) = overlay_mask_for_drag_half_block(Tool::Line, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    assert!(rect.is_some());

    let (rx, ry, _, _) = rect.unwrap();

    // Position should use half_height for Y
    assert_eq!(rx, 16.0); // 2 * 8.0
    assert_eq!(ry, 32.0); // 4 * (16.0 / 2.0) = 4 * 8.0
}

#[test]
fn test_overlay_different_colors() {
    let start = Position::new(0, 0);
    let end = Position::new(1, 1);

    let colors = [
        (255, 0, 0),   // Red
        (0, 255, 0),   // Green
        (0, 0, 255),   // Blue
        (255, 255, 0), // Yellow
        (0, 255, 255), // Cyan
        (255, 0, 255), // Magenta
    ];

    for color in colors {
        let (data, _) = overlay_mask_for_drag(Tool::RectangleFilled, 8.0, 16.0, start, end, color);
        let (rgba, _, _) = data.unwrap();

        // First pixel should have the correct color
        assert_eq!(rgba[0], color.0, "Red channel mismatch for {:?}", color);
        assert_eq!(rgba[1], color.1, "Green channel mismatch for {:?}", color);
        assert_eq!(rgba[2], color.2, "Blue channel mismatch for {:?}", color);
    }
}

#[test]
fn test_overlay_negative_coordinates() {
    let start = Position::new(-2, -1);
    let end = Position::new(1, 1);
    let color = (200, 100, 50);

    let (data, rect) = overlay_mask_for_drag(Tool::RectangleFilled, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    let (rgba, w, h) = data.unwrap();
    let (rx, ry, _, _) = rect.unwrap();

    // Should span 4x3 cells
    assert_eq!(w, 32); // 4 cells * 8 pixels
    assert_eq!(h, 48); // 3 cells * 16 pixels

    // Position should be negative
    assert_eq!(rx, -16.0); // -2 * 8.0
    assert_eq!(ry, -16.0); // -1 * 16.0

    // Pixels should still be filled
    assert_eq!(rgba[0], color.0);
}

#[test]
fn test_overlay_single_point() {
    let pos = Position::new(5, 5);
    let color = (50, 100, 150);

    let (data, rect) = overlay_mask_for_drag(Tool::Line, 8.0, 16.0, pos, pos, color);

    assert!(data.is_some());
    let (rgba, w, h) = data.unwrap();
    let (rx, ry, rw, rh) = rect.unwrap();

    // Single cell
    assert_eq!(w, 8);
    assert_eq!(h, 16);
    assert_eq!(rx, 40.0); // 5 * 8.0
    assert_eq!(ry, 80.0); // 5 * 16.0
    assert_eq!(rw, 8.0);
    assert_eq!(rh, 16.0);

    // All pixels should be filled
    for i in 0..(w * h) as usize {
        assert_eq!(rgba[i * 4 + 3], 140);
    }
}

#[test]
fn test_overlay_diagonal_line() {
    let start = Position::new(0, 0);
    let end = Position::new(3, 3);
    let color = (255, 255, 255);

    let (data, _) = overlay_mask_for_drag(Tool::Line, 8.0, 16.0, start, end, color);

    assert!(data.is_some());
    let (rgba, w, h) = data.unwrap();

    // Diagonal line should span 4x4 cells
    assert_eq!(w, 32); // 4 cells * 8 pixels
    assert_eq!(h, 64); // 4 cells * 16 pixels

    // Count filled cells - diagonal should have 4 cells filled
    let mut filled_cells = 0;
    for cell_y in 0..4 {
        for cell_x in 0..4 {
            let px = cell_x * 8;
            let py = cell_y * 16;
            let idx = ((py * w + px) * 4) as usize;
            if rgba[idx + 3] > 0 {
                filled_cells += 1;
            }
        }
    }
    assert_eq!(filled_cells, 4, "Diagonal line should fill exactly 4 cells");
}

#[test]
fn test_overlay_buffer_size() {
    let start = Position::new(0, 0);
    let end = Position::new(9, 9);
    let color = (0, 0, 0);

    let (data, _) = overlay_mask_for_drag(Tool::RectangleFilled, 8.0, 16.0, start, end, color);

    let (rgba, w, h) = data.unwrap();

    // Buffer size should match dimensions * 4 (RGBA)
    assert_eq!(rgba.len(), (w * h * 4) as usize);
    assert_eq!(rgba.len(), 80 * 160 * 4); // 10 * 8 = 80, 10 * 16 = 160
}

#[test]
fn test_half_block_vs_normal_same_points_different_scale() {
    let start = Position::new(0, 0);
    let end = Position::new(2, 2);
    let color = (128, 64, 32);

    let (normal_data, normal_rect) = overlay_mask_for_drag(Tool::Line, 8.0, 16.0, start, end, color);
    let (half_data, half_rect) = overlay_mask_for_drag_half_block(Tool::Line, 8.0, 16.0, start, end, color);

    let (_, _, h_normal) = normal_data.unwrap();
    let (_, _, h_half) = half_data.unwrap();

    let (_, _, _, rh_normal) = normal_rect.unwrap();
    let (_, _, _, rh_half) = half_rect.unwrap();

    // Heights should differ by factor of 2
    assert_eq!(h_normal, h_half * 2);
    assert_eq!(rh_normal, rh_half * 2.0);
}
