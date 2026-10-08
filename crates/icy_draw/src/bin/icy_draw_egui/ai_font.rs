//! Bitmap font tools with pixel/hex rows, atomic batches and draft-only transformations.

use icy_engine::BufferType;
use serde_json::{json, Value};
use std::collections::HashSet;

const MAX_GLYPHS_PER_CALL: usize = 64;
const MAX_COMPACT_READ_GLYPHS: usize = 512;
const MAX_PREVIEW_TEXT: usize = 32;

pub type Glyph = Vec<Vec<bool>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowFormat {
    Pixels,
    Hex,
}

impl RowFormat {
    fn parse(arguments: &Value) -> Result<Self, String> {
        match arguments.get("format") {
            None => Ok(Self::Pixels),
            Some(Value::String(format)) if format == "pixels" => Ok(Self::Pixels),
            Some(Value::String(format)) if format == "hex" => Ok(Self::Hex),
            Some(_) => Err("format must be 'pixels' or 'hex'".into()),
        }
    }
}

enum Transform {
    Bold { amount: usize, left: bool },
    Shift { dx: i64, dy: i64, wrap: bool },
    FlipX,
    FlipY,
    Invert,
    Clear,
}

impl Transform {
    fn parse(arguments: &Value, width: usize, height: usize) -> Result<Self, String> {
        match arguments.get("operation").and_then(Value::as_str).ok_or("operation is required")? {
            "bold" => {
                let amount = match arguments.get("amount") {
                    None => 1,
                    Some(value) => value
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or("amount must be a positive integer")?,
                };
                if amount == 0 || amount > width {
                    return Err(format!("amount must be between 1 and {width}"));
                }
                let left = match arguments.get("direction") {
                    None => false,
                    Some(Value::String(direction)) if direction == "right" => false,
                    Some(Value::String(direction)) if direction == "left" => true,
                    Some(_) => return Err("direction must be 'left' or 'right'".into()),
                };
                Ok(Self::Bold { amount, left })
            }
            "shift" => {
                if arguments.get("dx").is_none() && arguments.get("dy").is_none() {
                    return Err("shift requires dx or dy".into());
                }
                let offset = |name: &str, bound: usize| -> Result<i64, String> {
                    let value = match arguments.get(name) {
                        None => 0,
                        Some(value) => value.as_i64().ok_or_else(|| format!("{name} must be an integer"))?,
                    };
                    if value.unsigned_abs() > bound as u64 {
                        return Err(format!("{name} must be between -{bound} and {bound}"));
                    }
                    Ok(value)
                };
                let wrap = match arguments.get("wrap") {
                    None => false,
                    Some(value) => value.as_bool().ok_or("wrap must be a boolean")?,
                };
                Ok(Self::Shift {
                    dx: offset("dx", width)?,
                    dy: offset("dy", height)?,
                    wrap,
                })
            }
            "flip_x" => Ok(Self::FlipX),
            "flip_y" => Ok(Self::FlipY),
            "invert" => Ok(Self::Invert),
            "clear" => Ok(Self::Clear),
            operation => Err(format!("Unknown glyph operation {operation:?}")),
        }
    }

    fn apply(&self, source: &Glyph, width: usize, height: usize) -> (Glyph, usize) {
        let mut result = vec![vec![false; width]; height];
        let mut clipped = 0;
        for (y, row) in source.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                match *self {
                    Self::Bold { amount, left } if pixel => {
                        for distance in 0..=amount {
                            let target = if left { x.checked_sub(distance) } else { x.checked_add(distance) };
                            if let Some(target) = target.filter(|&target| target < width) {
                                result[y][target] = true;
                            }
                        }
                    }
                    Self::Shift { dx, dy, wrap } if pixel => {
                        let (mut target_x, mut target_y) = (x as i64 + dx, y as i64 + dy);
                        if wrap {
                            target_x = target_x.rem_euclid(width as i64);
                            target_y = target_y.rem_euclid(height as i64);
                        }
                        if target_x >= 0 && target_y >= 0 && target_x < width as i64 && target_y < height as i64 {
                            result[target_y as usize][target_x as usize] = true;
                        } else {
                            clipped += 1;
                        }
                    }
                    Self::FlipX => result[y][width - 1 - x] = pixel,
                    Self::FlipY => result[height - 1 - y][x] = pixel,
                    Self::Invert => result[y][x] = !pixel,
                    Self::Bold { .. } | Self::Shift { .. } | Self::Clear => {}
                }
            }
        }
        (result, clipped)
    }
}

#[derive(Clone)]
pub struct FontDraft {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub selected: u32,
    /// The glyphs when editing started; changes and stale checks compare against them.
    pub original: Vec<Glyph>,
    pub glyphs: Vec<Glyph>,
}

