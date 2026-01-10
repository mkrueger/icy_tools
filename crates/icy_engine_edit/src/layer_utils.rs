//! Utility functions for Layer operations that were previously part of icy_engine
//! but have been moved here as they are specific to editing operations.

use crate::{AttributedChar, Layer, Position, Rectangle, TextPane};

/// A 2D grid of characters, indexed as [row][column].
pub type CharGrid = Vec<Vec<AttributedChar>>;

/// Creates a new Layer from a portion of an existing layer.
///
/// This extracts the characters from the specified area of the source layer
/// and creates a new layer with the extracted content.
pub fn layer_from_area(source: &Layer, area: Rectangle) -> Layer {
    let mut result = Layer::new("extracted", area.size());

    for y in area.y_range() {
        for x in area.x_range() {
            let pos = Position::new(x, y) - area.start;
            result.set_char(pos, source.char_at((x, y).into()));
        }
    }
    result
}

/// Extracts a 2D char grid from a portion of an existing layer.
///
/// The returned grid is in local coordinates (0..width, 0..height) relative
/// to `area.start`.
pub fn chars_from_area(source: &Layer, area: Rectangle) -> CharGrid {
    let width = area.size.width.max(0) as usize;
    let height = area.size.height.max(0) as usize;
    let mut result = vec![vec![AttributedChar::invisible(); width]; height];

    for y in 0..height {
        for x in 0..width {
            let src_pos = area.start + Position::new(x as i32, y as i32);
            result[y][x] = source.char_at(src_pos);
        }
    }
    result
}

/// Stamps the content of a source layer onto a target layer at the specified position.
///
/// This copies all characters from the source layer to the target layer,
/// placing them at the given target position.
pub fn stamp_layer(target: &mut Layer, target_pos: Position, source: &Layer) {
    let area = source.rectangle();
    for y in area.y_range() {
        for x in area.x_range() {
            let pos = Position::new(x, y);
            target.set_char(pos + target_pos, source.char_at(pos));
        }
    }
}

/// Stamps a 2D char grid onto a target layer at the specified position.
pub fn stamp_char_grid(target: &mut Layer, target_pos: Position, source: &[Vec<AttributedChar>]) {
    for (y, row) in source.iter().enumerate() {
        for (x, ch) in row.iter().enumerate() {
            target.set_char(target_pos + Position::new(x as i32, y as i32), *ch);
        }
    }
}
