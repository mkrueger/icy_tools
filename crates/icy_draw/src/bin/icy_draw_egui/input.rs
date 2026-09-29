use eframe::egui::{Key, Modifiers};
use icy_draw::{brush::BrushPrimaryMode, document::Document, FKeySets};
use icy_engine::{Position, Selection, TextPane};
use icy_engine_edit::tools::Tool;

const FUNCTION_KEYS: [Key; 12] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
];

pub fn key(document: &mut Document, fkeys: &mut FKeySets, key: Key, modifiers: Modifiers) -> Result<bool, String> {
    if document.paste_active() {
        use icy_draw::document::PasteAction;
        let action = match key {
            Key::Escape => PasteAction::Cancel,
            Key::Enter => PasteAction::Anchor,
            Key::ArrowLeft => PasteAction::Move(Position::new(-1, 0)),
            Key::ArrowRight => PasteAction::Move(Position::new(1, 0)),
            Key::ArrowUp => PasteAction::Move(Position::new(0, -1)),
            Key::ArrowDown => PasteAction::Move(Position::new(0, 1)),
            Key::S if !modifiers.command && !modifiers.ctrl => PasteAction::Stamp,
            Key::R if !modifiers.command && !modifiers.ctrl => PasteAction::Rotate,
            Key::X if !modifiers.command && !modifiers.ctrl => PasteAction::FlipX,
            Key::Y if !modifiers.command && !modifiers.ctrl => PasteAction::FlipY,
            Key::T if !modifiers.command && !modifiers.ctrl => PasteAction::Transparent,
            Key::Equals if !modifiers.command && !modifiers.ctrl && !modifiers.alt => PasteAction::Center,
            _ => return Ok(true),
        };
        document.paste_action(action)?;
        return Ok(true);
    }
    if key == Key::Escape {
        if document.stroke_active() {
            document.cancel();
        } else {
            document.selected_tags.clear();
            document.with_state(|state| state.clear_selection()).map_err(|error| error.to_string())?;
        }
        return Ok(true);
    }
    if document.tool == Tool::Tag && matches!(key, Key::Delete | Key::Backspace) {
        document.delete_selected_tags()?;
        return Ok(true);
    }
    if document.tool == Tool::Tag && !modifiers.alt && !modifiers.ctrl && !modifiers.command {
        let step = if modifiers.shift { 10 } else { 1 };
        let delta = match key {
            Key::Tab => return Ok(document.select_next_tag(modifiers.shift)),
            Key::ArrowLeft => Position::new(-step, 0),
            Key::ArrowRight => Position::new(step, 0),
            Key::ArrowUp => Position::new(0, -step),
            Key::ArrowDown => Position::new(0, step),
            _ => return Ok(false),
        };
        return document.nudge_selected_tags(delta);
    }
    let brush_tool = document.tool == Tool::Pencil || document.tool.is_shape_tool();
    if brush_tool && modifiers.alt && !modifiers.ctrl && !modifiers.command {
        match key {
            Key::Equals | Key::Plus => document.brush.brush_size = (document.brush.brush_size + 1).min(9),
            Key::Minus => document.brush.brush_size = document.brush.brush_size.saturating_sub(1).max(1),
            Key::CloseBracket | Key::Num0 => document.brush.brush_size = 1,
            _ => return Ok(false),
        }
        return Ok(true);
    }
    // As in Moebius, F1–F12 pick the brush character from the current set in the brush tools.
    if brush_tool && !modifiers.any() {
        if let Some(slot) = FUNCTION_KEYS.iter().position(|candidate| *candidate == key) {
            let code = fkeys.code_at(fkeys.current_set(), slot);
            document.brush.primary = BrushPrimaryMode::Char;
            document.brush.paint_char = char::from_u32(code as u32).unwrap_or(' ');
            return Ok(true);
        }
    }
    if document.tool == Tool::Select {
        let selected = document.with_state(|state| state.is_something_selected());
        // The block keys of Moebius: move, copy, fill and erase the selection.
        let plain = !modifiers.any() || modifiers.shift_only();
        let result = match key {
            Key::Delete | Key::Backspace | Key::E if selected && document.can_paint() && (plain || key != Key::E) => {
                document.finish();
                document.with_state(|state| state.erase_selection()).map_err(|error| error.to_string())
            }
            Key::M if selected && plain => document.float_selection(true),
            Key::C if selected && plain => document.float_selection(false),
            Key::F if selected && plain => document.fill_selection(),
            _ => return Ok(false),
        };
        result?;
        return Ok(true);
    }
    if !matches!(document.tool, Tool::Click | Tool::Font) {
        return Ok(false);
    }
    if document.tool == Tool::Click {
        if let Some(slot) = FUNCTION_KEYS.iter().position(|candidate| *candidate == key) {
            if document.outline_font {
                if slot < 10 && document.can_paint() {
                    document.type_text(&char::from_u32('A' as u32 + slot as u32).unwrap().to_string())?;
                }
            } else if modifiers.alt && slot < 10 {
                let set = slot + if modifiers.shift { 10 } else { 0 };
                fkeys.current_set = set % fkeys.set_count().max(1);
            } else if !modifiers.command && !modifiers.ctrl && document.can_paint() {
                document.finish();
                let character = char::from_u32(fkeys.code_at(fkeys.current_set(), slot) as u32).unwrap_or(' ');
                document.with_state(|state| state.type_key(character)).map_err(|error| error.to_string())?;
            }
            return Ok(true);
        }
        if (modifiers.ctrl || modifiers.command) && !modifiers.alt {
            let count = fkeys.set_count().max(1);
            match key {
                Key::Comma => fkeys.current_set = (fkeys.current_set + count - 1) % count,
                Key::Period => fkeys.current_set = (fkeys.current_set + 1) % count,
                Key::Slash => fkeys.current_set = FKeySets::default().current_set % count,
                _ => {}
            }
            if matches!(key, Key::Comma | Key::Period | Key::Slash) {
                return Ok(true);
            }
        }
    }
    if modifiers.alt {
        return Ok(false);
    }
    let image_layer = document.with_state(|state| state.get_cur_layer().is_some_and(|layer| layer.role == icy_engine::Role::Image));
    if image_layer {
        if document.tool == Tool::Click && matches!(key, Key::Delete | Key::Backspace) {
            document.finish();
            document
                .with_state(|state| {
                    let index = state.get_current_layer()?;
                    if state.get_buffer().layers.len() > 1 && !state.get_buffer().layers[index].properties.is_locked {
                        state.remove_layer(index)?;
                    }
                    Ok::<(), icy_engine::EngineError>(())
                })
                .map_err(|error| error.to_string())?;
        }
        return Ok(true);
    }
    if matches!(
        key,
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown
    ) {
        document.finish();
        document
            .with_state(|state| {
                let start = state.get_caret().position();
                let Some(layer) = state.get_cur_layer() else {
                    return Ok(());
                };
                let offset = layer.offset();
                let maximum = Position::new((layer.width() - 1).max(0), (layer.height() - 1).max(0));
                let mut target = start;
                match key {
                    Key::ArrowLeft => target.x -= 1,
                    Key::ArrowRight => target.x += 1,
                    Key::ArrowUp => target.y -= 1,
                    Key::ArrowDown => target.y += 1,
                    Key::PageUp => target.y -= 24,
                    Key::PageDown => target.y += 24,
                    Key::Home if modifiers.ctrl || modifiers.command => target = Position::default(),
                    Key::End if modifiers.ctrl || modifiers.command => target = maximum,
                    Key::Home => target.x = 0,
                    Key::End => target.x = maximum.x,
                    _ => {}
                }
                target.x = target.x.clamp(0, maximum.x);
                target.y = target.y.clamp(0, maximum.y);
                if modifiers.shift {
                    let mut selection = state.selection().unwrap_or_else(|| Selection::new(start + offset));
                    selection.lead = target + offset;
                    state.set_selection(selection)?;
                }
                state.set_caret_position(target);
                Ok::<(), icy_engine::EngineError>(())
            })
            .map_err(|error| error.to_string())?;
        return Ok(true);
    }
    match key {
        Key::Delete | Key::Backspace if document.can_paint() => {
            document.finish();
            if key == Key::Backspace && document.tool == Tool::Font {
                document.font_backspace()?;
            } else {
                document
                    .with_state(|state| {
                        if state.is_something_selected() {
                            state.erase_selection()
                        } else if key == Key::Backspace {
                            state.backspace()
                        } else {
                            state.delete_key()
                        }
                    })
                    .map_err(|error| error.to_string())?;
            }
        }
        Key::Enter if document.tool == Tool::Click && document.can_paint() => {
            document.finish();
            document.with_state(|state| state.new_line()).map_err(|error| error.to_string())?;
        }
        Key::Tab => document.with_state(|state| {
            if modifiers.shift {
                state.handle_reverse_tab();
            } else {
                state.handle_tab();
            }
            if let Some(layer) = state.get_cur_layer() {
                let position = state.get_caret().position();
                let maximum = Position::new((layer.width() - 1).max(0), (layer.height() - 1).max(0));
                state.set_caret_position(Position::new(position.x.clamp(0, maximum.x), position.y.clamp(0, maximum.y)));
            }
        }),
        Key::Insert => document.with_state(|state| state.toggle_insert_mode()),
        Key::Space if modifiers.shift && document.tool == Tool::Click && document.can_paint() => {
            if document.outline_font && !modifiers.ctrl && !modifiers.command {
                return Ok(false);
            }
            document.finish();
            document.with_state(|state| state.type_key('\u{00ff}')).map_err(|error| error.to_string())?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::Size;

    #[test]
    fn keys_do_not_leak_between_edit_tools() {
        for tool in [
            Tool::Pencil,
            Tool::Line,
            Tool::RectangleFilled,
            Tool::EllipseOutline,
            Tool::Fill,
            Tool::Pipette,
            Tool::Tag,
            Tool::Select,
        ] {
            let mut document = Document::new(Size::new(20, 10));
            let mut fkeys = FKeySets::default();
            document.tool = tool;
            for pressed in [Key::F1, Key::Enter, Key::Tab, Key::ArrowRight, Key::Insert] {
                // F1–F12 pick the brush character in the brush tools instead of typing.
                let brush = tool == Tool::Pencil || tool.is_shape_tool();
                let handled = key(&mut document, &mut fkeys, pressed, Modifiers::NONE).unwrap();
                assert_eq!(handled, brush && pressed == Key::F1, "{tool:?}: {pressed:?}");
            }
            assert_eq!(document.with_state(|state| state.get_caret().position()), Position::default());
            assert!(!document.modified());
        }
    }

    #[test]
    fn tag_tool_tabs_through_tags_and_nudges_them_with_arrows() {
        let mut document = Document::new(Size::new(20, 10));
        let mut fkeys = FKeySets::default();
        document.tool = Tool::Tag;
        let tag = icy_engine::Tag {
            is_enabled: true,
            preview: "T".into(),
            replacement_value: String::new(),
            position: Position::new(4, 4),
            length: 1,
            alignment: std::fmt::Alignment::Left,
            tag_placement: icy_engine::TagPlacement::InText,
            tag_role: icy_engine::TagRole::Displaycode,
            attribute: icy_engine::TextAttribute::default(),
        };
        document.with_state(|state| state.add_new_tag(tag)).unwrap();
        assert!(!key(&mut document, &mut fkeys, Key::ArrowRight, Modifiers::NONE).unwrap(), "nothing selected");
        assert!(key(&mut document, &mut fkeys, Key::Tab, Modifiers::NONE).unwrap());
        assert_eq!(document.selected_tags, vec![0]);
        key(&mut document, &mut fkeys, Key::ArrowRight, Modifiers::NONE).unwrap();
        key(&mut document, &mut fkeys, Key::ArrowDown, Modifiers::SHIFT).unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().tags[0].position), Position::new(5, 9));
    }

    #[test]
    fn moebius_keys_pick_brush_characters_and_work_on_blocks() {
        let mut document = Document::new(Size::new(20, 10));
        let mut fkeys = FKeySets::default();
        document.tool = Tool::Line;
        key(&mut document, &mut fkeys, Key::F3, Modifiers::NONE).unwrap();
        assert_eq!(document.brush.primary, BrushPrimaryMode::Char);
        assert_eq!(document.brush.paint_char as u32, fkeys.code_at(fkeys.current_set(), 2) as u32);
        document.brush.brush_size = 5;
        key(&mut document, &mut fkeys, Key::Num0, Modifiers::ALT).unwrap();
        assert_eq!(document.brush.brush_size, 1, "Alt+0 resets the brush size");

        // Select: F fills with full blocks in the foreground color, C floats a copy, M moves it.
        document.tool = Tool::Click;
        document.type_text("AB").unwrap();
        document.tool = Tool::Select;
        let mut selection = Selection::new(Position::new(0, 0));
        selection.lead = Position::new(1, 0);
        document.with_state(|state| state.set_selection(selection)).unwrap();
        key(&mut document, &mut fkeys, Key::C, Modifiers::NONE).unwrap();
        assert!(document.paste_active(), "C floats a copy of the block");
        key(&mut document, &mut fkeys, Key::Equals, Modifiers::NONE).unwrap();
        assert_eq!(
            document.with_state(|state| state.get_cur_layer().unwrap().offset()),
            Position::new(9, 0),
            "= centers it"
        );
        key(&mut document, &mut fkeys, Key::Escape, Modifiers::NONE).unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'A');

        document.with_state(|state| state.set_selection(selection)).unwrap();
        key(&mut document, &mut fkeys, Key::M, Modifiers::NONE).unwrap();
        assert!(document.paste_active());
        key(&mut document, &mut fkeys, Key::ArrowDown, Modifiers::NONE).unwrap();
        key(&mut document, &mut fkeys, Key::Enter, Modifiers::NONE).unwrap();
        let at = |document: &Document, x, y| document.with_state(|state| state.get_buffer().char_at(Position::new(x, y)).ch);
        assert_eq!(
            (at(&document, 0, 0), at(&document, 0, 1), at(&document, 1, 1)),
            (' ', 'A', 'B'),
            "M moves the block and erases it below"
        );

        document.with_state(|state| state.set_selection(selection)).unwrap();
        key(&mut document, &mut fkeys, Key::F, Modifiers::NONE).unwrap();
        let filled = document.with_state(|state| state.get_buffer().char_at(Position::new(1, 0)));
        assert_eq!(filled.ch as u32, 219, "F fills with full blocks");
        assert_eq!(
            filled.attribute.foreground(),
            document.with_state(|state| state.get_caret().attribute.foreground())
        );
        document.with_state(|state| state.set_selection(selection)).unwrap();
        key(&mut document, &mut fkeys, Key::E, Modifiers::NONE).unwrap();
        assert_eq!(at(&document, 1, 0), ' ', "E erases the block");
    }

    #[test]
    fn shifted_navigation_clamps_and_uses_document_coordinates() {
        let mut document = Document::new(Size::new(20, 100));
        let mut fkeys = FKeySets::default();
        document.with_state(|state| {
            state.move_layer(Position::new(3, 4)).unwrap();
            state.set_caret_position(Position::new(2, 3));
        });
        key(&mut document, &mut fkeys, Key::PageDown, Modifiers::SHIFT).unwrap();
        assert_eq!(document.with_state(|state| state.get_caret().position()), Position::new(2, 27));
        assert_eq!(document.with_state(|state| state.selection().unwrap().anchor), Position::new(5, 7));
        assert_eq!(document.with_state(|state| state.selection().unwrap().lead), Position::new(5, 31));
        key(&mut document, &mut fkeys, Key::End, Modifiers::CTRL | Modifiers::SHIFT).unwrap();
        assert_eq!(document.with_state(|state| state.get_caret().position()), Position::new(19, 99));
        assert_eq!(document.with_state(|state| state.selection().unwrap().lead), Position::new(22, 103));
    }

    #[test]
    fn fkey_sets_and_brush_shortcuts_match_legacy() {
        let mut document = Document::new(Size::new(20, 10));
        let mut fkeys = FKeySets::default();
        key(&mut document, &mut fkeys, Key::F3, Modifiers::ALT | Modifiers::SHIFT).unwrap();
        assert_eq!(fkeys.current_set, 12 % fkeys.set_count());
        key(&mut document, &mut fkeys, Key::Comma, Modifiers::CTRL).unwrap();
        assert_eq!(fkeys.current_set, 11 % fkeys.set_count());
        key(&mut document, &mut fkeys, Key::F1, Modifiers::NONE).unwrap();
        assert_eq!(
            document.with_state(|state| state.get_buffer().char_at(Position::default()).ch as u32),
            fkeys.code_at(fkeys.current_set(), 0) as u32
        );
        document.tool = Tool::Pencil;
        for _ in 0..20 {
            key(&mut document, &mut fkeys, Key::Equals, Modifiers::ALT).unwrap();
        }
        assert_eq!(document.brush.brush_size, 9);
        key(&mut document, &mut fkeys, Key::CloseBracket, Modifiers::ALT).unwrap();
        assert_eq!(document.brush.brush_size, 1);
    }
}