impl FontDraft {
    pub fn new(name: &str, width: i32, height: i32, selected: u32, glyphs: Vec<Glyph>) -> Self {
        Self {
            name: name.to_owned(),
            width: width.max(0) as usize,
            height: height.max(0) as usize,
            selected,
            original: glyphs.clone(),
            glyphs,
        }
    }

    /// Codes of the glyphs that differ from the original.
    pub fn changed_codes(&self) -> Vec<u32> {
        (0..self.glyphs.len())
            .filter(|&code| self.glyphs[code] != self.original[code])
            .map(|code| code as u32)
            .collect()
    }

    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<String, String> {
        match tool {
            "icy_font_info" => Ok(self.info().to_string()),
            "icy_read_glyphs" => self.read(arguments),
            "icy_write_glyph" => self.write(arguments),
            "icy_write_glyphs" => self.write_batch(arguments),
            "icy_transform_glyphs" => self.transform(arguments),
            "icy_preview_font_text" => self.preview_text(arguments),
            _ => Err(format!("Unknown tool {tool}")),
        }
    }

    fn info(&self) -> Value {
        json!({
            "editor": "bitmap font",
            "name": self.name,
            "glyph_width": self.width,
            "glyph_height": self.height,
            "glyph_count": self.glyphs.len(),
            "selected_code": self.selected,
            "selected_char": character(self.selected),
            "row_formats": ["pixels", "hex"],
            "max_write_batch": MAX_GLYPHS_PER_CALL,
            "max_hex_read": MAX_COMPACT_READ_GLYPHS,
            "blank_glyph_count": self.glyphs.iter().filter(|glyph| glyph.iter().flatten().all(|&pixel| !pixel)).count(),
            "notes": "Glyph codes are CP437 positions (0-255, or more for 512-glyph fonts). Rows run top to bottom; \
                      pixels rows use '#' and '.'; hex rows use two hex digits per byte, MSB/leftmost pixel first, \
                      with zero padding in unused low bits. Prefer hex and icy_write_glyphs for multi-glyph work. \
                      Use icy_transform_glyphs for mechanical changes instead of regenerating bitmaps.",
        })
    }

    fn code(&self, value: &Value) -> Result<usize, String> {
        let code = match value {
            Value::Number(number) => number
                .as_u64()
                .and_then(|code| usize::try_from(code).ok())
                .ok_or("Glyph codes are non-negative integers")?,
            Value::String(text) => {
                let mut characters = text.chars();
                match (characters.next(), characters.next()) {
                    (Some(character), None) => BufferType::CP437.convert_from_unicode(character) as usize,
                    _ => return Err(format!("{text:?} is not a single character")),
                }
            }
            _ => return Err("A glyph is a code number or a single character".into()),
        };
        if code >= self.glyphs.len() {
            return Err(format!("Glyph {code} does not exist; the font has {} glyphs", self.glyphs.len()));
        }
        Ok(code)
    }

    fn source(&self, code: usize) -> Result<&Glyph, String> {
        if self.width == 0 || self.height == 0 {
            return Err("Font dimensions must be positive".into());
        }
        let glyph = &self.glyphs[code];
        if glyph.len() != self.height || glyph.iter().any(|row| row.len() != self.width) {
            return Err(format!("Glyph {code} does not match the font dimensions"));
        }
        Ok(glyph)
    }

    fn glyph_json(&self, code: usize, format: RowFormat) -> Result<Value, String> {
        let rows: Vec<String> = self
            .source(code)?
            .iter()
            .map(|row| match format {
                RowFormat::Pixels => row.iter().map(|&set| if set { '#' } else { '.' }).collect(),
                RowFormat::Hex => row
                    .chunks(8)
                    .map(|chunk| {
                        let byte = chunk
                            .iter()
                            .enumerate()
                            .fold(0u8, |byte, (bit, &set)| if set { byte | (1 << (7 - bit)) } else { byte });
                        format!("{byte:02X}")
                    })
                    .collect(),
            })
            .collect();
        Ok(json!({ "code": code, "char": character(code as u32), "rows": rows }))
    }

