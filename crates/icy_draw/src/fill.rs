use crate::{brush::BrushPrimaryMode, paint::BrushSettings};
use icy_engine::{paint::HalfBlock, AttributedChar, MouseButton, Position, TextAttribute, TextPane};
use icy_engine_edit::EditState;
use std::collections::HashSet;

pub fn fill(state: &mut EditState, settings: BrushSettings, position: Position, top: bool, button: MouseButton, shift: bool) {
    let primary = match settings.primary {
        BrushPrimaryMode::HalfBlock | BrushPrimaryMode::Char | BrushPrimaryMode::Colorize => settings.primary,
        _ => BrushPrimaryMode::Char,
    };
    if primary == BrushPrimaryMode::Colorize && !settings.colorize_fg && !settings.colorize_bg {
        return;
    }
    let Some(layer) = state.get_cur_layer() else {
        return;
    };
    if layer.properties.is_locked || !layer.is_visible() {
        return;
    }
    let (offset, width, height) = (layer.offset(), layer.width(), layer.height());
    let selected = state.is_something_selected();
    let attribute = state.get_caret().attribute;
    let swapped = button == MouseButton::Right || shift;
    let (foreground, background) = if swapped {
        (attribute.background_color(), attribute.foreground_color())
    } else {
        (attribute.foreground_color(), attribute.background_color())
    };
    let fill_color = if swapped {
        attribute.background_color()
    } else {
        attribute.foreground_color()
    };
    let _undo = state.begin_atomic_undo("Bucket fill");
    let replacement = |mut character: AttributedChar| {
        if primary == BrushPrimaryMode::Char {
            character.ch = settings.paint_char;
        }
        if settings.colorize_fg {
            character.attribute.set_foreground_color(foreground);
            character.attribute.set_is_bold(attribute.is_bold());
        }
        if settings.colorize_bg {
            character.attribute.set_background_color(background);
        }
        character.set_font_page(attribute.font_page());
        character.attribute.attr &= !icy_engine::attribute::INVISIBLE;
        character
    };
    if selected {
        for row in 0..height {
            for column in 0..width {
                let point = Position::new(column, row);
                if state.is_selected(point + offset) {
                    let character = if primary == BrushPrimaryMode::HalfBlock {
                        AttributedChar::new(219 as char, TextAttribute::from_colors(fill_color, fill_color))
                    } else {
                        replacement(state.get_cur_layer().unwrap().char_at(point))
                    };
                    let _ = state.set_char_in_atomic(point, character);
                }
            }
        }
        return;
    }
    let local = position - offset;
    if local.x < 0 || local.y < 0 || local.x >= width || local.y >= height {
        return;
    }
    let base = state.get_cur_layer().unwrap().char_at(local);
    if primary != BrushPrimaryMode::HalfBlock {
        let mut visited = HashSet::new();
        let mut stack = vec![local];
        while let Some(point) = stack.pop() {
            if point.x < 0 || point.y < 0 || point.x >= width || point.y >= height || !visited.insert(point) {
                continue;
            }
            let character = state.get_cur_layer().unwrap().char_at(point);
            let matches = if settings.exact {
                character == base
            } else if primary == BrushPrimaryMode::Char {
                character.ch == base.ch
            } else {
                character.attribute == base.attribute
            };
            if !matches {
                continue;
            }
            let _ = state.set_char_in_atomic(point, replacement(character));
            for delta in [Position::new(-1, 0), Position::new(1, 0), Position::new(0, -1), Position::new(0, 1)] {
                stack.push(point + delta);
            }
        }
        return;
    }
    let start = Position::new(local.x, local.y * 2 + i32::from(!top));
    let block = HalfBlock::from_char(base, start);
    if !block.is_blocky() {
        return;
    }
    let target = if block.is_top { block.upper_block_color } else { block.lower_block_color };
    if target == fill_color {
        return;
    }
    let mut visited = HashSet::new();
    let mut stack = vec![(start, start)];
    while let Some((from, point)) = stack.pop() {
        if point.x < 0 || point.y < 0 || point.x >= width || point.y >= height * 2 || !visited.insert(point) {
            continue;
        }
        let cell = Position::new(point.x, point.y / 2);
        let block = HalfBlock::from_char(state.get_cur_layer().unwrap().char_at(cell), point);
        let replacement = if block.is_blocky() && (if block.is_top { block.upper_block_color } else { block.lower_block_color }) == target {
            Some(block.get_half_block_char(fill_color, true))
        } else if block.is_vertically_blocky() {
            let left = block.left_block_color == target;
            let right = block.right_block_color == target;
            if (from.x == point.x - 1 && left) || ((from.y != point.y || from == point) && left) {
                Some(AttributedChar::new(
                    221 as char,
                    TextAttribute::from_colors(fill_color, block.right_block_color),
                ))
            } else if (from.x == point.x + 1 && right) || ((from.y != point.y || from == point) && right) {
                Some(AttributedChar::new(222 as char, TextAttribute::from_colors(fill_color, block.left_block_color)))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(character) = replacement {
            let _ = state.set_char_in_atomic(cell, character);
            for delta in [Position::new(-1, 0), Position::new(1, 0), Position::new(0, -1), Position::new(0, 1)] {
                stack.push((point, point + delta));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Selection, Size, TextBuffer};
    use icy_engine_edit::UndoState;

    #[test]
    fn fill_stops_at_character_boundaries_and_undoes_once() {
        let mut state = EditState::from_buffer(TextBuffer::new(Size::new(10, 4)));
        for row in 0..4 {
            state.set_char((5, row), AttributedChar::new('#', TextAttribute::default())).unwrap();
        }
        let count = state.undo_stack_len();
        let settings = BrushSettings {
            primary: BrushPrimaryMode::Char,
            paint_char: '.',
            ..Default::default()
        };
        fill(&mut state, settings, Position::new(0, 0), true, MouseButton::Left, false);
        assert_eq!(state.get_buffer().char_at(Position::new(4, 3)).ch, '.');
        assert_ne!(state.get_buffer().char_at(Position::new(6, 3)).ch, '.');
        assert_eq!(state.undo_stack_len(), count + 1);
        state.undo().unwrap();
        assert_ne!(state.get_buffer().char_at(Position::new(4, 3)).ch, '.');
    }

    #[test]
    fn selected_area_fill_ignores_start_and_respects_layer_offset() {
        let mut state = EditState::from_buffer(TextBuffer::new(Size::new(10, 4)));
        state.move_layer(Position::new(2, 0)).unwrap();
        state.set_selection(Selection::new(Position::new(3, 1))).unwrap();
        fill(
            &mut state,
            BrushSettings {
                primary: BrushPrimaryMode::Char,
                paint_char: '#',
                ..Default::default()
            },
            Position::new(0, 0),
            true,
            MouseButton::Left,
            false,
        );
        assert_eq!(state.get_cur_layer().unwrap().char_at(Position::new(1, 1)).ch, '#');
        assert_ne!(state.get_cur_layer().unwrap().char_at(Position::new(2, 1)).ch, '#');
    }

    #[test]
    fn shift_fill_preserves_rgb_background_color() {
        let mut state = EditState::from_buffer(TextBuffer::new(Size::new(4, 2)));
        let attribute = TextAttribute::from_colors(icy_engine::AttributeColor::Rgb(12, 34, 56), icy_engine::AttributeColor::Rgb(70, 80, 90));
        state.set_caret_attribute(attribute);
        fill(
            &mut state,
            BrushSettings {
                primary: BrushPrimaryMode::Char,
                paint_char: '#',
                ..Default::default()
            },
            Position::new(0, 0),
            true,
            MouseButton::Left,
            true,
        );
        assert_eq!(
            state.get_buffer().char_at(Position::new(0, 0)).attribute.foreground_color(),
            attribute.background_color()
        );
    }
}
