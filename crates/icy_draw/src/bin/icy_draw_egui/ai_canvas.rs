//! Drawing tools for the AI assistant. They edit a draft copy of the document; the user
//! previews the result and accepts or discards it as a whole.

use std::collections::HashMap;

use icy_draw::screen_profile::ScreenProfile;
use icy_engine::{AttributeColor, AttributedChar, BufferType, PetsciiCase, Position, TextBuffer, TextPane};
use serde_json::{json, Value};

#[path = "ai_raster.rs"]
mod raster;

pub(super) const ANSI_GUIDANCE: &str = include_str!("../../../data/ai/references/ansi-art.md");

const MAX_READ_CELLS: i64 = 4000;
const MAX_WRITE_CELLS: usize = 8000;

#[derive(Clone, Copy)]
enum Glyph {
    Unicode(char),
    Code(u8),
}

struct CellWrite {
    x: i64,
    y: i64,
    glyph: Option<Glyph>,
    foreground: Option<AttributeColor>,
    background: Option<AttributeColor>,
}

pub(super) fn is_retro(buffer: &TextBuffer) -> bool {
    matches!(buffer.buffer_type, BufferType::Petscii | BufferType::Atascii | BufferType::AtariSt)
}

pub(super) fn character_profile(buffer: &TextBuffer) -> Value {
    let profile = ScreenProfile::of(buffer);
    let mut data = json!({
        "screen_profile": format!("{profile:?}"),
        "machine_mode": format!("{:?}", buffer.machine_mode),
        "palette_len": buffer.palette.len(),
        "per_character_colors": profile.per_character_colors(),
        "inverse_in_character": profile.inverse_in_character(),
        "per_character_background": !matches!(profile, ScreenProfile::Atascii(_) | ScreenProfile::Petscii(..)),
        "native_code_range": [0, 255],
        "notes": "char_code is a native screen glyph code, not a terminal control byte. Existing font pages are preserved. \
                  Retro Unicode writes must be representable; PETSCII text follows the current character set. \
                  glyph_codes/font_pages are authoritative; Unicode text is an approximation. \
                  Use editor palette controls for machine-wide colors, not per-cell overrides.",
    });
    if buffer.buffer_type == BufferType::Petscii {
        let (machine, case) = icy_engine::petscii_charset(buffer);
        data["petscii_machine"] = json!(format!("{machine:?}"));
        data["petscii_charset"] = json!(format!("{case:?}"));
        data["charset_per_character"] = json!(machine.charset_per_character());
        data["foreground_color_count"] = json!(machine.text_colors());
        data["shared_background"] = json!(icy_engine::petscii_background(buffer));
    }
    if is_retro(buffer) {
        data["editable_properties"] = json!(["cell glyphs", "representable text"]);
        data["read_only_properties"] = json!([
            "screen size",
            "resolution",
            "font size",
            "font bitmaps",
            "font pages",
            "machine mode",
            "global palette"
        ]);
    }
    if buffer.buffer_type == BufferType::Atascii {
        data["image_conversion"] = json!(
            "This is fixed-grid, two-tone character art, not an ANSI color image or pixel canvas. \
             Preserve the current grid, font and shared foreground/background; omit fg/bg in writes. \
             Read the actual font bitmaps with icy_read_canvas_glyphs, including inverse glyphs. \
             Reduce the picture to its silhouette, large light/dark areas and essential features, \
             choose native glyph shapes and write char_code batches. Use the cell display aspect ratio \
             to preserve proportions. Do not invent CP437 shades, resize the screen or edit the font."
        );
    }
    data
}

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
    pub reference_image: Option<super::image_attachment::ReferenceImage>,
    pub preview_enabled: bool,
    preview_calls: usize,
    conversion_calls: usize,
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
            reference_image: None,
            preview_enabled: true,
            preview_calls: 0,
            conversion_calls: 0,
        }
    }

    pub fn begin_turn(&mut self, image: Option<super::image_attachment::ReferenceImage>) {
        self.reference_image = image;
        self.preview_calls = 0;
        self.conversion_calls = 0;
        self.preview_enabled = true;
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
            "icy_read_canvas_glyphs" => self.read_glyphs(arguments),
            "icy_convert_reference_image" => self.convert_image(arguments),
            "icy_read_region" => self.read_region(arguments),
            "icy_draw_text" => self.draw_text(arguments),
            "icy_fill_rect" => self.fill_rect(arguments),
            "icy_set_cells" => self.set_cells(arguments),
            _ => Err(format!("Unknown tool {tool}")),
        }
    }

    pub(super) fn info(&self) -> Value {
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
        let mut fonts: Vec<_> = buffer
            .font_iter()
            .map(|(page, font)| {
                json!({
                    "font_page": page,
                    "name": font.name(),
                    "glyph_width": font.width,
                    "glyph_height": font.height,
                    "glyph_count": 256,
                })
            })
            .collect();
        fonts.sort_by_key(|font| font["font_page"].as_u64());
        let cell_size = buffer.font_dimensions();
        let mut display_size = buffer.font_dimensions_with_aspect_ratio();
        if buffer.use_letter_spacing() && cell_size.width == 8 {
            display_size.width = 9;
        }
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
            "character_profile": character_profile(buffer),
            "fonts": fonts,
            "font_cell_size": [cell_size.width, cell_size.height],
            "display_cell_size": [display_size.width, display_size.height],
            "letter_spacing": buffer.use_letter_spacing(),
            "aspect_ratio_correction": buffer.use_aspect_ratio(),
            "tool_capabilities": {
                "write_cells": true,
                "resize_screen": false,
                "change_resolution": false,
                "change_font": false,
                "edit_font_bitmaps": false,
                "change_global_colors": false,
                "render_preview": self.preview_enabled,
                "convert_attached_image": self.reference_image.is_some(),
            },
            "reference_image": self.reference_image.as_ref().map(|image| json!({
                "name": image.name, "width": image.width, "height": image.height,
                "note": "Most recent explicitly attached picture in this conversation. No local files are accessible.",
            })),
            "notes": "Coordinates are layer-relative, (0,0) is the top-left cell. Colors are palette indices. \
                      Characters are Unicode and converted to the document encoding; retro editors reject unmappable text. \
                      Use char_code for exact native byte glyphs, never terminal escapes. \
                      Without iCE colors, background indices 8-15 blink in classic ANSI.",
        })
    }

    fn read_glyphs(&self, arguments: &Value) -> Result<String, String> {
        let number = |key: &str, default: u64| -> Result<u64, String> {
            match arguments.get(key) {
                None => Ok(default),
                Some(value) => value.as_u64().ok_or_else(|| format!("{key} must be a nonnegative integer")),
            }
        };
        let page = u8::try_from(number("font_page", 0)?).map_err(|_| "font_page must be in 0..255")?;
        let start = number("start", 0)?;
        let count = number("count", 256)?;
        if start > 255 || count == 0 || count > 256 || start + count > 256 {
            return Err("Read 1..256 glyphs with start + count <= 256".into());
        }
        let font = self
            .buffer
            .font_for_render(page)
            .ok_or_else(|| format!("Font page {page} does not exist in the document"))?;
        if !(1..=8).contains(&font.width) || !(1..=32).contains(&font.height) {
            return Err(format!("Unsupported canvas font dimensions {}x{}", font.width, font.height));
        }
        let glyphs: Vec<_> = (start..start + count)
            .map(|code| {
                let glyph = &font.glyphs[code as usize];
                let rows: Vec<_> = (0..usize::from(font.height))
                    .map(|y| {
                        let mut row = 0u8;
                        for x in 0..usize::from(font.width) {
                            if glyph.get_pixel(x, y) {
                                row |= 0x80 >> x;
                            }
                        }
                        format!("{row:02X}")
                    })
                    .collect();
                json!({"char_code": code, "rows": rows})
            })
            .collect();
        Ok(json!({
            "font_page": page,
            "name": font.name(),
            "glyph_width": font.width,
            "glyph_height": font.height,
            "format": "hex",
            "notes": "Read-only actual font bitmaps. Rows run top to bottom; each is one hex byte, MSB/leftmost pixel first. \
                      Set bits draw the existing foreground; clear bits draw the existing background. \
                      Write the corresponding char_code, not the bitmap, to the canvas. Inverse glyphs are included as stored.",
            "glyphs": glyphs,
        })
        .to_string())
    }

    fn layer_index(&self, arguments: &Value, write: bool) -> Result<usize, String> {
        let index = match arguments.get("layer") {
            Some(value) => value
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .ok_or("layer must be a nonnegative integer")?,
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
                let mut palette = self.buffer.palette.len().min(256);
                if key == "fg" && self.buffer.buffer_type == BufferType::Petscii {
                    palette = palette.min(icy_engine::petscii_charset(&self.buffer).0.text_colors() as usize);
                }
                match u8::try_from(index) {
                    Ok(index) if usize::from(index) < palette => Ok(Some(AttributeColor::Palette(index))),
                    _ => Err(format!("{key} {index} is outside the palette (0-{})", palette - 1)),
                }
            }
        }
    }

    fn character(&self, arguments: &Value) -> Result<Option<Glyph>, String> {
        let value = arguments.get("char");
        if let Some(code) = arguments.get("char_code").filter(|value| !value.is_null()) {
            if value.is_some_and(|value| !value.is_null()) {
                return Err("Specify char or char_code, not both.".into());
            }
            let code = code
                .as_u64()
                .and_then(|code| u8::try_from(code).ok())
                .ok_or("char_code must be an integer in 0..255")?;
            return Ok(Some(Glyph::Code(code)));
        }
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let text = value.as_str().ok_or("char must be a string")?;
        let mut characters = text.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) => Ok(Some(Glyph::Unicode(character))),
            _ => Err(format!("char must be exactly one character, got {text:?}")),
        }
    }

    fn encode(&self, glyph: Glyph, font_page: u8) -> Result<char, String> {
        let character = match glyph {
            Glyph::Code(code) => return Ok(char::from(code)),
            Glyph::Unicode(character) => character,
        };
        if self.buffer.buffer_type == BufferType::Petscii {
            let (machine, mut case) = icy_engine::petscii_charset(&self.buffer);
            if machine.charset_per_character() {
                case = if font_page == 1 { PetsciiCase::Lower } else { PetsciiCase::Upper };
            }
            return icy_engine::petscii_screen_code(character, case)
                .map(char::from)
                .ok_or_else(|| format!("{character:?} is not mapped in this PETSCII character set. Use an existing native char_code."));
        }
        if is_retro(&self.buffer) {
            return self.buffer.buffer_type.try_convert_from_unicode(character).ok_or_else(|| {
                format!(
                    "{character:?} is not representable in {:?}. Use an existing native char_code.",
                    self.buffer.buffer_type
                )
            });
        }
        Ok(self.buffer.buffer_type.convert_from_unicode(character))
    }

    fn write_cells(&mut self, layer: usize, writes: Vec<CellWrite>) -> Result<String, String> {
        let target = &self.buffer.layers[layer];
        let profile = ScreenProfile::of(&self.buffer);
        let mut staged = HashMap::new();
        let (mut changed, mut clipped) = (0, 0);
        for write in writes {
            if write.x < 0 || write.y < 0 || write.x >= i64::from(target.width()) || write.y >= i64::from(target.height()) {
                clipped += 1;
                continue;
            }
            let x = i32::try_from(write.x).map_err(|_| "Cell x is out of range")?;
            let y = i32::try_from(write.y).map_err(|_| "Cell y is out of range")?;
            let mut cell = self.writable_cell(staged.get(&(x, y)).copied().unwrap_or_else(|| target.char_at(Position::new(x, y))))?;
            if !profile.per_character_colors()
                && (write.foreground.is_some_and(|color| color != cell.attribute.foreground_color())
                    || write.background.is_some_and(|color| color != cell.attribute.background_color()))
            {
                return Err("ATASCII has shared screen colors. Use inverse char_code glyphs or the editor's palette controls.".into());
            }
            if self.buffer.buffer_type == BufferType::Petscii {
                let background = u8::try_from(icy_engine::petscii_background(&self.buffer)).map_err(|_| "PETSCII screen background is outside 0..255")?;
                if write.background.is_some_and(|color| color != AttributeColor::Palette(background)) {
                    return Err("PETSCII has a shared screen background. Change it using the editor's background control.".into());
                }
            }
            if let Some(glyph) = write.glyph {
                cell.ch = self.encode(glyph, cell.attribute.font_page())?;
            }
            if let Some(color) = write.foreground {
                cell.attribute.set_foreground_color(color);
            }
            if let Some(color) = write.background {
                cell.attribute.set_background_color(color);
            }
            staged.insert((x, y), cell);
            changed += 1;
        }
        for ((x, y), cell) in staged {
            self.buffer.layers[layer].set_char(Position::new(x, y), cell);
        }
        Ok(Self::written(changed, clipped))
    }

    fn writable_cell(&self, mut cell: AttributedChar) -> Result<AttributedChar, String> {
        if cell.is_visible() {
            return Ok(cell);
        }
        let page = cell.attribute.font_page();
        if is_retro(&self.buffer) {
            let (fg, bg) = match self.buffer.buffer_type {
                BufferType::Atascii => (7, 0),
                BufferType::Petscii => (
                    icy_engine::petscii_charset(&self.buffer).0.start_colors().0,
                    icy_engine::petscii_background(&self.buffer),
                ),
                _ => (icy_engine::atari_st_text_color(icy_engine::atari_st_resolution(&self.buffer)), 0),
            };
            cell.ch = ' ';
            cell.attribute.attr &= !icy_engine::attribute::INVISIBLE;
            if cell.attribute.foreground_color() == AttributeColor::Transparent {
                cell.attribute
                    .set_foreground_color(AttributeColor::Palette(u8::try_from(fg).map_err(|_| "Native foreground is outside 0..255")?));
            }
            if cell.attribute.background_color() == AttributeColor::Transparent {
                cell.attribute
                    .set_background_color(AttributeColor::Palette(u8::try_from(bg).map_err(|_| "Native background is outside 0..255")?));
            }
        } else {
            cell = AttributedChar::default();
        }
        cell.attribute.set_font_page(page);
        Ok(cell)
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
            let mut glyph_codes = Vec::new();
            let mut font_pages = Vec::new();
            // Runs of equal colors: [start x, length, foreground, background].
            let mut runs: Vec<(i64, i64, Value, Value)> = Vec::new();
            for column in x..right {
                let cell = layer.char_at(Position::new(column as i32, row as i32));
                if is_retro(&self.buffer) {
                    glyph_codes.push(cell.is_visible().then_some(cell.ch as u32));
                    font_pages.push(cell.is_visible().then_some(cell.attribute.font_page()));
                }
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
            let mut data = json!({ "y": row, "text": text, "colors": runs });
            if is_retro(&self.buffer) {
                data["glyph_codes"] = json!(glyph_codes);
                data["font_pages"] = json!(font_pages);
            }
            rows.push(data);
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
        let mut writes = Vec::new();
        for (row, line) in text.split('\n').enumerate() {
            for (column, character) in line.chars().enumerate() {
                writes.push(CellWrite {
                    x: x.checked_add(column as i64).ok_or("Text x coordinate overflow")?,
                    y: y.checked_add(row as i64).ok_or("Text y coordinate overflow")?,
                    glyph: Some(Glyph::Unicode(character)),
                    foreground,
                    background,
                });
            }
        }
        self.write_cells(layer, writes)
    }

    fn fill_rect(&mut self, arguments: &Value) -> Result<String, String> {
        let layer = self.layer_index(arguments, true)?;
        let number = |key: &str| arguments.get(key).and_then(Value::as_i64).ok_or(format!("{key} is required"));
        let (x, y, width, height) = (number("x")?, number("y")?, number("width")?, number("height")?);
        let character = self.character(arguments)?;
        let foreground = self.color(arguments, "fg")?;
        let background = self.color(arguments, "bg")?;
        if width <= 0 || height <= 0 {
            return Err("width and height must be positive".into());
        }
        if character.is_none() && foreground.is_none() && background.is_none() {
            return Err("Set at least one of char, char_code, fg or bg".into());
        }
        if width.saturating_mul(height) > MAX_WRITE_CELLS as i64 {
            return Err(format!("Fill at most {MAX_WRITE_CELLS} cells at a time"));
        }
        let right = x.checked_add(width).ok_or("Rectangle x coordinate overflow")?;
        let bottom = y.checked_add(height).ok_or("Rectangle y coordinate overflow")?;
        let mut writes = Vec::new();
        for row in y..bottom {
            for column in x..right {
                writes.push(CellWrite {
                    x: column,
                    y: row,
                    glyph: character,
                    foreground,
                    background,
                });
            }
        }
        self.write_cells(layer, writes)
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
            let character = self.character(cell).map_err(|error| format!("cells[{index}]: {error}"))?;
            let foreground = self.color(cell, "fg").map_err(|error| format!("cells[{index}]: {error}"))?;
            let background = self.color(cell, "bg").map_err(|error| format!("cells[{index}]: {error}"))?;
            parsed.push(CellWrite {
                x,
                y,
                glyph: character,
                foreground,
                background,
            });
        }
        self.write_cells(layer, parsed)
    }
}