    fn codes(&self, arguments: &Value, all_by_default: bool, limit: usize) -> Result<Vec<usize>, String> {
        let selectors = ["codes", "text", "from"].iter().filter(|&&key| arguments.get(key).is_some()).count();
        if selectors > 1 || (arguments.get("to").is_some() && arguments.get("from").is_none()) {
            return Err("Choose codes, text, or from/to, not a combination; to requires from".into());
        }
        let codes: Vec<usize> = if let Some(list) = arguments.get("codes") {
            let list = list.as_array().ok_or("codes must be an array")?;
            if list.len() > limit {
                return Err(format!("Select at most {limit} glyphs"));
            }
            list.iter().map(|value| self.code(value)).collect::<Result<_, _>>()?
        } else if let Some(text) = arguments.get("text") {
            let text = text.as_str().ok_or("text must be a string")?;
            if text.chars().count() > limit {
                return Err(format!("Select at most {limit} glyphs"));
            }
            text.chars()
                .map(|character| self.code(&Value::String(character.to_string())))
                .collect::<Result<_, _>>()?
        } else if let Some(from) = arguments.get("from") {
            let from = self.code(from)?;
            let to = arguments.get("to").map(|to| self.code(to)).transpose()?.unwrap_or(from);
            if to < from {
                return Err("to must not be smaller than from".into());
            }
            if to - from + 1 > limit {
                return Err(format!("Select at most {limit} glyphs"));
            }
            (from..=to).collect()
        } else if all_by_default {
            (0..self.glyphs.len()).collect()
        } else {
            vec![self.code(&json!(self.selected))?]
        };
        if codes.is_empty() {
            return Err("Select at least one glyph".into());
        }
        Ok(codes)
    }

    fn read(&self, arguments: &Value) -> Result<String, String> {
        let format = RowFormat::parse(arguments)?;
        let limit = if format == RowFormat::Hex {
            MAX_COMPACT_READ_GLYPHS
        } else {
            MAX_GLYPHS_PER_CALL
        };
        let codes = self.codes(arguments, false, limit)?;
        let glyphs: Vec<_> = codes.into_iter().map(|code| self.glyph_json(code, format)).collect::<Result<_, _>>()?;
        let mut output = json!({ "width": self.width, "height": self.height, "glyphs": glyphs });
        if format == RowFormat::Hex {
            output["format"] = "hex".into();
        }
        Ok(output.to_string())
    }

    fn write(&mut self, arguments: &Value) -> Result<String, String> {
        let code = self.code(arguments.get("code").ok_or("code is required")?)?;
        let glyph = self.parse_glyph(arguments, RowFormat::parse(arguments)?)?;
        self.glyphs[code] = glyph;
        Ok(format!(
            "Updated glyph {code} ({}) in the draft. Successful font drafts are automatically applied at turn completion as one undo step, unless the target changed.",
            character(code as u32)
        ))
    }

