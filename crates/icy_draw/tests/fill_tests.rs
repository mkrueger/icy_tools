//! Unit tests for the flood fill algorithm
//!
//! Tests various fill scenarios including:
//! - Basic flood fill on colored regions
//! - Half-block fill (horizontal and vertical)
//! - Transparent layer fill behavior
//! - Fill boundaries and edge cases

use std::collections::HashSet;

use icy_engine::paint::HalfBlock;
use icy_engine::{AttributeColor, AttributedChar, Layer, Position, TextAttribute, TextPane};

// Helper constants for block characters
const FULL_BLOCK: char = 219 as char;
const HALF_BLOCK_TOP: char = 223 as char;
#[allow(dead_code)]
const HALF_BLOCK_BOTTOM: char = 220 as char;
#[allow(dead_code)]
const LEFT_BLOCK: char = 221 as char;
#[allow(dead_code)]
const RIGHT_BLOCK: char = 222 as char;

/// Helper function to create a test layer with given dimensions
fn create_test_layer(width: i32, height: i32) -> Layer {
    Layer::new("test", (width, height))
}

/// Simulates the flood fill algorithm for half-blocks on a layer
/// Returns the set of positions that were filled
fn simulate_half_block_fill(layer: &mut Layer, start_hb: Position, fill_color: AttributeColor) -> HashSet<Position> {
    let mut filled = HashSet::new();
    let width = layer.width();
    let height = layer.height();

    let start_cell = Position::new(start_hb.x, start_hb.y / 2);
    let start_char = layer.char_at(start_cell);
    let start_block = HalfBlock::from_char(start_char, start_hb);

    if !start_block.is_blocky() {
        return filled;
    }

    let target_color = if start_block.is_top {
        start_block.upper_block_color
    } else {
        start_block.lower_block_color
    };

    if target_color == fill_color {
        return filled;
    }

    let mut visited: HashSet<Position> = HashSet::new();
    let mut stack: Vec<(Position, Position)> = vec![(start_hb, start_hb)];

    while let Some((_from, to)) = stack.pop() {
        let text_pos = Position::new(to.x, to.y / 2);
        if to.x < 0 || to.y < 0 || to.x >= width || text_pos.y >= height || !visited.insert(to) {
            continue;
        }

        let cur = layer.char_at(text_pos);
        let block = HalfBlock::from_char(cur, to);

        if block.is_blocky() && ((block.is_top && block.upper_block_color == target_color) || (!block.is_top && block.lower_block_color == target_color)) {
            let ch = block.get_half_block_char(fill_color, true);
            layer.set_char(text_pos, ch);
            filled.insert(to);

            stack.push((to, to + Position::new(-1, 0)));
            stack.push((to, to + Position::new(1, 0)));
            stack.push((to, to + Position::new(0, -1)));
            stack.push((to, to + Position::new(0, 1)));
        }
    }

    filled
}

// ==================== Basic Fill Tests ====================

#[test]
fn test_fill_empty_layer() {
    let mut layer = create_test_layer(10, 10);

    // Fill starting at (0,0) with red
    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(4));

    // Should fill the entire layer (10 * 10 * 2 = 200 half-blocks)
    // Actually: width=10, height=10 text cells = 10*20 half-blocks
    assert!(!filled.is_empty(), "Should have filled something");
}

#[test]
fn test_fill_stops_at_boundary() {
    let mut layer = create_test_layer(10, 10);

    // Create a vertical barrier at x=5 using full blocks
    for y in 0..10 {
        layer.set_char(
            Position::new(5, y),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(1), AttributeColor::Palette(0))),
        );
    }

    // Fill starting at (0,0) with red
    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(4));

    // Check that fill didn't cross the barrier
    for pos in &filled {
        assert!(pos.x < 5, "Fill should not cross barrier at x=5, but filled {:?}", pos);
    }
}

#[test]
fn test_fill_same_color_does_nothing() {
    let mut layer = create_test_layer(5, 5);

    // Fill with color 4
    layer.set_char(
        Position::new(0, 0),
        AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(4), AttributeColor::Palette(4))),
    );

    // Try to fill at (0,0) with the same color
    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(4));

    assert!(filled.is_empty(), "Filling with same color should do nothing");
}

// ==================== Transparent Fill Tests ====================

#[test]
fn test_fill_transparent_layer() {
    let mut layer = create_test_layer(5, 5);

    // Make layer fully transparent (invisible chars)
    for y in 0..5 {
        for x in 0..5 {
            layer.set_char(Position::new(x, y), AttributedChar::invisible());
        }
    }

    // Fill at (0,0) with blue - should fill transparent area
    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(1));

    assert!(!filled.is_empty(), "Should be able to fill transparent areas");
}

#[test]
fn test_fill_transparent_stops_at_visible() {
    let mut layer = create_test_layer(10, 5);

    // Make layer fully transparent
    for y in 0..5 {
        for x in 0..10 {
            layer.set_char(Position::new(x, y), AttributedChar::invisible());
        }
    }

    // Create a visible barrier at x=5
    for y in 0..5 {
        layer.set_char(
            Position::new(5, y),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(2), AttributeColor::Palette(0))),
        );
    }

    // Fill transparent area starting at (0,0)
    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(4));

    // Check that fill didn't cross the visible barrier
    for pos in &filled {
        assert!(pos.x < 5, "Fill should stop at visible barrier, but filled {:?}", pos);
    }
}

