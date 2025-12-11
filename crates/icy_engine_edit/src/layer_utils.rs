//! Utility functions for Layer operations that were previously part of icy_engine
//! but have been moved here as they are specific to editing operations.

use crate::{Layer, Position, Rectangle, TextPane};

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