    fn parse_glyph(&self, arguments: &Value, format: RowFormat) -> Result<Glyph, String> {
        if self.width == 0 || self.height == 0 {
            return Err("Font dimensions must be positive".into());
        }
        let rows = arguments.get("rows").and_then(Value::as_array).ok_or("rows must be an array of strings")?;
        if rows.len() != self.height {
            return Err(format!("Expected {} rows, got {}", self.height, rows.len()));
        }
        let mut glyph = Vec::with_capacity(self.height);
        for (index, row) in rows.iter().enumerate() {
            let row = row.as_str().ok_or(format!("rows[{index}] must be a string"))?;
            if format == RowFormat::Hex {
                let bytes = self.width.div_ceil(8);
                if !row.is_ascii() || row.len() != bytes * 2 {
                    return Err(format!("rows[{index}] must have exactly {} hexadecimal digits", bytes * 2));
                }
                if !row.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(format!("rows[{index}] contains invalid hexadecimal digits"));
                }
                let packed: Vec<u8> = (0..bytes)
                    .map(|byte| u8::from_str_radix(&row[byte * 2..byte * 2 + 2], 16).map_err(|_| format!("rows[{index}] contains invalid hexadecimal digits")))
                    .collect::<Result<_, _>>()?;
                let padding = (8 - self.width % 8) % 8;
                if padding > 0 && packed[bytes - 1] & ((1 << padding) - 1) != 0 {
                    return Err(format!("rows[{index}] has nonzero padding bits beyond glyph_width"));
                }
                glyph.push((0..self.width).map(|pixel| packed[pixel / 8] & (1 << (7 - pixel % 8)) != 0).collect());
                continue;
            }
            let pixels: Vec<bool> = row
                .chars()
                .map(|pixel| match pixel {
                    '#' | 'X' | 'x' | '1' | '█' => Ok(true),
                    '.' | ' ' | '0' | '_' => Ok(false),
                    other => Err(format!("rows[{index}] contains {other:?}; use '#' and '.'")),
                })
                .collect::<Result<_, _>>()?;
            if pixels.len() != self.width {
                return Err(format!("rows[{index}] has {} pixels; expected {}", pixels.len(), self.width));
            }
            glyph.push(pixels);
        }
        Ok(glyph)
    }

    fn write_batch(&mut self, arguments: &Value) -> Result<String, String> {
        let format = RowFormat::parse(arguments)?;
        let glyphs = arguments.get("glyphs").and_then(Value::as_array).ok_or("glyphs must be an array")?;
        if glyphs.is_empty() || glyphs.len() > MAX_GLYPHS_PER_CALL {
            return Err(format!("Write between 1 and {MAX_GLYPHS_PER_CALL} glyphs per batch"));
        }
        let mut codes = HashSet::new();
        let mut updates = Vec::with_capacity(glyphs.len());
        for glyph in glyphs {
            if glyph.get("format").is_some() {
                return Err("Specify format once at the batch level".into());
            }
            let code = self.code(glyph.get("code").ok_or("Every glyph requires code")?)?;
            if !codes.insert(code) {
                return Err(format!("Duplicate glyph {code} in batch"));
            }
            let pixels = self.parse_glyph(glyph, format).map_err(|error| format!("Glyph {code}: {error}"))?;
            updates.push((code, pixels));
        }
        Ok(self.commit_updates(updates, 0))
    }

    fn transform(&mut self, arguments: &Value) -> Result<String, String> {
        if self.width == 0 || self.height == 0 {
            return Err("Font dimensions must be positive".into());
        }
        let operation = Transform::parse(arguments, self.width, self.height)?;
        let codes = self.codes(arguments, true, self.glyphs.len())?;
        let mut unique = HashSet::new();
        let mut updates = Vec::with_capacity(codes.len());
        let mut clipped = 0;
        for code in codes {
            if !unique.insert(code) {
                return Err(format!("Duplicate glyph {code} in transformation"));
            }
            let (glyph, lost) = operation.apply(self.source(code)?, self.width, self.height);
            clipped += lost;
            updates.push((code, glyph));
        }
        Ok(self.commit_updates(updates, clipped))
    }

    fn commit_updates(&mut self, updates: Vec<(usize, Glyph)>, clipped: usize) -> String {
        let mut changed = Vec::new();
        let count = updates.len();
        for (code, glyph) in updates {
            if self.glyphs[code] != glyph {
                changed.push(code);
            }
            self.glyphs[code] = glyph;
        }
        json!({
            "targeted_count": count, "changed_count": changed.len(), "changed_codes": changed,
            "clipped_source_pixels": clipped,
            "note": "Draft only until successful turn completion, then automatically applied as one undo step unless the target changed.",
        })
        .to_string()
    }

    /// Lays out text with the draft glyphs, as an ASCII-art check of spacing and consistency.
    fn preview_text(&self, arguments: &Value) -> Result<String, String> {
        let text = arguments.get("text").and_then(Value::as_str).ok_or("text is required")?;
        if text.chars().count() > MAX_PREVIEW_TEXT {
            return Err(format!("Preview at most {MAX_PREVIEW_TEXT} characters"));
        }
        let codes: Vec<usize> = text
            .chars()
            .map(|character| self.code(&Value::String(character.to_string())))
            .collect::<Result<_, _>>()?;
        let mut lines = Vec::with_capacity(self.height);
        for row in 0..self.height {
            let line: String = codes
                .iter()
                .flat_map(|&code| self.glyphs[code].get(row).into_iter().flatten())
                .map(|&set| if set { '#' } else { '.' })
                .collect();
            lines.push(line);
        }
        Ok(lines.join("\n"))
    }
}

/// The CP437 character a glyph code stands for.
fn character(code: u32) -> String {
    char::from_u32(code)
        .map(|character| BufferType::CP437.convert_to_unicode(character).to_string())
        .unwrap_or_default()
}

pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let code = json!({ "description": "Glyph code (integer) or a single character", "type": ["integer", "string"] });
    let format = json!({
        "type": "string", "enum": ["pixels", "hex"], "default": "pixels",
        "description": "pixels: '#'/'.' rows. hex: two digits per byte, leftmost pixel is MSB; unused low padding bits must be zero.",
    });
    let rows = json!({ "type": "array", "items": { "type": "string" } });
    vec![
        (
            "icy_font_info",
            "[Bitmap font editor] Describe the font: name, glyph size, glyph count and the selected glyph.",
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "icy_read_glyphs",
            "[Bitmap font editor] Read glyph bitmaps. Prefer format='hex' for compact rows (up to 512 glyphs); \
             default pixels rows use '#' and '.' (up to 64 glyphs). Choose codes, text, or from/to; \
             without a selector the selected glyph is returned.",
            json!({
                "type": "object",
                "properties": {
                    "codes": { "type": "array", "items": code, "maxItems": MAX_COMPACT_READ_GLYPHS, "minItems": 1 },
                    "text": { "type": "string", "description": "Characters whose glyphs to read" },
                    "from": code,
                    "to": code,
                    "format": format,
                },
            }),
        ),
        (
            "icy_write_glyph",
            "[Bitmap font editor] Replace one glyph. Prefer icy_write_glyphs for multiple glyphs. \
             rows has exactly glyph_height strings; format='hex' uses packed MSB-first hexadecimal rows, \
             default format='pixels' uses glyph_width '#'/'.' characters.",
            json!({
                "type": "object",
                "properties": {
                    "code": code,
                    "rows": rows,
                    "format": format,
                },
                "required": ["code", "rows"],
            }),
        ),
        (
            "icy_write_glyphs",
            "[Bitmap font editor] Atomically replace 1-64 glyphs in one call. Prefer format='hex' to reduce output. \
             Every glyph has code and glyph_height rows; no duplicate codes. All glyphs are validated before \
             any changes are made. Set format once for the entire batch.",
            json!({
                "type": "object",
                "properties": {
                    "format": format,
                    "glyphs": {
                        "type": "array", "minItems": 1, "maxItems": MAX_GLYPHS_PER_CALL,
                        "items": {
                            "type": "object", "additionalProperties": false,
                            "properties": { "code": code, "rows": rows },
                            "required": ["code", "rows"],
                        },
                    },
                },
                "required": ["glyphs"],
            }),
        ),
        (
            "icy_transform_glyphs",
            "[Bitmap font editor] Apply a mechanical change to the entire font in one call, or choose codes, text, \
             or from/to. No selector means ALL glyphs, including CP437 graphics. Operations: bold (amount=1, \
             direction=right by default; grows strokes without cascading), shift (dx/dy, positive=right/down, \
             clips by default; wrap=true rotates), flip_x, flip_y, invert, clear. Dimensions never change. \
             Atomic validation, no duplicate target codes. Use this instead of reading/writing every glyph \
             for mechanical edits. Shift reports clipped source pixels.",
            json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": ["bold", "shift", "flip_x", "flip_y", "invert", "clear"] },
                    "amount": { "type": "integer", "minimum": 1 },
                    "direction": { "type": "string", "enum": ["left", "right"] },
                    "dx": { "type": "integer" }, "dy": { "type": "integer" },
                    "wrap": { "type": "boolean" },
                    "codes": { "type": "array", "items": code, "minItems": 1 },
                    "text": { "type": "string" },
                    "from": code, "to": code,
                },
                "required": ["operation"],
            }),
        ),
        (
            "icy_preview_font_text",
            "[Bitmap font editor] Render up to 32 characters side by side with the draft glyphs to check them.",
            json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> FontDraft {
        FontDraft::new("Test", 3, 2, 65, vec![vec![vec![false; 3]; 2]; 256])
    }

    #[test]
    fn hex_round_trips_byte_aligned_and_padded_widths() {
        for width in [1, 3, 7, 8, 9, 16, 17] {
            let source: Glyph = (0..3).map(|row| (0..width).map(|column| (row + column) % 3 == 0).collect()).collect();
            let mut draft = FontDraft::new("Hex", width, 3, 0, vec![source.clone(), vec![vec![false; width as usize]; 3]]);
            let read: Value = serde_json::from_str(&draft.call("icy_read_glyphs", &json!({"codes": [0], "format": "hex"})).unwrap()).unwrap();
            assert_eq!(read["format"], "hex");
            let rows = &read["glyphs"][0]["rows"];
            assert!(rows
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row.as_str().unwrap().len() == (width as usize).div_ceil(8) * 2));
            draft.call("icy_write_glyph", &json!({"code": 1, "format": "hex", "rows": rows})).unwrap();
            assert_eq!(draft.glyphs[1], source, "width {width}");
        }
        let mut draft = draft();
        draft
            .call("icy_write_glyph", &json!({"code": "A", "format": "hex", "rows": ["a0", "40"]}))
            .unwrap();
        assert_eq!(draft.call("icy_preview_font_text", &json!({"text": "A"})).unwrap(), "#.#\n.#.");
        let read: Value = serde_json::from_str(&draft.call("icy_read_glyphs", &json!({"format": "hex"})).unwrap()).unwrap();
        assert_eq!(read["glyphs"][0]["rows"], json!(["A0", "40"]));
    }

    #[test]
    fn batches_validate_every_glyph_before_committing() {
        let mut draft = draft();
        let original = draft.glyphs.clone();
        for arguments in [
            json!({"glyphs": []}),
            json!({"glyphs": vec![json!({"code": 0, "rows": ["###", "###"]}); MAX_GLYPHS_PER_CALL + 1]}),
            json!({"glyphs": [{"code": "A", "rows": ["###", "###"]}, {"code": 66, "rows": ["invalid"]}]}),
            json!({"glyphs": [{"code": "A", "rows": ["###", "###"]}, {"code": 65, "rows": ["...", "..."]}]}),
            json!({"format": "hex", "glyphs": [{"code": 65, "rows": ["E0", "E0"]}, {"code": 66, "rows": ["E1", "E0"]}]}),
            json!({"format": "hex", "glyphs": [{"code": 65, "format": "pixels", "rows": ["###", "###"]}]}),
            json!({"format": "unknown", "glyphs": [{"code": 65, "rows": ["###", "###"]}]}),
        ] {
            assert!(draft.call("icy_write_glyphs", &arguments).is_err(), "{arguments}");
            assert_eq!(draft.glyphs, original);
        }
        let result: Value = serde_json::from_str(
            &draft
                .call(
                    "icy_write_glyphs",
                    &json!({
                        "format": "hex", "glyphs": [{"code": "A", "rows": ["A0", "40"]}, {"code": "B", "rows": ["C0", "E0"]}],
                    }),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["changed_count"], 2);
        assert_eq!(draft.changed_codes(), [65, 66]);
        assert_eq!(draft.original, original);
    }

    #[test]
    fn invalid_hex_and_selectors_are_explicit_errors() {
        let mut draft = draft();
        for rows in [
            json!(["+0", "00"]),
            json!(["FF", "00"]),
            json!(["GG", "00"]),
            json!(["A", "00"]),
            json!(["A000", "00"]),
            json!(["é", "00"]),
            json!([1, "00"]),
            json!(["A0"]),
        ] {
            assert!(
                draft.call("icy_write_glyph", &json!({"code": 65, "format": "hex", "rows": rows})).is_err(),
                "{rows}"
            );
        }
        for arguments in [
            json!({"format": 1}),
            json!({"codes": "A"}),
            json!({"text": 1}),
            json!({"codes": []}),
            json!({"to": 65}),
            json!({"codes": [65], "text": "A"}),
            json!({"from": 66, "to": 65}),
        ] {
            assert!(draft.call("icy_read_glyphs", &arguments).is_err(), "{arguments}");
        }
        assert!(draft.changed_codes().is_empty());
        let mut wide = FontDraft::new("Nine", 9, 1, 0, vec![vec![vec![false; 9]]]);
        assert!(wide.call("icy_write_glyph", &json!({"code": 0, "format": "hex", "rows": ["8001"]})).is_err());
        wide.call("icy_write_glyph", &json!({"code": 0, "format": "hex", "rows": ["8080"]})).unwrap();
        assert!(wide.glyphs[0][0][0] && wide.glyphs[0][0][8]);
        let mut empty = FontDraft::new("Empty", 8, 16, 0, vec![]);
        assert!(empty.call("icy_read_glyphs", &json!({})).is_err());
    }

    #[test]
    fn whole_256_and_512_glyph_fonts_use_four_and_eight_write_batches() {
        for count in [256, 512] {
            let mut draft = FontDraft::new("Full", 8, 16, 0, vec![vec![vec![false; 8]; 16]; count]);
            let original = draft.original.clone();
            let mut calls = 0;
            for from in (0..count).step_by(MAX_GLYPHS_PER_CALL) {
                let glyphs: Vec<_> = (from..from + MAX_GLYPHS_PER_CALL)
                    .map(|code| {
                        let rows: Vec<_> = (0..16).map(|row| format!("{:02X}", (code as u8).wrapping_add(row))).collect();
                        json!({"code": code, "rows": rows})
                    })
                    .collect();
                let result: Value = serde_json::from_str(&draft.call("icy_write_glyphs", &json!({"format": "hex", "glyphs": glyphs})).unwrap()).unwrap();
                assert_eq!(result["changed_count"], 64);
                calls += 1;
            }
            assert_eq!(calls, count / 64);
            assert_eq!(draft.changed_codes().len(), count);
            assert_eq!(draft.original, original);
            let read: Value = serde_json::from_str(&draft.call("icy_read_glyphs", &json!({"from": 0, "to": count - 1, "format": "hex"})).unwrap()).unwrap();
            assert_eq!(read["glyphs"].as_array().unwrap().len(), count);
            for glyph in read["glyphs"].as_array().unwrap() {
                let code = glyph["code"].as_u64().unwrap() as usize;
                assert_eq!(draft.parse_glyph(glyph, RowFormat::Hex).unwrap(), draft.glyphs[code]);
            }
        }
    }

    #[test]
    fn compact_reads_reduce_actual_json_bytes_by_at_least_forty_percent() {
        let mut draft = FontDraft::new("Size", 8, 16, 0, vec![vec![vec![false; 8]; 16]; 256]);
        let pixels = draft.call("icy_read_glyphs", &json!({"from": 32, "to": 95})).unwrap();
        let hex = draft.call("icy_read_glyphs", &json!({"from": 32, "to": 95, "format": "hex"})).unwrap();
        assert!(hex.len() * 100 <= pixels.len() * 60, "hex {} bytes vs pixels {} bytes", hex.len(), pixels.len());
        assert_eq!(pixels.matches("........").count(), 64 * 16);
        assert_eq!(hex.matches("\"00\"").count(), 64 * 16);
    }

    #[test]
    fn read_limits_and_font_metadata_advertise_the_efficient_tools() {
        let mut draft = FontDraft::new("Limits", 8, 16, 65, vec![vec![vec![false; 8]; 16]; 513]);
        let info: Value = serde_json::from_str(&draft.call("icy_font_info", &json!({})).unwrap()).unwrap();
        assert_eq!(info["row_formats"], json!(["pixels", "hex"]));
        assert_eq!(info["max_write_batch"], 64);
        assert_eq!(info["max_hex_read"], 512);
        assert_eq!(info["blank_glyph_count"], 513);
        assert!(draft.call("icy_read_glyphs", &json!({"from": 0, "to": 64})).unwrap_err().contains("at most 64"));
        assert!(draft
            .call("icy_read_glyphs", &json!({"from": 0, "to": 512, "format": "hex"}))
            .unwrap_err()
            .contains("at most 512"));
        let read: Value = serde_json::from_str(&draft.call("icy_read_glyphs", &json!({})).unwrap()).unwrap();
        assert_eq!(read["glyphs"].as_array().unwrap().len(), 1);
        assert_eq!(read["glyphs"][0]["code"], 65);
        assert!(read.get("format").is_none(), "legacy pixel reads keep their original response shape");
    }

    #[test]
    fn negative_shifts_clip_and_wrap_in_both_dimensions() {
        let mut draft = FontDraft::new("Shift", 3, 2, 0, vec![vec![vec![true, false, false], vec![false, false, true]]]);
        let original = draft.glyphs.clone();
        let result: Value = serde_json::from_str(&draft.call("icy_transform_glyphs", &json!({"operation": "shift", "dx": -1, "dy": -1})).unwrap()).unwrap();
        assert_eq!(result["clipped_source_pixels"], 1);
        assert_eq!(draft.glyphs[0], [vec![false, true, false], vec![false; 3]]);
        draft.glyphs = original.clone();
        let result: Value = serde_json::from_str(
            &draft
                .call("icy_transform_glyphs", &json!({"operation": "shift", "dx": -1, "dy": -1, "wrap": true}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["clipped_source_pixels"], 0);
        assert_eq!(draft.glyphs[0], [vec![false, true, false], vec![false, false, true]]);
        draft.glyphs = original.clone();
        draft
            .call("icy_transform_glyphs", &json!({"operation": "shift", "dx": -3, "dy": -2, "wrap": true}))
            .unwrap();
        assert_eq!(draft.glyphs, original, "wrapping by full dimensions is an identity");
        let result: Value = serde_json::from_str(&draft.call("icy_transform_glyphs", &json!({"operation": "shift", "dx": -3})).unwrap()).unwrap();
        assert_eq!(result["clipped_source_pixels"], 2);
        assert!(draft.glyphs[0].iter().flatten().all(|&pixel| !pixel));
    }

    #[test]
    fn transformations_keep_size_and_use_the_original_pixels_without_cascading() {
        let mut draft = FontDraft::new(
            "Transform",
            5,
            2,
            0,
            vec![vec![vec![false, false, true, false, false], vec![false; 5]], vec![vec![false; 5]; 2]],
        );
        let original = draft.original.clone();
        draft.call("icy_transform_glyphs", &json!({"operation": "bold", "codes": [0]})).unwrap();
        assert_eq!(draft.glyphs[0][0], [false, false, true, true, false]);
        assert_eq!(draft.glyphs[1], original[1]);
        draft
            .call("icy_transform_glyphs", &json!({"operation": "shift", "codes": [0], "dx": 1, "dy": 1}))
            .unwrap();
        assert_eq!(draft.glyphs[0][1], [false, false, false, true, true]);
        let result: Value = serde_json::from_str(
            &draft
                .call("icy_transform_glyphs", &json!({"operation": "shift", "codes": [0], "dx": 1}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["clipped_source_pixels"], 1);
        assert_eq!(draft.glyphs[0][1], [false, false, false, false, true]);
        draft
            .call("icy_transform_glyphs", &json!({"operation": "shift", "codes": [0], "dx": 1, "wrap": true}))
            .unwrap();
        assert_eq!(draft.glyphs[0][1], [true, false, false, false, false]);
        draft.call("icy_transform_glyphs", &json!({"operation": "flip_x", "codes": [0]})).unwrap();
        assert!(draft.glyphs[0][1][4]);
        draft.call("icy_transform_glyphs", &json!({"operation": "flip_y", "codes": [0]})).unwrap();
        assert!(draft.glyphs[0][0][4]);
        draft
            .call(
                "icy_transform_glyphs",
                &json!({"operation": "bold", "direction": "left", "amount": 2, "codes": [0]}),
            )
            .unwrap();
        assert_eq!(draft.glyphs[0][0], [false, false, true, true, true]);
        draft.call("icy_transform_glyphs", &json!({"operation": "clear", "codes": [0]})).unwrap();
        assert!(draft.glyphs[0].iter().flatten().all(|&pixel| !pixel));
        assert_eq!(draft.original, original);
    }

    #[test]
    fn full_font_transformations_take_one_call_and_can_be_scoped() {
        for count in [256, 512] {
            let mut draft = FontDraft::new("Full", 8, 16, 65, vec![vec![vec![false; 8]; 16]; count]);
            let result: Value = serde_json::from_str(&draft.call("icy_transform_glyphs", &json!({"operation": "invert"})).unwrap()).unwrap();
            assert_eq!(result["targeted_count"], count);
            assert_eq!(result["changed_count"], count);
            assert!(draft.glyphs.iter().flatten().flatten().all(|&pixel| pixel));
            draft
                .call("icy_transform_glyphs", &json!({"operation": "invert", "from": 32, "to": 126}))
                .unwrap();
            assert!(draft.glyphs[31].iter().flatten().all(|&pixel| pixel));
            assert!(draft.glyphs[32].iter().flatten().all(|&pixel| !pixel));
            assert!(draft.glyphs[127].iter().flatten().all(|&pixel| pixel));
            let result: Value = serde_json::from_str(&draft.call("icy_transform_glyphs", &json!({"operation": "clear", "text": "AB"})).unwrap()).unwrap();
            assert_eq!(result["targeted_count"], 2);
        }
    }

    #[test]
    fn transformations_are_atomic_on_invalid_operations_targets_or_source() {
        let mut draft = draft();
        let original = draft.glyphs.clone();
        for arguments in [
            json!({"operation": "unknown"}),
            json!({"operation": "bold", "amount": 0}),
            json!({"operation": "bold", "amount": 4}),
            json!({"operation": "bold", "direction": "up"}),
            json!({"operation": "shift"}),
            json!({"operation": "shift", "dx": 4}),
            json!({"operation": "shift", "dy": -3}),
            json!({"operation": "shift", "dx": 0.5}),
            json!({"operation": "shift", "dx": 1, "wrap": "true"}),
            json!({"operation": "invert", "codes": [65, 300]}),
            json!({"operation": "invert", "codes": ["A", 65]}),
            json!({"operation": "clear", "text": ""}),
        ] {
            assert!(draft.call("icy_transform_glyphs", &arguments).is_err(), "{arguments}");
            assert_eq!(draft.glyphs, original);
        }
        draft.glyphs[255].clear();
        let malformed = draft.glyphs.clone();
        assert!(draft.call("icy_transform_glyphs", &json!({"operation": "invert"})).is_err());
        assert_eq!(draft.glyphs, malformed);
    }

    #[test]
    fn reads_and_writes_glyphs_as_text_rows() {
        let mut draft = draft();
        let result = draft.call("icy_write_glyph", &json!({ "code": "A", "rows": ["#.#", ".#."] })).unwrap();
        assert!(result.contains("glyph 65"), "{result}");
        assert_eq!(draft.changed_codes(), [65]);
        let read: Value = serde_json::from_str(&draft.call("icy_read_glyphs", &json!({})).unwrap()).unwrap();
        assert_eq!(read["glyphs"][0]["rows"], json!(["#.#", ".#."]));
        let read: Value = serde_json::from_str(&draft.call("icy_read_glyphs", &json!({ "from": 64, "to": 66 })).unwrap()).unwrap();
        assert_eq!(read["glyphs"].as_array().unwrap().len(), 3);
        assert_eq!(draft.call("icy_preview_font_text", &json!({ "text": "AA" })).unwrap(), "#.##.#\n.#..#.");
        let block: Value = serde_json::from_str(&draft.call("icy_read_glyphs", &json!({ "text": "█" })).unwrap()).unwrap();
        assert_eq!(block["glyphs"][0]["code"], 219);
    }

    #[test]
    fn invalid_glyphs_are_rejected_without_changes() {
        let mut draft = draft();
        for arguments in [
            json!({ "code": 65, "rows": ["#.#"] }),
            json!({ "code": 65, "rows": ["#.", ".#."] }),
            json!({ "code": 65, "rows": ["#?#", ".#."] }),
            json!({ "code": 300, "rows": ["###", "###"] }),
            json!({ "code": "AB", "rows": ["###", "###"] }),
        ] {
            assert!(draft.call("icy_write_glyph", &arguments).is_err(), "{arguments}");
        }
        assert!(draft.changed_codes().is_empty());
        assert!(draft.call("icy_read_glyphs", &json!({ "from": 0, "to": 100 })).is_err());
        let info: Value = serde_json::from_str(&draft.call("icy_font_info", &json!({})).unwrap()).unwrap();
        assert_eq!((info["glyph_width"].as_u64(), info["selected_char"].as_str()), (Some(3), Some("A")));
    }
}
