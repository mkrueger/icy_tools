//! Drawing tools for the AI assistant. They edit a draft copy of the document; the user
//! previews the result and accepts or discards it as a whole.

use icy_engine::{AttributeColor, AttributedChar, Position, TextBuffer, TextPane};
use serde_json::{json, Value};

const MAX_READ_CELLS: i64 = 4000;
const MAX_WRITE_CELLS: usize = 8000;

#[derive(Clone)]
pub struct Draft {
    /// Editor kind, e.g. "ANSI/ASCII" or "PETSCII".
    pub editor: String,
    /// The document as it was when drawing started; changes are computed against it.
    pub original: TextBuffer,
    pub buffer: TextBuffer,
    pub current_layer: usize,
    /// Selection bounds in document coordinates.
    pub selection: Option<(i32, i32, i32, i32)>,
}

/// A cell the assistant changed: layer, layer position and new content.
pub type Change = (usize, Position, AttributedChar);

impl Draft {
    pub fn new(editor: &str, buffer: TextBuffer, current_layer: usize, selection: Option<(i32, i32, i32, i32)>) -> Self {
        Self {
            editor: editor.to_owned(),
            original: buffer.clone(),
            buffer,
            current_layer,
            selection,
        }
    }

    pub fn changes(&self) -> Vec<Change> {
        let mut changes = Vec::new();
        for (index, (before, after)) in self.original.layers.iter().zip(&self.buffer.layers).enumerate() {
            for y in 0..after.height().min(before.height()) {
                for x in 0..after.width().min(before.width()) {
                    let position = Position::new(x, y);
                    let cell = after.char_at(position);
                    if before.char_at(position) != cell {
                        changes.push((index, position, cell));
                    }
                }
            }
        }
        changes
    }

    /// Runs a tool and returns the text reported back to the model.
    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<String, String> {
        match tool {
            "icy_canvas_info" => Ok(self.info().to_string()),
            "icy_read_region" => self.read_region(arguments),
            "icy_draw_text" => self.draw_text(arguments),
            "icy_fill_rect" => self.fill_rect(arguments),
            "icy_set_cells" => self.set_cells(arguments),
            _ => Err(format!("Unknown tool {tool}")),
        }
    }

