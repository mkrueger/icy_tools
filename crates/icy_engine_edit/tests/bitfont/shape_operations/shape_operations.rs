//! Tests for shape drawing operations with undo support

use crate::bitfont::helpers::create_test_state;
use icy_engine_edit::bitfont::BitFontUndoState;

// ═══════════════════════════════════════════════════════════════════════════
// Line Drawing Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_draw_line_horizontal() {
    let mut state = create_test_state();
    let ch = 'A';

    // Draw horizontal line
    state.draw_line(ch, 0, 2, 5, 2, true).unwrap();

    let pixels = state.get_glyph_pixels(ch);
    for x in 0..=5 {
        assert!(pixels[2][x], "Pixel at ({}, 2) should be set", x);
    }
}

#[test]
fn test_draw_line_vertical() {
    let mut state = create_test_state();
    let ch = 'A';

    // Draw vertical line
    state.draw_line(ch, 3, 0, 3, 7, true).unwrap();

    let pixels = state.get_glyph_pixels(ch);
    for y in 0..=7 {
        assert!(pixels[y][3], "Pixel at (3, {}) should be set", y);
    }
}

#[test]
fn test_draw_line_diagonal() {
    let mut state = create_test_state();
    let ch = 'A';

    // Draw diagonal line
    state.draw_line(ch, 0, 0, 7, 7, true).unwrap();

    let pixels = state.get_glyph_pixels(ch);
    for i in 0..8 {
        assert!(pixels[i][i], "Pixel at ({}, {}) should be set", i, i);
    }
}

#[test]
fn test_draw_line_undo() {
    let mut state = create_test_state();
    let ch = 'A';

    let original_pixels = state.get_glyph_pixels(ch).clone();

    // Draw line
    state.draw_line(ch, 0, 0, 7, 7, true).unwrap();

    // Verify line was drawn
    assert!(state.get_glyph_pixels(ch)[0][0]);

    // Undo
    state.undo().unwrap();

    // Verify pixels are restored
    assert_eq!(*state.get_glyph_pixels(ch), original_pixels);
}

#[test]
fn test_draw_line_erase() {
    let mut state = create_test_state();
    let ch = 'A';

    // First fill row 2
    for x in 0..8 {
        state.set_pixel(ch, x, 2, true).unwrap();
    }

    // Draw line with value=false to erase part of it
    state.draw_line(ch, 2, 2, 5, 2, false).unwrap();

    let pixels = state.get_glyph_pixels(ch);
    assert!(pixels[2][0]); // untouched
    assert!(pixels[2][1]); // untouched
    assert!(!pixels[2][2]); // erased
    assert!(!pixels[2][3]); // erased
    assert!(!pixels[2][4]); // erased
    assert!(!pixels[2][5]); // erased
    assert!(pixels[2][6]); // untouched
    assert!(pixels[2][7]); // untouched
}

// ═══════════════════════════════════════════════════════════════════════════
// Rectangle Drawing Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_draw_rectangle_outline() {
    let mut state = create_test_state();
    let ch = 'A';

    // Clear the glyph first (VGA font has content in 'A')
    state.clear_glyph(ch).unwrap();

    // Draw rectangle outline
    state.draw_rectangle(ch, 1, 1, 6, 6, false, true).unwrap();

    let pixels = state.get_glyph_pixels(ch);

    // Top edge
    for x in 1..=6 {
        assert!(pixels[1][x], "Top edge at ({}, 1) should be set", x);
    }
    // Bottom edge
    for x in 1..=6 {
        assert!(pixels[6][x], "Bottom edge at ({}, 6) should be set", x);
    }
    // Left edge
    for y in 1..=6 {
        assert!(pixels[y][1], "Left edge at (1, {}) should be set", y);
    }
    // Right edge
    for y in 1..=6 {
        assert!(pixels[y][6], "Right edge at (6, {}) should be set", y);
    }
    // Interior should be empty
    for y in 2..6 {
        for x in 2..6 {
            assert!(!pixels[y][x], "Interior at ({}, {}) should NOT be set", x, y);
        }
    }
}

#[test]
fn test_draw_rectangle_filled() {
    let mut state = create_test_state();
    let ch = 'A';

    // Draw filled rectangle
    state.draw_rectangle(ch, 2, 2, 5, 5, true, true).unwrap();

    let pixels = state.get_glyph_pixels(ch);

    // All interior points should be filled
    for y in 2..=5 {
        for x in 2..=5 {
            assert!(pixels[y][x], "Point ({}, {}) should be set", x, y);
        }
    }
}

