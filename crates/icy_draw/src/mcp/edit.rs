use super::{types::*, McpCommand, SenderType};
use crate::document::Document;
use icy_engine::{AttributeColor, AttributedChar, Position, Rectangle, TextAttribute, TextPane};
use icy_engine_edit::EditState;

pub fn respond<T>(response: &SenderType<T>, value: T) {
    if let Some(sender) = response.lock().take() {
        let _ = sender.send(value);
    }
}

fn color_info(color: AttributeColor) -> ColorInfo {
    match color {
        AttributeColor::Palette(index) => ColorInfo::Palette(index),
        AttributeColor::ExtendedPalette(index) => ColorInfo::ExtendedPalette(index),
        AttributeColor::Rgb(red, green, blue) => ColorInfo::Rgb { r: red, g: green, b: blue },
        AttributeColor::Transparent => ColorInfo::Transparent,
    }
}

fn color(info: &ColorInfo) -> AttributeColor {
    match *info {
        ColorInfo::Palette(index) => AttributeColor::Palette(index),
        ColorInfo::ExtendedPalette(index) => AttributeColor::ExtendedPalette(index),
        ColorInfo::Rgb { r: red, g: green, b: blue } => AttributeColor::Rgb(red, green, blue),
        ColorInfo::Transparent => AttributeColor::Transparent,
    }
}

fn attribute(info: &TextAttributeInfo) -> TextAttribute {
    let mut attribute = TextAttribute::from_colors(color(&info.foreground), color(&info.background));
    attribute.set_is_bold(info.bold);
    attribute.set_is_blinking(info.blink);
    attribute
}

fn cell_info(buffer: &icy_engine::TextBuffer, cell: AttributedChar) -> CharInfo {
    CharInfo {
        ch: buffer.buffer_type.convert_to_unicode(cell.ch).to_string(),
        fg: color_info(cell.attribute.foreground_color()),
        bg: color_info(cell.attribute.background_color()),
        font_page: cell.attribute.font_page(),
        bold: cell.attribute.is_bold(),
        blink: cell.attribute.is_blinking(),
        is_visible: cell.is_visible(),
    }
}

fn caret(state: &EditState) -> CaretInfo {
    let caret = state.get_caret();
    let position = state.layer_to_document_position(caret.position());
    CaretInfo {
        x: caret.x,
        y: caret.y,
        doc_x: position.x,
        doc_y: position.y,
        attribute: TextAttributeInfo {
            foreground: color_info(caret.attribute.foreground_color()),
            background: color_info(caret.attribute.background_color()),
            bold: caret.attribute.is_bold(),
            blink: caret.attribute.is_blinking(),
        },
        insert_mode: caret.insert_mode,
        font_page: caret.font_page(),
    }
}

fn layers(state: &EditState) -> Vec<LayerInfo> {
    state
        .get_buffer()
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| LayerInfo {
            index,
            title: layer.properties.title.clone(),
            is_visible: layer.is_visible(),
            is_locked: layer.properties.is_locked,
            is_position_locked: layer.properties.is_position_locked,
            offset_x: layer.offset().x,
            offset_y: layer.offset().y,
            width: layer.width(),
            height: layer.height(),
            mode: format!("{:?}", layer.properties.mode),
            role: format!("{:?}", layer.role),
        })
        .collect()
}

fn selection(state: &EditState) -> Option<SelectionInfo> {
    state.selection().map(|selection| {
        let bounds = selection.as_rectangle();
        SelectionInfo {
            anchor_x: selection.anchor.x,
            anchor_y: selection.anchor.y,
            lead_x: selection.lead.x,
            lead_y: selection.lead.y,
            shape: format!("{:?}", selection.shape),
            locked: selection.locked,
            bounds: RectangleInfo {
                x: bounds.left(),
                y: bounds.top(),
                width: bounds.width(),
                height: bounds.height(),
            },
        }
    })
}

pub fn status(state: &EditState, outline_style: usize) -> AnsiStatus {
    let buffer = state.get_buffer();
    AnsiStatus {
        buffer: BufferInfo {
            width: buffer.width(),
            height: buffer.height(),
            layer_count: buffer.layers.len(),
            font_count: buffer.font_iter().count(),
            font_mode: format!("{:?}", buffer.font_mode),
            ice_mode: format!("{:?}", buffer.ice_mode),
            palette: format!("{} colors", buffer.palette.len()),
        },
        caret: caret(state),
        layers: layers(state),
        current_layer: state.get_current_layer().unwrap_or(0),
        selection: selection(state),
        format_mode: format!("{:?}", state.get_format_mode()),
        outline_style,
        mirror_mode: state.get_mirror_mode(),
    }
}