    fn info(&self) -> Value {
        let buffer = &self.buffer;
        let palette: Vec<_> = (0..buffer.palette.len() as u32)
            .map(|index| {
                let (r, g, b) = buffer.palette.rgb(index);
                json!({ "index": index, "rgb": format!("#{r:02x}{g:02x}{b:02x}") })
            })
            .collect();
        let layers: Vec<_> = buffer
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                json!({
                    "index": index,
                    "title": layer.properties.title,
                    "offset": [layer.offset().x, layer.offset().y],
                    "width": layer.width(),
                    "height": layer.height(),
                    "visible": layer.is_visible(),
                    "locked": layer.properties.is_locked,
                })
            })
            .collect();
        json!({
            "editor": self.editor,
            "width": buffer.width(),
            "height": buffer.height(),
            "encoding": format!("{:?}", buffer.buffer_type),
            "ice_colors": format!("{:?}", buffer.ice_mode),
            "palette": palette,
            "layers": layers,
            "current_layer": self.current_layer,
            "selection": self.selection.map(|(x, y, width, height)| json!({ "x": x, "y": y, "width": width, "height": height })),
            "notes": "Coordinates are layer-relative, (0,0) is the top-left cell. Colors are palette indices. \
                      Characters are Unicode and converted to the document encoding; unmappable characters may not display. \
                      Without iCE colors, background indices 8-15 blink in classic ANSI.",
        })
    }

    fn layer_index(&self, arguments: &Value, write: bool) -> Result<usize, String> {
        let index = match arguments.get("layer").and_then(Value::as_u64) {
            Some(index) => index as usize,
            None => self.current_layer,
        };
        let layer = self.buffer.layers.get(index).ok_or_else(|| format!("Layer {index} does not exist"))?;
        if write && (layer.properties.is_locked || !layer.is_visible()) {
            return Err(format!("Layer {index} is locked or hidden"));
        }
        Ok(index)
    }

    fn color(&self, arguments: &Value, key: &str) -> Result<Option<AttributeColor>, String> {
        match arguments.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => {
                let index = value.as_u64().ok_or_else(|| format!("{key} must be a palette index"))?;
                let palette = self.buffer.palette.len().min(256);
                match u8::try_from(index) {
                    Ok(index) if usize::from(index) < palette => Ok(Some(AttributeColor::Palette(index))),
                    _ => Err(format!("{key} {index} is outside the palette (0-{})", palette - 1)),
                }
            }
        }
    }

    fn character(&self, value: Option<&Value>) -> Result<Option<char>, String> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let text = value.as_str().ok_or("char must be a string")?;
        let mut characters = text.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) => Ok(Some(self.buffer.buffer_type.convert_from_unicode(character))),
            _ => Err(format!("char must be exactly one character, got {text:?}")),
        }
    }

    /// Writes one cell, keeping font page and flags; returns false when outside the layer.
    fn put(&mut self, layer: usize, x: i64, y: i64, character: Option<char>, foreground: Option<AttributeColor>, background: Option<AttributeColor>) -> bool {
        let target = &mut self.buffer.layers[layer];
        if x < 0 || y < 0 || x >= i64::from(target.width()) || y >= i64::from(target.height()) {
            return false;
        }
        let position = Position::new(x as i32, y as i32);
        let mut cell = target.char_at(position);
        if !cell.is_visible() {
            cell = AttributedChar::default();
        }
        if let Some(character) = character {
            cell.ch = character;
        }
        if let Some(color) = foreground {
            cell.attribute.set_foreground_color(color);
        }
        if let Some(color) = background {
            cell.attribute.set_background_color(color);
        }
        target.set_char(position, cell);
        true
    }

    fn written(changed: usize, clipped: usize) -> String {
        let mut text = format!("Changed {changed} cells in the draft");
        if clipped > 0 {
            text.push_str(&format!("; {clipped} cells outside the layer were skipped"));
        }
        text.push_str(". The user previews the draft and decides whether to apply it.");
        text
    }

    fn read_region(&self, arguments: &Value) -> Result<String, String> {
        let layer_index = self.layer_index(arguments, false)?;
        let layer = &self.buffer.layers[layer_index];
        let number = |key: &str, default: i64| arguments.get(key).and_then(Value::as_i64).unwrap_or(default);
        let x = number("x", 0).max(0);
        let y = number("y", 0).max(0);
        let right = (x + number("width", i64::from(layer.width()))).min(i64::from(layer.width()));
        let bottom = (y + number("height", i64::from(layer.height()))).min(i64::from(layer.height()));
        if right <= x || bottom <= y {
            return Err("The region is empty or outside the layer".into());
        }
        if (right - x) * (bottom - y) > MAX_READ_CELLS {
            return Err(format!("Read at most {MAX_READ_CELLS} cells at a time; use a smaller region"));
        }
        let color = |color: AttributeColor| match color {
            AttributeColor::Palette(index) | AttributeColor::ExtendedPalette(index) => json!(index),
            AttributeColor::Rgb(r, g, b) => json!(format!("#{r:02x}{g:02x}{b:02x}")),
            AttributeColor::Transparent => Value::Null,
        };
        let mut rows = Vec::new();
        for row in y..bottom {
            let mut text = String::new();
            // Runs of equal colors: [start x, length, foreground, background].
            let mut runs: Vec<(i64, i64, Value, Value)> = Vec::new();
            for column in x..right {
                let cell = layer.char_at(Position::new(column as i32, row as i32));
                let (character, foreground, background) = if cell.is_visible() {
                    (
                        self.buffer.buffer_type.convert_to_unicode(cell.ch),
                        color(cell.attribute.foreground_color()),
                        color(cell.attribute.background_color()),
                    )
                } else {
                    (' ', Value::Null, Value::Null)
                };
                text.push(character);
                match runs.last_mut() {
                    Some(run) if run.2 == foreground && run.3 == background => run.1 += 1,
                    _ => runs.push((column, 1, foreground, background)),
                }
            }
            let runs: Vec<_> = runs.into_iter().map(|(start, length, fg, bg)| json!([start, length, fg, bg])).collect();
            rows.push(json!({ "y": row, "text": text, "colors": runs }));
        }
        Ok(json!({ "layer": layer_index, "x": x, "rows": rows, "colors_format": "[start_x, length, fg, bg]" }).to_string())
    }

    fn draw_text(&mut self, arguments: &Value) -> Result<String, String> {
        let layer = self.layer_index(arguments, true)?;
        let x = arguments.get("x").and_then(Value::as_i64).ok_or("x is required")?;
        let y = arguments.get("y").and_then(Value::as_i64).ok_or("y is required")?;
        let text = arguments.get("text").and_then(Value::as_str).ok_or("text is required")?;
        let foreground = self.color(arguments, "fg")?;
        let background = self.color(arguments, "bg")?;
        if text.chars().count() > MAX_WRITE_CELLS {
            return Err(format!("Write at most {MAX_WRITE_CELLS} characters at a time"));
        }
        let (mut changed, mut clipped) = (0, 0);
        for (row, line) in text.split('\n').enumerate() {
            for (column, character) in line.chars().enumerate() {
                let character = self.buffer.buffer_type.convert_from_unicode(character);
                if self.put(layer, x + column as i64, y + row as i64, Some(character), foreground, background) {
                    changed += 1;
                } else {
                    clipped += 1;
                }
            }
        }
        Ok(Self::written(changed, clipped))
    }

    fn fill_rect(&mut self, arguments: &Value) -> Result<String, String> {
        let layer = self.layer_index(arguments, true)?;
        let number = |key: &str| arguments.get(key).and_then(Value::as_i64).ok_or(format!("{key} is required"));
        let (x, y, width, height) = (number("x")?, number("y")?, number("width")?, number("height")?);
        let character = self.character(arguments.get("char"))?;
        let foreground = self.color(arguments, "fg")?;
        let background = self.color(arguments, "bg")?;
        if width <= 0 || height <= 0 {
            return Err("width and height must be positive".into());
        }
        if character.is_none() && foreground.is_none() && background.is_none() {
            return Err("Set at least one of char, fg or bg".into());
        }
        if width.saturating_mul(height) > MAX_WRITE_CELLS as i64 {
            return Err(format!("Fill at most {MAX_WRITE_CELLS} cells at a time"));
        }
        let (mut changed, mut clipped) = (0, 0);
        for row in y..y + height {
            for column in x..x + width {
                if self.put(layer, column, row, character, foreground, background) {
                    changed += 1;
                } else {
                    clipped += 1;
                }
            }
        }
        Ok(Self::written(changed, clipped))
    }

    fn set_cells(&mut self, arguments: &Value) -> Result<String, String> {
        let layer = self.layer_index(arguments, true)?;
        let cells = arguments.get("cells").and_then(Value::as_array).ok_or("cells must be an array")?;
        if cells.len() > MAX_WRITE_CELLS {
            return Err(format!("Set at most {MAX_WRITE_CELLS} cells at a time"));
        }
        // Validate everything first so a bad cell leaves the draft untouched.
        let mut parsed = Vec::with_capacity(cells.len());
        for (index, cell) in cells.iter().enumerate() {
            let x = cell.get("x").and_then(Value::as_i64).ok_or(format!("cells[{index}].x is required"))?;
            let y = cell.get("y").and_then(Value::as_i64).ok_or(format!("cells[{index}].y is required"))?;
            let character = self.character(cell.get("char")).map_err(|error| format!("cells[{index}]: {error}"))?;
            let foreground = self.color(cell, "fg").map_err(|error| format!("cells[{index}]: {error}"))?;
            let background = self.color(cell, "bg").map_err(|error| format!("cells[{index}]: {error}"))?;
            parsed.push((x, y, character, foreground, background));
        }
        let (mut changed, mut clipped) = (0, 0);
        for (x, y, character, foreground, background) in parsed {
            if self.put(layer, x, y, character, foreground, background) {
                changed += 1;
            } else {
                clipped += 1;
            }
        }
        Ok(Self::written(changed, clipped))
    }
}

