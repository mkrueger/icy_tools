//! Shared test helpers for BitFont tests

#![allow(dead_code)]

use icy_engine_edit::bitfont::BitFontEditState;

/// Create a test state with a simple test pattern in glyph 'A'
pub fn create_test_state() -> BitFontEditState {
    BitFontEditState::new()
}

/// Create a simple 8x16 test pattern (a diagonal line)
pub fn create_diagonal_pattern() -> Vec<Vec<bool>> {
    let mut pattern = vec![vec![false; 8]; 16];
    for i in 0..8 {
        pattern[i][i] = true;
        pattern[i + 8][i] = true;
    }
    pattern
}

/// Create a filled rectangle pattern
pub fn create_filled_rect(width: usize, height: usize, x: usize, y: usize, w: usize, h: usize) -> Vec<Vec<bool>> {
    let mut pattern = vec![vec![false; width]; height];
    for row in y..(y + h).min(height) {
        for col in x..(x + w).min(width) {
            pattern[row][col] = true;
        }
    }
    pattern
}

/// Create a horizontal line pattern at specified row
pub fn create_horizontal_line(width: usize, height: usize, row: usize) -> Vec<Vec<bool>> {
    let mut pattern = vec![vec![false; width]; height];
    if row < height {
        for col in 0..width {
            pattern[row][col] = true;
        }
    }
    pattern
}

/// Create a vertical line pattern at specified column
pub fn create_vertical_line(width: usize, height: usize, col: usize) -> Vec<Vec<bool>> {
    let mut pattern = vec![vec![false; width]; height];
    if col < width {
        for row in 0..height {
            pattern[row][col] = true;
        }
    }
    pattern
}

/// Assert that two glyph patterns are equal
pub fn assert_glyph_equals(actual: &Vec<Vec<bool>>, expected: &Vec<Vec<bool>>, message: &str) {
    assert_eq!(actual.len(), expected.len(), "{}: height mismatch", message);
    for (row_idx, (actual_row, expected_row)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(actual_row.len(), expected_row.len(), "{}: width mismatch at row {}", message, row_idx);
        for (col_idx, (actual_pixel, expected_pixel)) in actual_row.iter().zip(expected_row.iter()).enumerate() {
            assert_eq!(actual_pixel, expected_pixel, "{}: pixel mismatch at ({}, {})", message, col_idx, row_idx);
        }
    }
}

/// Count set pixels in a glyph pattern
pub fn count_pixels(pattern: &Vec<Vec<bool>>) -> usize {
    pattern.iter().flat_map(|row| row.iter()).filter(|&&p| p).count()
}

/// Print a glyph pattern for debugging (useful in test failures)
pub fn print_glyph(pattern: &Vec<Vec<bool>>) {
    for row in pattern {
        let line: String = row.iter().map(|&p| if p { '#' } else { '.' }).collect();
        println!("{}", line);
    }
}