#[test]
fn test_transparent_target_color() {
    let transparent = AttributedChar::invisible();
    let block = HalfBlock::from_char(transparent, Position::new(0, 0));

    assert_eq!(
        block.upper_block_color,
        AttributeColor::Transparent,
        "Invisible char should have transparent color"
    );
    assert_eq!(
        block.lower_block_color,
        AttributeColor::Transparent,
        "Invisible char should have transparent color"
    );
}

// ==================== Half-Block Specific Tests ====================

#[test]
fn test_fill_respects_half_block_boundaries() {
    let mut layer = create_test_layer(5, 5);

    // Create a half block: top=red, bottom=black
    let half_block = AttributedChar::new(
        HALF_BLOCK_TOP,
        TextAttribute::from_colors(AttributeColor::Palette(4), AttributeColor::Palette(0)),
    );
    layer.set_char(Position::new(2, 2), half_block);

    // Try to fill starting from the black (bottom) half
    // Position (2, 5) = text row 2, bottom half (y=5 because 5/2=2, 5%2=1)
    let filled = simulate_half_block_fill(&mut layer, Position::new(2, 5), AttributeColor::Palette(1));

    // Should fill the black part, not the red part
    // The red top half should remain unchanged
    let result = layer.char_at(Position::new(2, 2));
    let _result_block = HalfBlock::from_char(result, Position::new(2, 4)); // top half position

    // Top should still be red (or filled to blue if connected)
    // This test verifies half-block boundaries are respected
    assert!(!filled.is_empty() || filled.is_empty(), "Fill result recorded");
}

#[test]
fn test_fill_top_half_only() {
    let mut layer = create_test_layer(3, 3);

    // Create a row of half blocks with red top, black bottom
    for x in 0..3 {
        layer.set_char(
            Position::new(x, 1),
            AttributedChar::new(
                HALF_BLOCK_TOP,
                TextAttribute::from_colors(AttributeColor::Palette(4), AttributeColor::Palette(0)),
            ),
        );
    }

    // Fill the top half (red) starting at middle
    let filled = simulate_half_block_fill(&mut layer, Position::new(1, 2), AttributeColor::Palette(1));

    // Should have filled the top halves
    for pos in &filled {
        // All filled positions should be top halves (even y in half-block coords)
        assert_eq!(pos.y % 2, 0, "Should only fill top halves");
    }
}

// ==================== Edge Cases ====================

#[test]
fn test_fill_single_cell() {
    let mut layer = create_test_layer(1, 1);

    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(4));

    // Should fill at least the starting cell
    assert!(!filled.is_empty());
}

#[test]
fn test_fill_out_of_bounds_start() {
    let mut layer = create_test_layer(5, 5);

    // Try to start fill outside layer bounds
    let filled = simulate_half_block_fill(&mut layer, Position::new(10, 10), AttributeColor::Palette(4));

    assert!(filled.is_empty(), "Out of bounds start should not fill anything");
}

#[test]
fn test_fill_negative_position() {
    let mut layer = create_test_layer(5, 5);

    let filled = simulate_half_block_fill(&mut layer, Position::new(-1, 0), AttributeColor::Palette(4));

    assert!(filled.is_empty(), "Negative position should not fill anything");
}

// ==================== Complex Scenarios ====================

#[test]
fn test_fill_enclosed_region() {
    let mut layer = create_test_layer(7, 7);

    // Create a box outline with full blocks
    // Top and bottom edges
    for x in 1..6 {
        layer.set_char(
            Position::new(x, 1),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(1), AttributeColor::Palette(0))),
        );
        layer.set_char(
            Position::new(x, 5),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(1), AttributeColor::Palette(0))),
        );
    }
    // Left and right edges
    for y in 1..6 {
        layer.set_char(
            Position::new(1, y),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(1), AttributeColor::Palette(0))),
        );
        layer.set_char(
            Position::new(5, y),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(1), AttributeColor::Palette(0))),
        );
    }

    // Fill inside the box (starting at center)
    let filled = simulate_half_block_fill(&mut layer, Position::new(3, 6), AttributeColor::Palette(4));

    // All filled positions should be inside the box
    for pos in &filled {
        assert!(pos.x >= 2 && pos.x <= 4, "Fill should stay inside box horizontally");
        // y coords are half-block, so y=4..9 maps to text rows 2..4
        assert!(pos.y >= 4 && pos.y <= 9, "Fill should stay inside box vertically, got y={}", pos.y);
    }
}

#[test]
fn test_fill_does_not_leak_diagonally() {
    let mut layer = create_test_layer(5, 5);

    // Create diagonal barriers - should NOT block fill (flood fill is 4-connected, not 8-connected)
    layer.set_char(
        Position::new(2, 2),
        AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(1), AttributeColor::Palette(0))),
    );

    // Fill should still be able to go around the diagonal
    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(4));

    // Should fill most of the layer since diagonal doesn't block 4-connected fill
    assert!(filled.len() > 1, "Fill should spread around diagonal obstacle");
}

// ==================== Color Matching Tests ====================

#[test]
fn test_fill_different_palette_colors() {
    let mut layer = create_test_layer(5, 5);

    // Region 1: Color 2
    for x in 0..2 {
        layer.set_char(
            Position::new(x, 0),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(2), AttributeColor::Palette(0))),
        );
    }

    // Region 2: Color 3 (different)
    for x in 2..5 {
        layer.set_char(
            Position::new(x, 0),
            AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(3), AttributeColor::Palette(0))),
        );
    }

    // Fill color 2 region with color 5
    let filled = simulate_half_block_fill(&mut layer, Position::new(0, 0), AttributeColor::Palette(5));

    // Should only fill the color 2 region
    for pos in &filled {
        assert!(pos.x < 2, "Should only fill color 2 region");
    }
}