/// Name, description and JSON schema of each drawing tool.
pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let layer = json!({ "type": "integer", "minimum": 0, "description": "Layer index; defaults to the current layer" });
    let color = |what: &str| json!({ "type": "integer", "minimum": 0, "description": format!("{what} palette index; omit to keep the existing color") });
    vec![
        (
            "icy_canvas_info",
            "Describe the open drawing: size, character encoding, palette, layers, current layer and selection. Call this first.",
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "icy_read_region",
            "Read characters and colors of a rectangular area (at most 4000 cells) from the draft.",
            json!({
                "type": "object",
                "properties": {
                    "layer": layer,
                    "x": { "type": "integer" },
                    "y": { "type": "integer" },
                    "width": { "type": "integer", "minimum": 1 },
                    "height": { "type": "integer", "minimum": 1 },
                },
            }),
        ),
        (
            "icy_draw_text",
            "Write text starting at x,y. Newlines continue on the next row at the same x. Spaces overwrite cells. \
             Good for ASCII/ANSI art blocks: pass several lines at once.",
            json!({
                "type": "object",
                "properties": {
                    "layer": layer,
                    "x": { "type": "integer" },
                    "y": { "type": "integer" },
                    "text": { "type": "string" },
                    "fg": color("Foreground"),
                    "bg": color("Background"),
                },
                "required": ["x", "y", "text"],
            }),
        ),
        (
            "icy_fill_rect",
            "Fill a rectangle with a character and/or colors. Omitted values keep the existing cell content.",
            json!({
                "type": "object",
                "properties": {
                    "layer": layer,
                    "x": { "type": "integer" },
                    "y": { "type": "integer" },
                    "width": { "type": "integer", "minimum": 1 },
                    "height": { "type": "integer", "minimum": 1 },
                    "char": { "type": "string", "description": "A single character, e.g. \"█\" or \" \"" },
                    "fg": color("Foreground"),
                    "bg": color("Background"),
                },
                "required": ["x", "y", "width", "height"],
            }),
        ),
        (
            "icy_set_cells",
            "Set individual cells. Each cell has x, y and optional char, fg and bg.",
            json!({
                "type": "object",
                "properties": {
                    "layer": layer,
                    "cells": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "x": { "type": "integer" },
                                "y": { "type": "integer" },
                                "char": { "type": "string" },
                                "fg": { "type": "integer", "minimum": 0 },
                                "bg": { "type": "integer", "minimum": 0 },
                            },
                            "required": ["x", "y"],
                        },
                    },
                },
                "required": ["cells"],
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> Draft {
        Draft::new("ANSI/ASCII", TextBuffer::new((20, 5)), 0, None)
    }

    #[test]
    fn info_describes_canvas_and_palette() {
        let info: Value = serde_json::from_str(&draft().call("icy_canvas_info", &json!({})).unwrap()).unwrap();
        assert_eq!(info["width"], 20);
        assert_eq!(info["height"], 5);
        assert_eq!(info["palette"].as_array().unwrap().len(), 16);
        assert_eq!(info["layers"][0]["locked"], false);
    }

    #[test]
    fn draw_text_converts_to_cp437_clips_and_reports_changes() {
        let mut draft = draft();
        let result = draft
            .call("icy_draw_text", &json!({ "x": 18, "y": 0, "text": "█▀Hi\nok", "fg": 14, "bg": 1 }))
            .unwrap();
        assert!(result.contains("Changed 4 cells") && result.contains("2 cells outside"), "{result}");
        let cell = draft.buffer.layers[0].char_at(Position::new(18, 0));
        assert_eq!(cell.ch as u32, 219);
        assert_eq!(cell.attribute.foreground_color(), AttributeColor::Palette(14));
        assert_eq!(draft.buffer.layers[0].char_at(Position::new(19, 1)).ch, 'k');
        assert_eq!(draft.changes().len(), 4);
        assert!(
            draft.original.layers[0].char_at(Position::new(18, 0)).ch != cell.ch,
            "the original stays untouched"
        );
    }

    #[test]
    fn fill_keeps_unspecified_values_and_validates_input() {
        let mut draft = draft();
        draft.call("icy_draw_text", &json!({ "x": 0, "y": 0, "text": "AB" })).unwrap();
        draft
            .call("icy_fill_rect", &json!({ "x": 0, "y": 0, "width": 2, "height": 1, "bg": 4 }))
            .unwrap();
        let cell = draft.buffer.layers[0].char_at(Position::new(1, 0));
        assert_eq!((cell.ch, cell.attribute.background_color()), ('B', AttributeColor::Palette(4)));
        for arguments in [
            json!({ "x": 0, "y": 0, "width": 1, "height": 1 }),
            json!({ "x": 0, "y": 0, "width": 0, "height": 1, "char": "x" }),
            json!({ "x": 0, "y": 0, "width": 1, "height": 1, "fg": 16 }),
            json!({ "x": 0, "y": 0, "width": 1, "height": 1, "char": "ab" }),
            json!({ "x": 0, "y": 0, "width": 1000, "height": 1000, "char": "x" }),
            json!({ "layer": 3, "x": 0, "y": 0, "width": 1, "height": 1, "char": "x" }),
        ] {
            assert!(draft.call("icy_fill_rect", &arguments).is_err(), "{arguments}");
        }
    }

    #[test]
    fn invalid_cells_leave_the_draft_untouched_and_locked_layers_are_refused() {
        let mut draft = draft();
        let cells = json!({ "cells": [{ "x": 0, "y": 0, "char": "X" }, { "x": 1, "y": 0, "fg": 99 }] });
        assert!(draft.call("icy_set_cells", &cells).is_err());
        assert!(draft.changes().is_empty());
        draft.buffer.layers[0].properties.is_locked = true;
        assert!(draft
            .call("icy_draw_text", &json!({ "x": 0, "y": 0, "text": "X" }))
            .unwrap_err()
            .contains("locked"));
        assert!(draft.call("icy_read_region", &json!({})).is_ok(), "locked layers can still be read");
    }

    #[test]
    fn read_region_returns_text_and_color_runs() {
        let mut draft = draft();
        draft.call("icy_draw_text", &json!({ "x": 1, "y": 1, "text": "▄▄a", "fg": 2 })).unwrap();
        let read: Value = serde_json::from_str(&draft.call("icy_read_region", &json!({ "x": 0, "y": 1, "width": 5, "height": 1 })).unwrap()).unwrap();
        assert_eq!(read["rows"][0]["text"], " ▄▄a ");
        assert_eq!(read["rows"][0]["colors"][1], json!([1, 3, 2, 0]));
        assert!(draft.call("icy_read_region", &json!({ "x": 30 })).is_err());
        let mut large = Draft::new("ANSI/ASCII", TextBuffer::new((200, 100)), 0, None);
        assert!(large.call("icy_read_region", &json!({})).unwrap_err().contains("4000"));
        assert!(large.call("icy_unknown", &json!({})).is_err());
    }

    #[test]
    fn tool_names_are_valid_for_copilot() {
        for (name, description, schema) in tool_specs() {
            assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            assert!(!description.is_empty());
            assert_eq!(schema["type"], "object");
        }
    }
}