/// Name, description and JSON schema of each drawing tool.
pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let char_code = json!({ "type": "integer", "minimum": 0, "maximum": 255, "description": "Native screen glyph code, not a terminal control byte. Use char OR char_code. Preserves the existing font page." });
    let layer = json!({ "type": "integer", "minimum": 0, "description": "Layer index; defaults to the current layer" });
    let color = |what: &str| json!({ "type": "integer", "minimum": 0, "description": format!("{what} palette index; omit to keep the existing color") });
    let mut specs = vec![
        (
            "icy_canvas_info",
            "Describe the open drawing: size, character encoding, palette, layers, current layer and selection. Call this first.",
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "icy_read_canvas_glyphs",
            "Read the active document's real font bitmaps, not Unicode approximations. Read this before converting pictures to native character art. Read-only; at most 256 glyphs.",
            json!({
                "type": "object",
                "properties": {
                    "font_page": { "type": "integer", "minimum": 0, "maximum": 255, "description": "Font page from icy_canvas_info; defaults to 0" },
                    "start": { "type": "integer", "minimum": 0, "maximum": 255, "description": "First native glyph code; defaults to 0" },
                    "count": { "type": "integer", "minimum": 1, "maximum": 256, "description": "Number of glyphs; defaults to 256. start + count must be <= 256" },
                },
            }),
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
                    "char_code": char_code,
                    "fg": color("Foreground"),
                    "bg": color("Background"),
                },
                "required": ["x", "y", "width", "height"],
            }),
        ),
        (
            "icy_set_cells",
            "Set individual cells. Each cell has x, y and optional char OR native char_code, fg and bg. Retro screen-wide colors cannot be changed per cell.",
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
                                "char_code": char_code,
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
    ];
    specs.extend(raster::tool_specs());
    specs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atascii_font_catalog_matches_actual_bitmaps_in_both_modes() {
        for mode in icy_draw::screen_profile::AtasciiMode::ALL {
            let mut draft = Draft::new("ATASCII", icy_draw::screen_profile::atascii_buffer(mode), 0, None);
            let info = draft.info();
            let font = draft.buffer.font_for_render(0).unwrap().clone();
            assert_eq!(info["fonts"][0]["name"], font.name());
            assert_eq!(info["fonts"][0]["glyph_width"], font.width);
            assert_eq!(info["fonts"][0]["glyph_height"], font.height);
            assert_eq!(info["character_profile"]["per_character_colors"], false);
            assert_eq!(info["tool_capabilities"]["change_font"], false);
            assert!(info["character_profile"]["image_conversion"].as_str().unwrap().contains("omit fg/bg"));
            let catalog = draft.call("icy_read_canvas_glyphs", &json!({})).unwrap();
            assert!(catalog.len() <= 64 * 1024);
            let catalog: Value = serde_json::from_str(&catalog).unwrap();
            let glyphs = catalog["glyphs"].as_array().unwrap();
            assert_eq!(glyphs.len(), 256);
            for (code, glyph) in glyphs.iter().enumerate() {
                assert_eq!(glyph["char_code"], code);
                let rows = glyph["rows"].as_array().unwrap();
                assert_eq!(rows.len(), usize::from(font.height));
                for (y, row) in rows.iter().enumerate() {
                    let bits = u8::from_str_radix(row.as_str().unwrap(), 16).unwrap();
                    for x in 0..usize::from(font.width) {
                        assert_eq!(bits & (0x80 >> x) != 0, font.glyphs[code].get_pixel(x, y));
                    }
                }
            }
            assert!(draft.changes().is_empty(), "reading fonts never edits the draft");
        }
    }

    #[test]
    fn canvas_glyph_reads_use_document_fonts_and_validate_ranges() {
        let mut draft = draft();
        let font = icy_engine::BitFont::create_8("Custom", 3, 2, &[0xA0, 0x40]);
        draft.buffer.set_font(7, font);
        let read: Value = serde_json::from_str(&draft.call("icy_read_canvas_glyphs", &json!({"font_page": 7, "start": 0, "count": 1})).unwrap()).unwrap();
        assert_eq!(read["name"], "Custom");
        assert_eq!(read["glyphs"][0]["rows"], json!(["A0", "40"]));
        for arguments in [
            json!({"count": 0}),
            json!({"count": 257}),
            json!({"start": 255, "count": 2}),
            json!({"start": -1}),
            json!({"start": 1.5}),
            json!({"font_page": 256}),
            json!({"font_page": "0"}),
            json!({"count": null}),
            json!({"font_page": 9}),
        ] {
            assert!(draft.call("icy_read_canvas_glyphs", &arguments).is_err(), "{arguments}");
        }
        assert!(draft.changes().is_empty());
    }

    #[test]
    fn petscii_text_uses_the_current_set_and_existing_vdc_font_pages() {
        use icy_engine::{PetsciiMachine, Size};
        for (case, expected) in [(PetsciiCase::Upper, 1), (PetsciiCase::Lower, 65)] {
            let buffer = icy_engine::petscii_buffer(PetsciiMachine::C64, case, Size::new(40, 25), 14, 6);
            let mut draft = Draft::new("PETSCII", buffer, 0, None);
            draft.call("icy_draw_text", &json!({"x": 0, "y": 0, "text": "A"})).unwrap();
            assert_eq!(draft.buffer.layers[0].char_at(Position::new(0, 0)).ch as u32, expected);
            assert_eq!(draft.buffer.background_color, Some(6));
        }
        let mut buffer = icy_engine::petscii_buffer(PetsciiMachine::C128Vdc, PetsciiCase::Upper, Size::new(80, 25), 14, 0);
        let mut cell = buffer.layers[0].char_at(Position::new(1, 0));
        cell.attribute.set_font_page(1);
        cell.attribute.attr |= icy_engine::attribute::INVISIBLE;
        buffer.layers[0].set_char(Position::new(1, 0), cell);
        let mut draft = Draft::new("PETSCII", buffer, 0, None);
        draft.call("icy_draw_text", &json!({"x": 0, "y": 0, "text": "AA"})).unwrap();
        let read: Value = serde_json::from_str(&draft.call("icy_read_region", &json!({"width": 2, "height": 1})).unwrap()).unwrap();
        assert_eq!(read["rows"][0]["glyph_codes"], json!([1, 65]));
        assert_eq!(read["rows"][0]["font_pages"], json!([0, 1]));
    }

    #[test]
    fn retro_native_codes_preserve_inverse_glyphs_and_reject_unmappable_text_atomically() {
        for buffer in [
            icy_draw::screen_profile::atascii_buffer(icy_draw::screen_profile::AtasciiMode::Antic),
            icy_draw::screen_profile::atari_st_buffer(icy_engine::TerminalResolution::Medium),
            icy_draw::screen_profile::petscii_buffer(icy_engine::PetsciiMachine::C64, PetsciiCase::Upper),
        ] {
            let mut draft = Draft::new("retro", buffer, 0, None);
            assert!(draft.call("icy_draw_text", &json!({"x": 0, "y": 0, "text": "A😀"})).is_err());
            assert!(draft.changes().is_empty());
            draft.call("icy_set_cells", &json!({"cells": [{"x": 0, "y": 0, "char_code": 193}]})).unwrap();
            assert_eq!(draft.buffer.layers[0].char_at(Position::new(0, 0)).ch as u32, 193);
            let read: Value = serde_json::from_str(&draft.call("icy_read_region", &json!({"width": 1, "height": 1})).unwrap()).unwrap();
            assert_eq!(read["rows"][0]["glyph_codes"], json!([193]));
            let before = draft.changes();
            assert!(draft
                .call("icy_fill_rect", &json!({"x": 0, "y": 0, "width": 1, "height": 1, "char_code": 256}))
                .is_err());
            assert_eq!(draft.changes(), before);
        }
    }

    #[test]
    fn shared_retro_colors_and_machine_text_color_limits_are_enforced() {
        let buffer = icy_draw::screen_profile::atascii_buffer(icy_draw::screen_profile::AtasciiMode::Antic);
        let mut draft = Draft::new("ATASCII", buffer, 0, None);
        let current = draft.buffer.layers[0].char_at(Position::new(0, 0)).attribute.foreground();
        let different = if current == 0 { 1 } else { 0 };
        assert!(draft
            .call(
                "icy_set_cells",
                &json!({"cells": [
                    {"x": 0, "y": 0, "char_code": 193}, {"x": 1, "y": 0, "fg": different}
                ]})
            )
            .unwrap_err()
            .contains("shared"));
        assert!(draft.changes().is_empty());
        let buffer = icy_draw::screen_profile::petscii_buffer(icy_engine::PetsciiMachine::Vic20, PetsciiCase::Upper);
        let mut draft = Draft::new("PETSCII", buffer, 0, None);
        assert!(draft.call("icy_draw_text", &json!({"x": 0, "y": 0, "text": "A", "fg": 8})).is_err());
        let background = icy_engine::petscii_background(&draft.buffer);
        assert!(draft
            .call("icy_fill_rect", &json!({"x": 0, "y": 0, "width": 1, "height": 1, "bg": (background + 1) % 8}))
            .unwrap_err()
            .contains("shared"));
        assert!(draft.changes().is_empty());
    }

    #[test]
    fn native_code_validation_and_duplicate_cell_updates_preserve_existing_behavior() {
        let mut draft = draft();
        draft
            .call(
                "icy_set_cells",
                &json!({"cells": [
                    {"x": 0, "y": 0, "char_code": 219}, {"x": 0, "y": 0, "fg": 3}
                ]}),
            )
            .unwrap();
        let cell = draft.buffer.layers[0].char_at(Position::new(0, 0));
        assert_eq!(cell.ch as u32, 219);
        assert_eq!(cell.attribute.foreground(), 3);
        let before = draft.changes();
        for arguments in [
            json!({"char": "A", "char_code": 1}),
            json!({"char_code": -1}),
            json!({"char_code": 1.5}),
            json!({"char_code": "65"}),
        ] {
            let mut cell = arguments;
            cell["x"] = json!(0);
            cell["y"] = json!(0);
            assert!(draft.call("icy_set_cells", &json!({"cells": [cell]})).is_err());
            assert_eq!(draft.changes(), before);
        }
    }

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