#[test]
fn test_draw_rectangle_undo() {
    let mut state = create_test_state();
    let ch = 'A';

    let original_pixels = state.get_glyph_pixels(ch).clone();

    // Draw filled rectangle
    state.draw_rectangle(ch, 0, 0, 3, 3, true, true).unwrap();

    // Undo
    state.undo().unwrap();

    // Verify pixels are restored
    assert_eq!(*state.get_glyph_pixels(ch), original_pixels);
}

// ═══════════════════════════════════════════════════════════════════════════
// Flood Fill Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_flood_fill_empty_area() {
    let mut state = create_test_state();
    let ch = 'A';

    // Clear the glyph first (VGA font has content in 'A')
    state.clear_glyph(ch).unwrap();

    // Fill entire empty glyph
    state.flood_fill(ch, 0, 0, true).unwrap();

    let pixels = state.get_glyph_pixels(ch);
    // Default font is 8x16, check all pixels
    for y in 0..16 {
        for x in 0..8 {
            assert!(pixels[y][x], "Point ({}, {}) should be set", x, y);
        }
    }
}

#[test]
fn test_flood_fill_bounded_area() {
    let mut state = create_test_state();
    let ch = 'A';

    // Clear the glyph first (VGA font has content in 'A')
    state.clear_glyph(ch).unwrap();

    // Create a boundary: vertical line at x=4 for FULL font height (16 rows for default 8x16 font)
    for y in 0..16 {
        state.set_pixel(ch, 4, y, true).unwrap();
    }

    // Fill left side
    state.flood_fill(ch, 0, 0, true).unwrap();

    let pixels = state.get_glyph_pixels(ch);

    // Left side (x < 4) should be filled for all rows
    for y in 0..16 {
        for x in 0..4 {
            assert!(pixels[y][x], "Left side ({}, {}) should be set", x, y);
        }
    }

    // Right side (x > 4) should be empty for all rows
    for y in 0..16 {
        for x in 5..8 {
            assert!(!pixels[y][x], "Right side ({}, {}) should NOT be set", x, y);
        }
    }
}

#[test]
fn test_flood_fill_no_change_same_value() {
    let mut state = create_test_state();
    let ch = 'A';

    // Fill with true
    state.flood_fill(ch, 0, 0, true).unwrap();
    let stack_len_after_fill = state.undo_stack_len();

    // Try to fill again with true - should do nothing
    state.flood_fill(ch, 0, 0, true).unwrap();

    // Stack should be same length (no new undo operation)
    assert_eq!(state.undo_stack_len(), stack_len_after_fill);
}

#[test]
fn test_flood_fill_undo() {
    let mut state = create_test_state();
    let ch = 'A';

    let original_pixels = state.get_glyph_pixels(ch).clone();

    // Fill
    state.flood_fill(ch, 0, 0, true).unwrap();

    // Undo
    state.undo().unwrap();

    // Verify pixels are restored
    assert_eq!(*state.get_glyph_pixels(ch), original_pixels);
}

#[test]
fn test_flood_fill_out_of_bounds() {
    let mut state = create_test_state();
    let ch = 'A';

    let original_pixels = state.get_glyph_pixels(ch).clone();

    // Fill from out of bounds - should do nothing
    state.flood_fill(ch, -1, 0, true).unwrap();
    state.flood_fill(ch, 8, 0, true).unwrap();
    state.flood_fill(ch, 0, -1, true).unwrap();
    state.flood_fill(ch, 0, 8, true).unwrap();

    // Pixels should be unchanged
    assert_eq!(*state.get_glyph_pixels(ch), original_pixels);
}

#[test]
fn test_flood_fill_erase() {
    let mut state = create_test_state();
    let ch = 'A';

    // First fill everything
    for y in 0..8 {
        for x in 0..8 {
            state.set_pixel(ch, x, y, true).unwrap();
        }
    }

    // Create boundary
    for y in 0..8 {
        state.set_pixel(ch, 4, y, false).unwrap();
    }

    // Flood fill erase on left side
    state.flood_fill(ch, 0, 0, false).unwrap();

    let pixels = state.get_glyph_pixels(ch);

    // Left side should be erased
    for y in 0..8 {
        for x in 0..4 {
            assert!(!pixels[y][x], "Left side ({}, {}) should be erased", x, y);
        }
    }

    // Right side should still be filled
    for y in 0..8 {
        for x in 5..8 {
            assert!(pixels[y][x], "Right side ({}, {}) should still be set", x, y);
        }
    }
}