fn validate_region(state: &EditState, layer: usize, rect: Rectangle, write: bool) -> Result<(), String> {
    let layer = state.get_buffer().layers.get(layer).ok_or("Layer index out of bounds")?;
    if write && (layer.properties.is_locked || !layer.is_visible()) {
        return Err("Layer is locked or hidden".into());
    }
    if rect.left() < 0
        || rect.top() < 0
        || rect.width() <= 0
        || rect.height() <= 0
        || i64::from(rect.left()) + i64::from(rect.width()) > i64::from(layer.width())
        || i64::from(rect.top()) + i64::from(rect.height()) > i64::from(layer.height())
    {
        return Err("Region out of bounds".into());
    }
    Ok(())
}

fn region(state: &EditState, layer: usize, rect: Rectangle) -> Result<RegionData, String> {
    validate_region(state, layer, rect, false)?;
    let buffer = state.get_buffer();
    let mut chars = Vec::new();
    for row in rect.top()..rect.bottom() {
        for column in rect.left()..rect.right() {
            chars.push(cell_info(buffer, buffer.layers[layer].char_at((column, row).into())));
        }
    }
    Ok(RegionData {
        layer,
        x: rect.left(),
        y: rect.top(),
        width: rect.width(),
        height: rect.height(),
        chars,
    })
}

fn engine<T>(result: icy_engine::Result<T>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

pub fn handle(document: &mut Document, command: &McpCommand, unavailable: Option<&str>) -> bool {
    let read_only = matches!(
        command,
        McpCommand::AnsiGetCaret { .. }
            | McpCommand::AnsiListLayers { .. }
            | McpCommand::AnsiGetSelection { .. }
            | McpCommand::AnsiGetRegion { .. }
            | McpCommand::AnsiGetLayer { .. }
            | McpCommand::AnsiGetScreen { .. }
    );
    macro_rules! reply {
        ($response:expr, $action:expr) => {
            respond(
                $response,
                match unavailable {
                    Some(error) => Err(error.to_owned()),
                    None => {
                        if !read_only {
                            document.finish();
                        }
                        document.with_state($action)
                    }
                },
            )
        };
    }
    match command {
        McpCommand::AnsiGetCaret { response } => reply!(response, |state| Ok(caret(state))),
        McpCommand::AnsiListLayers { response } => reply!(response, |state| Ok(layers(state))),
        McpCommand::AnsiGetSelection { response } => reply!(response, |state| Ok(selection(state))),
        McpCommand::AnsiClearSelection { response } => reply!(response, |state| engine(state.clear_selection())),
        McpCommand::AnsiSetSelection { x, y, width, height, response } => reply!(response, |state| {
            if *width <= 0 || *height <= 0 || x.checked_add(*width).is_none() || y.checked_add(*height).is_none() {
                return Err("Invalid selection bounds".into());
            }
            engine(state.set_selection(Rectangle::from(*x, *y, *width, *height)))
        }),
        McpCommand::AnsiGetRegion {
            layer,
            x,
            y,
            width,
            height,
            response,
        } => reply!(response, |state| region(state, *layer, Rectangle::from(*x, *y, *width, *height))),
        McpCommand::AnsiGetLayer { layer, response } => reply!(response, |state| {
            let info = layers(state).into_iter().nth(*layer).ok_or("Layer index out of bounds")?;
            let region = region(state, *layer, Rectangle::from(0, 0, info.width, info.height))?;
            Ok(LayerData {
                index: *layer,
                title: info.title,
                is_visible: info.is_visible,
                is_locked: info.is_locked,
                is_position_locked: info.is_position_locked,
                offset_x: info.offset_x,
                offset_y: info.offset_y,
                width: info.width,
                height: info.height,
                mode: info.mode,
                role: info.role,
                chars: region.chars,
            })
        }),
        McpCommand::AnsiSetChar {
            layer,
            x,
            y,
            ch,
            attribute: attr,
            response,
        } => reply!(response, |state| {
            validate_region(state, *layer, Rectangle::from(*x, *y, 1, 1), true)?;
            let character = ch.chars().next().ok_or("Empty character")?;
            let character = state.get_buffer().buffer_type.convert_from_unicode(character);
            let _undo = state.begin_atomic_undo("MCP Set Char");
            engine(state.set_char_at_layer_in_atomic(*layer, Position::new(*x, *y), AttributedChar::new(character, attribute(attr))))
        }),
        McpCommand::AnsiSetRegion {
            layer,
            x,
            y,
            width,
            height,
            chars,
            response,
        } => reply!(response, |state| {
            validate_region(state, *layer, Rectangle::from(*x, *y, *width, *height), true)?;
            if i64::from(*width) * i64::from(*height) != chars.len() as i64 {
                return Err("Region cell count mismatch".into());
            }
            let converted: Result<Vec<_>, String> = chars
                .iter()
                .map(|cell| {
                    if !cell.is_visible {
                        return Ok(AttributedChar::invisible());
                    }
                    let character = cell.ch.chars().next().ok_or("Empty character")?;
                    let mut attr = attribute(&TextAttributeInfo {
                        foreground: cell.fg.clone(),
                        background: cell.bg.clone(),
                        bold: cell.bold,
                        blink: cell.blink,
                    });
                    attr.set_font_page(cell.font_page);
                    Ok(AttributedChar::new(state.get_buffer().buffer_type.convert_from_unicode(character), attr))
                })
                .collect();
            let converted = converted?;
            let _undo = state.begin_atomic_undo("MCP Set Region");
            for (index, cell) in converted.into_iter().enumerate() {
                engine(state.set_char_at_layer_in_atomic(*layer, Position::new(*x + index as i32 % *width, *y + index as i32 / *width), cell))?;
            }
            Ok(())
        }),
        McpCommand::AnsiSetCaret {
            x,
            y,
            attribute: attr,
            response,
        } => reply!(response, |state| {
            state.set_caret_position(Position::new(*x, *y));
            state.set_caret_attribute(attribute(attr));
            Ok(())
        }),
        McpCommand::AnsiSetColor {
            index,
            r: red,
            g: green,
            b: blue,
            response,
        } => reply!(response, |state| {
            let mut palette = state.get_buffer().palette.clone();
            if usize::from(*index) >= palette.len() {
                return Err("Palette index out of bounds".into());
            }
            palette.set_color(u32::from(*index), icy_engine::Color::new(*red, *green, *blue));
            engine(state.switch_to_palette(palette))
        }),
        McpCommand::AnsiAddLayer { after_layer, response } => reply!(response, |state| {
            if *after_layer >= state.get_buffer().layers.len() {
                return Err("Layer index out of bounds".into());
            }
            engine(state.add_new_layer(*after_layer))?;
            engine(state.get_current_layer())
        }),
        McpCommand::AnsiDeleteLayer { layer, response } => reply!(response, |state| {
            if state.get_buffer().layers.len() <= 1 {
                return Err("Cannot remove the last layer".into());
            }
            engine(state.remove_layer(*layer))
        }),
        McpCommand::AnsiMergeDownLayer { layer, response } => reply!(response, |state| engine(state.merge_layer_down(*layer))),
        McpCommand::AnsiMoveLayer { layer, direction, response } => reply!(response, |state| engine(match direction {
            LayerMoveDirection::Up => state.raise_layer(*layer),
            LayerMoveDirection::Down => state.lower_layer(*layer),
        })),
        McpCommand::AnsiResize { width, height, response } => reply!(response, |state| {
            if !(1..=1000).contains(width) || !(1..=10000).contains(height) {
                return Err("Invalid document dimensions".into());
            }
            engine(state.resize_buffer(true, (*width, *height)))
        }),
        McpCommand::AnsiSetLayerProps {
            layer,
            title,
            is_visible,
            is_locked,
            is_position_locked,
            offset_x,
            offset_y,
            transparency,
            response,
        } => reply!(response, |state| {
            if transparency.is_some() {
                return Err("Layer opacity is not supported by the editing backend".into());
            }
            let mut properties = state.get_buffer().layers.get(*layer).ok_or("Layer index out of bounds")?.properties.clone();
            if properties.is_position_locked && (offset_x.is_some() || offset_y.is_some()) {
                return Err("Layer position is locked".into());
            }
            if let Some(title) = title {
                properties.title = title.clone();
            }
            if let Some(value) = is_visible {
                properties.is_visible = *value;
            }
            if let Some(value) = is_locked {
                properties.is_locked = *value;
            }
            if let Some(value) = is_position_locked {
                properties.is_position_locked = *value;
            }
            if let Some(value) = offset_x {
                properties.offset.x = *value;
            }
            if let Some(value) = offset_y {
                properties.offset.y = *value;
            }
            engine(state.update_layer_properties(*layer, properties))
        }),
        McpCommand::AnsiSelectionAction { action, response } => reply!(response, |state| {
            if state.get_cur_layer().is_none_or(|layer| layer.properties.is_locked || !layer.is_visible()) {
                return Err("Layer is locked or hidden".into());
            }
            engine(match action.trim().to_lowercase().as_str() {
                "flip_x" => state.flip_x(),
                "flip_y" => state.flip_y(),
                "crop" => state.crop(),
                "justify_left" => state.justify_left(),
                "justify_center" | "center" => state.center(),
                "justify_right" => state.justify_right(),
                "justify_line_left" => state.justify_line_left(),
                "justify_line_center" | "center_line" => state.center_line(),
                "justify_line_right" => state.justify_line_right(),
                "delete_selection" | "delete" => state.erase_selection(),
                "deselect" | "clear" => state.clear_selection(),
                _ => return Err("Unknown selection action".into()),
            })
        }),
        McpCommand::AnsiGetScreen { format, response } => reply!(response, |state| {
            let buffer = state.get_buffer();
            match format {
                AnsiScreenFormat::Ascii => Ok((0..buffer.height())
                    .map(|row| {
                        (0..buffer.width())
                            .map(|column| buffer.buffer_type.convert_to_unicode(buffer.char_at((column, row).into()).ch))
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")),
                AnsiScreenFormat::Ansi => {
                    let bytes = engine(
                        icy_engine::FileFormat::Ansi.to_bytes(buffer, &icy_engine::SaveOptions::ansi(icy_engine::AnsiCompatibilityLevel::Utf8Terminal)),
                    )?;
                    String::from_utf8(bytes).map_err(|error| error.to_string())
                }
            }
        }),
        McpCommand::AnsiRunScript {
            script,
            undo_description,
            response,
        } => {
            respond(
                response,
                if let Some(error) = unavailable {
                    Err(error.into())
                } else if !document.can_paint() {
                    Err("Layer is locked or hidden".into())
                } else {
                    document.finish();
                    crate::plugins::Plugin::run_script_string(&document.screen, script, undo_description.as_deref().unwrap_or("MCP Script"))
                },
            );
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_region_does_not_partially_write_and_colors_undo() {
        let mut document = Document::new(icy_engine::Size::new(20, 10));
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        let response = std::sync::Arc::new(parking_lot::Mutex::new(Some(sender)));
        let command = McpCommand::AnsiGetRegion {
            layer: 0,
            x: i32::MAX,
            y: 0,
            width: 2,
            height: 1,
            response,
        };
        assert!(handle(&mut document, &command, None));
        assert!(receiver.try_recv().unwrap().is_err());
        let before = document.with_state(|state| state.get_buffer().char_at((0, 0).into()));
        let valid = document.with_state(|state| cell_info(state.get_buffer(), AttributedChar::new('A', TextAttribute::default())));
        let mut invalid = valid.clone();
        invalid.ch.clear();
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        let command = McpCommand::AnsiSetRegion {
            layer: 0,
            x: 0,
            y: 0,
            width: 2,
            height: 1,
            chars: vec![valid, invalid],
            response: std::sync::Arc::new(parking_lot::Mutex::new(Some(sender))),
        };
        assert!(handle(&mut document, &command, None));
        assert!(receiver.try_recv().unwrap().is_err());
        assert_eq!(document.with_state(|state| state.get_buffer().char_at((0, 0).into())), before);
        assert!(!document.modified());
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        let command = McpCommand::AnsiSetColor {
            index: 1,
            r: 12,
            g: 34,
            b: 56,
            response: std::sync::Arc::new(parking_lot::Mutex::new(Some(sender))),
        };
        handle(&mut document, &command, None);
        receiver.try_recv().unwrap().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().palette.rgb(1)), (12, 34, 56));
        document.undo().unwrap();
        assert!(!document.modified());
    }
}
