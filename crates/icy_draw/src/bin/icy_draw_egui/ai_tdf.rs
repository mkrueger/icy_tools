//! Draft-only, whole-glyph tools for the selected TheDraw text-art font.

use icy_draw::{charfont::CharFontDocument, document::Document};
use icy_engine::{char_set::TdfBufferRenderer, AttributedChar, Position, TextBuffer};
use retrofont::{tdf::TdfFont, Glyph, GlyphPart, RenderOptions};
use serde_json::{json, Value};

pub const GUIDANCE: &str = include_str!("../../../data/ai/references/tdf.md");
const FIRST: u32 = 33;
const LAST: u32 = 126;
pub const PAGE_SIZE: usize = 32;

#[derive(Clone)]
pub struct TdfDraft {
    pub font: TdfFont,
    original: TdfFont,
    pub font_index: usize,
    pub selected: Option<char>,
    preview_enabled: bool,
    preview_calls: usize,
}

/// Include unsaved edits to the currently selected glyph without committing the live document.
pub fn snapshot(editor: &CharFontDocument, document: &Document) -> Option<TdfFont> {
    let mut font = editor.state.selected_font()?.clone();
    if document.modified() {
        if let Some(ch) = editor.state.selected_char() {
            match document.with_state(|state| icy_draw::charfont::buffer_to_glyph(state.get_buffer(), font.font_type)) {
                Some(glyph) => font.add_glyph(ch, glyph),
                None => {
                    font.remove_glyph(ch);
                }
            }
        }
    }
    Some(font)
}

impl TdfDraft {
    pub fn new(font: TdfFont, font_index: usize, selected: Option<char>) -> Self {
        Self {
            original: font.clone(),
            font,
            font_index,
            selected,
            preview_enabled: false,
            preview_calls: 0,
        }
    }

    pub fn begin_turn(&mut self, preview_enabled: bool) {
        self.preview_enabled = preview_enabled;
        self.preview_calls = 0;
    }

    pub fn preview_feedback(&mut self, args: &Value) -> Result<TextBuffer, String> {
        if !self.preview_enabled {
            return Err("Choose an image-capable model for TDF preview feedback".into());
        }
        if self.preview_calls >= 3 {
            return Err("TDF preview limit reached (3 per turn); continue in another turn".into());
        }
        let buffer = self.preview_buffer(args)?;
        self.preview_calls += 1;
        Ok(buffer)
    }

    pub fn changed_codes(&self) -> Vec<char> {
        ('!'..='~').filter(|&ch| !same_glyph(self.original.glyph(ch), self.font.glyph(ch))).collect()
    }

    pub fn matches(&self, editor: &CharFontDocument, document: &Document) -> Result<bool, String> {
        if editor.state.selected_font_index() != self.font_index {
            return Ok(false);
        }
        let Some(current) = snapshot(editor, document) else {
            return Ok(false);
        };
        let bytes = |font: &TdfFont| TdfFont::serialize_bundle(std::slice::from_ref(font)).map_err(|error| error.to_string());
        Ok(bytes(&current)? == bytes(&self.original)?)
    }

    pub fn info(&self) -> Value {
        json!({
            "name": self.font.name, "font_index": self.font_index, "font_type": format!("{:?}", self.font.font_type),
            "spacing": self.font.spacing, "selected_char": self.selected,
            "first_code": FIRST, "last_code": LAST, "glyph_slots": 94, "max_width": 30, "max_height": 12,
            "defined_codes": ('!'..='~').filter(|&ch| self.font.glyph(ch).is_some()).map(|ch| ch as u32).collect::<Vec<_>>(),
            "missing_codes": ('!'..='~').filter(|&ch| self.font.glyph(ch).is_none()).map(|ch| ch as u32).collect::<Vec<_>>(),
            "changed_codes": self.changed_codes().iter().map(|&ch| ch as u32).collect::<Vec<_>>(),
            "outline_styles": retrofont::OUTLINE_CHAR_SET_UNICODE.iter().enumerate().map(|(style, chars)| {
                json!({"style": style, "codes": "ABCDEFGHIJKLMNOPQ", "characters": chars.iter().collect::<String>()})
            }).collect::<Vec<_>>(),
            "note": "Read-only metadata. TDF reference explains space, ~, 0xFF and outline markers. All writes are draft-only."
        })
    }

    pub fn call(&mut self, tool: &str, args: &Value) -> Result<String, String> {
        let value = match tool {
            "icy_tdf_info" => self.info(),
            "icy_read_tdf_glyphs" => {
                let (start, count) = range(args, 16)?;
                let mut glyphs = Vec::new();
                for code in start..start + count {
                    let ch = char::from_u32(code).ok_or("Invalid character code")?;
                    let glyph = self.font.glyph(ch).map(glyph_data).transpose()?;
                    glyphs.push(json!({"code": code, "glyph": glyph}));
                }
                json!({"glyphs": glyphs})
            }
            "icy_write_tdf_glyphs" => {
                if args.to_string().len() > 128 * 1024 {
                    return Err("TDF write exceeds 128 KiB; split into smaller batches".into());
                }
                let glyphs = args["glyphs"].as_array().ok_or("glyphs must be an array")?;
                if glyphs.is_empty() || glyphs.len() > 94 {
                    return Err("Write between 1 and 94 glyphs per call".into());
                }
                let mut candidate = self.font.clone();
                let mut seen = std::collections::HashSet::new();
                for entry in glyphs {
                    let code = entry["code"]
                        .as_u64()
                        .filter(|code| (33..=126).contains(code))
                        .ok_or("glyph code must be 33..126")?;
                    if !seen.insert(code) {
                        return Err(format!("Duplicate glyph code {code}"));
                    }
                    let ch = char::from_u32(code as u32).ok_or("Invalid glyph code")?;
                    if entry.get("rows") == Some(&Value::Null) {
                        candidate.remove_glyph(ch);
                        continue;
                    }
                    let rows = entry["rows"].as_array().ok_or("rows must be an array of cell arrays, or null to delete")?;
                    if rows.is_empty() || rows.len() > 12 {
                        return Err("Glyph height must be 1..12".into());
                    }
                    let width = rows[0].as_array().ok_or("Each row must be a cell array")?.len();
                    if !(1..=30).contains(&width) {
                        return Err("Glyph width must be 1..30".into());
                    }
                    let mut parts = Vec::new();
                    for (y, row) in rows.iter().enumerate() {
                        let cells = row
                            .as_array()
                            .filter(|row| row.len() == width)
                            .ok_or("All glyph rows must have the same width")?;
                        if y > 0 {
                            parts.push(GlyphPart::NewLine);
                        }
                        for cell in cells {
                            parts.push(cell_part(cell, candidate.font_type)?);
                        }
                    }
                    candidate.add_glyph(
                        ch,
                        Glyph {
                            width,
                            height: rows.len(),
                            parts,
                        },
                    );
                }
                TdfFont::serialize_bundle(std::slice::from_ref(&candidate)).map_err(|error| format!("Cannot serialize TDF draft: {error}"))?;
                self.font = candidate;
                json!({"written": glyphs.len(), "changed_glyphs": self.changed_codes().len(), "applied": false})
            }
            _ => return Err(format!("Unknown TDF tool: {tool}")),
        };
        let result = serde_json::to_string(&value).map_err(|error| format!("Cannot encode TDF response: {error}"))?;
        if result.len() > 64 * 1024 {
            return Err("TDF response exceeds 64 KiB; read fewer glyphs".into());
        }
        Ok(result)
    }

    pub fn preview_buffer(&self, args: &Value) -> Result<TextBuffer, String> {
        let (start, count) = range(args, PAGE_SIZE as u32)?;
        let style = args
            .get("outline_style")
            .map_or(Ok(0), |value| value.as_u64().filter(|&style| style < 19).ok_or("outline_style must be 0..18"))?;
        let mut buffer = TextBuffer::new((128, (count.div_ceil(4) * 14) as i32));
        for i in 0..count {
            let ch = char::from_u32(start + i).ok_or("Invalid glyph code")?;
            let (x, y) = ((i % 4 * 32) as i32, (i / 4 * 14) as i32);
            buffer.layers[0].set_char(Position::new(x, y), AttributedChar::new(ch, icy_engine::TextAttribute::from_color(14, 0)));
            if let Some(glyph) = self.font.glyph(ch) {
                if glyph.width > 30 || glyph.height > 12 {
                    return Err(format!("Glyph {ch:?} exceeds the 30x12 preview grid"));
                }
                glyph
                    .render(
                        &mut TdfBufferRenderer::new(&mut buffer, x, y + 1),
                        &RenderOptions {
                            outline_style: style as usize,
                            ..RenderOptions::display()
                        },
                    )
                    .map_err(|error| format!("Cannot render TDF glyph: {error}"))?;
            }
        }
        Ok(buffer)
    }
}

fn same_glyph(left: Option<&Glyph>, right: Option<&Glyph>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.width == right.width && left.height == right.height && left.parts == right.parts,
        (None, None) => true,
        _ => false,
    }
}

fn glyph_data(glyph: &Glyph) -> Result<Value, String> {
    if glyph.width > 30 || glyph.height > 12 {
        return Err("Existing glyph exceeds the supported 30x12 grid".into());
    }
    let mut rows = vec![vec![json!({"char_code": 32}); glyph.width]; glyph.height];
    let mut end_markers = Vec::new();
    let (mut x, mut y) = (0, 0);
    for part in &glyph.parts {
        if *part == GlyphPart::NewLine {
            y += 1;
            x = 0;
            continue;
        }
        if *part == GlyphPart::EndMarker {
            end_markers.push(json!({"x": x, "y": y}));
            continue;
        }
        let cell = match part {
            GlyphPart::NewLine => unreachable!(),
            GlyphPart::Skip => json!({"char_code": 32}),
            GlyphPart::HardBlank => json!({"char_code": 255}),
            GlyphPart::FillMarker => json!({"char_code": 64}),
            GlyphPart::EndMarker => json!({"char_code": 38}),
            GlyphPart::OutlineHole => json!({"char_code": 79}),
            GlyphPart::OutlinePlaceholder(code) => json!({"char_code": code}),
            GlyphPart::Char(ch) | GlyphPart::AnsiChar { ch, .. } => {
                let code = codepages::tables::UNICODE_TO_CP437
                    .get(ch)
                    .ok_or("Existing glyph contains a non-CP437 character")?;
                match part {
                    GlyphPart::AnsiChar { fg, bg, blink, .. } => json!({"char_code": code, "fg": fg, "bg": bg, "blink": blink}),
                    _ => json!({"char_code": code}),
                }
            }
        };
        let target = rows
            .get_mut(y)
            .and_then(|row| row.get_mut(x))
            .ok_or("Existing glyph parts exceed its declared dimensions")?;
        *target = cell;
        x += 1;
    }
    Ok(json!({"width": glyph.width, "height": glyph.height, "rows": rows, "end_markers": end_markers}))
}

fn range(args: &Value, max: u32) -> Result<(u32, u32), String> {
    let integer = |key: &str, default| args.get(key).map_or(Ok(default), |value| value.as_u64().ok_or("start/count must be integers"));
    let start = integer("start", FIRST as u64)?;
    let count = integer("count", max as u64)?;
    if start < FIRST as u64 || count == 0 || count > max as u64 || start.checked_add(count).is_none_or(|end| end > LAST as u64 + 1) {
        return Err(format!("Use start 33..126 and count 1..{max}, ending at or before 126"));
    }
    Ok((start as u32, count as u32))
}

fn cell_part(cell: &Value, kind: retrofont::tdf::TdfFontType) -> Result<GlyphPart, String> {
    use retrofont::tdf::TdfFontType;
    if kind != TdfFontType::Color && (cell.get("fg").is_some() || cell.get("bg").is_some() || cell.get("blink").is_some()) {
        return Err("Block/Outline fonts do not store per-cell colors".into());
    }
    let code = cell["char_code"]
        .as_u64()
        .filter(|&code| (1..=255).contains(&code) && code != 13)
        .ok_or("char_code must be 1..255, excluding newline 13")? as u8;
    if kind == TdfFontType::Color && matches!(code, 32 | 255) && (cell.get("fg").is_some() || cell.get("bg").is_some() || cell.get("blink").is_some()) {
        return Err("Space and hard blank do not store colors; omit fg/bg/blink for these cells".into());
    }
    if code == b' ' {
        return Ok(GlyphPart::Skip);
    }
    if kind == TdfFontType::Outline {
        return match code {
            b'A'..=b'Q' => Ok(GlyphPart::OutlinePlaceholder(code)),
            b'@' => Ok(GlyphPart::FillMarker),
            b'&' => Ok(GlyphPart::EndMarker),
            _ => Err("Outline cells must be space, A..Q, @ or &; use O/@ for occupied blanks".into()),
        };
    }
    if code == b'&' {
        return Err("& is a reserved end marker, not ordinary artwork".into());
    }
    if code == 255 {
        return Ok(GlyphPart::HardBlank);
    }
    let ch = codepages::tables::CP437_TO_UNICODE[code as usize];
    if kind == TdfFontType::Color {
        let color = |key: &str, max| {
            cell[key]
                .as_u64()
                .filter(|&color| color <= max)
                .ok_or_else(|| format!("{key} is required and must be 0..{max}"))
        };
        let blink = cell.get("blink").map_or(Ok(false), |value| value.as_bool().ok_or("blink must be boolean"))?;
        Ok(GlyphPart::AnsiChar {
            ch,
            fg: color("fg", 15)? as u8,
            bg: color("bg", 7)? as u8,
            blink,
        })
    } else {
        Ok(GlyphPart::Char(ch))
    }
}

pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let range = json!({"start": {"type": "integer", "minimum": 33, "maximum": 126}, "count": {"type": "integer", "minimum": 1, "maximum": 16}});
    vec![
        ("icy_tdf_info", "Read selected TheDraw font type, coverage, limits and outline style mappings.", json!({"type":"object","properties":{}})),
        ("icy_read_tdf_glyphs", "Read TDF glyph dimensions and native cell rows in pages of up to 16 glyphs. Default start 33/count 16.", json!({"type":"object","properties":range})),
        ("icy_write_tdf_glyphs", "Atomically replace whole TDF glyphs in the draft, never the live font. Cells use CP437 char_code; Color artwork requires fg 0..15/bg 0..7. Space skips, 255 is a hard blank, ~ is ordinary artwork. Null rows deletes a glyph.", json!({
            "type":"object","properties":{"glyphs":{"type":"array","minItems":1,"maxItems":94,"items":{
                "type":"object","properties":{"code":{"type":"integer","minimum":33,"maximum":126},"rows":{
                    "type":["array","null"],"maxItems":12,"items":{"type":"array","maxItems":30,"items":{
                        "type":"object","properties":{"char_code":{"type":"integer","minimum":1,"maximum":255},"fg":{"type":"integer","minimum":0,"maximum":15},"bg":{"type":"integer","minimum":0,"maximum":7},"blink":{"type":"boolean"}},"required":["char_code"]
                    }}
                }},"required":["code","rows"]
            }}},"required":["glyphs"]
        })),
        ("icy_preview_tdf", "Render a labelled TDF contact-sheet page (up to 32 glyphs; three pages cover the full font). Select outline_style 0..18. User Apply is still required.", json!({
            "type":"object","properties":{"start":{"type":"integer","minimum":33,"maximum":126},"count":{"type":"integer","minimum":1,"maximum":32},"outline_style":{"type":"integer","minimum":0,"maximum":18}}
        })),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::TextPane;
    use retrofont::tdf::TdfFontType;

    #[test]
    fn full_sets_cover_all_94_slots_and_roundtrip_for_each_type() {
        for kind in [TdfFontType::Color, TdfFontType::Block, TdfFontType::Outline] {
            let mut draft = TdfDraft::new(TdfFont::new("Test", kind, 1), 0, Some('A'));
            let cell = match kind {
                TdfFontType::Color => json!({"char_code": 219, "fg": 15, "bg": 1}),
                TdfFontType::Block => json!({"char_code": 219}),
                TdfFontType::Outline => json!({"char_code": 65}),
            };
            let glyphs: Vec<_> = (33..=126).map(|code| json!({"code": code, "rows": [[cell.clone()]]})).collect();
            draft.call("icy_write_tdf_glyphs", &json!({"glyphs": glyphs})).unwrap();
            assert_eq!(draft.changed_codes().len(), 94);
            assert_eq!(draft.info()["missing_codes"], json!([]));
            let bytes = TdfFont::serialize_bundle(std::slice::from_ref(&draft.font)).unwrap();
            let loaded = TdfFont::load(&bytes).unwrap().remove(0);
            assert_eq!(loaded.glyph_count(), 94);
            for ch in '!'..='~' {
                assert!(same_glyph(draft.font.glyph(ch), loaded.glyph(ch)), "{kind:?} {ch}");
            }
            let read: Value = serde_json::from_str(&draft.call("icy_read_tdf_glyphs", &json!({"start": 126, "count": 1})).unwrap()).unwrap();
            assert_eq!(read["glyphs"][0]["code"], 126);
            assert_eq!(read["glyphs"][0]["glyph"]["rows"][0][0]["char_code"], cell["char_code"]);
        }
    }

    #[test]
    fn blanks_and_outline_styles_have_distinct_rendering_semantics() {
        let mut block = TdfDraft::new(TdfFont::new("Block", TdfFontType::Block, 1), 0, None);
        block
            .call(
                "icy_write_tdf_glyphs",
                &json!({"glyphs": [{
                    "code": 126, "rows": [[{"char_code": 32}, {"char_code": 255}, {"char_code": 126}]]
                }]}),
            )
            .unwrap();
        assert_eq!(
            block.font.glyph('~').unwrap().parts,
            vec![GlyphPart::Skip, GlyphPart::HardBlank, GlyphPart::Char('~')]
        );
        let buffer = block.preview_buffer(&json!({"start": 126, "count": 1})).unwrap();
        assert_eq!(buffer.char_at(Position::new(1, 1)).ch, ' ');
        assert_eq!(buffer.char_at(Position::new(2, 1)).ch, '~');
        let mut outline = TdfDraft::new(TdfFont::new("Outline", TdfFontType::Outline, 1), 0, None);
        outline
            .call(
                "icy_write_tdf_glyphs",
                &json!({"glyphs": [{
                    "code": 65, "rows": [[{"char_code": 65}, {"char_code": 79}, {"char_code": 64}]]
                }]}),
            )
            .unwrap();
        let line = outline.preview_buffer(&json!({"start": 65, "count": 1, "outline_style": 0})).unwrap();
        let block = outline.preview_buffer(&json!({"start": 65, "count": 1, "outline_style": 16})).unwrap();
        assert_ne!(line.char_at(Position::new(0, 1)).ch, block.char_at(Position::new(0, 1)).ch);
        assert_eq!(line.char_at(Position::new(1, 1)).ch, ' ');
        assert_eq!(line.char_at(Position::new(2, 1)).ch, ' ');
    }

    #[test]
    fn invalid_batches_are_atomic_and_preview_feedback_is_bounded() {
        let mut draft = TdfDraft::new(TdfFont::new("Test", TdfFontType::Block, 1), 0, None);
        for invalid in [
            json!({"code": 32, "rows": [[{"char_code": 219}]]}),
            json!({"code": 66, "rows": [[{"char_code": 13}]]}),
            json!({"code": 66, "rows": [[{"char_code": 38}]]}),
            json!({"code": 66, "rows": [[{"char_code": 219, "fg": 8}]]}),
            json!({"code": 66, "rows": [[{"char_code": 219}], []]}),
            json!({"code": 66, "rows": vec![vec![json!({"char_code": 219}); 31]]}),
        ] {
            assert!(draft
                .call(
                    "icy_write_tdf_glyphs",
                    &json!({"glyphs": [{"code": 65, "rows": [[{"char_code": 219}]]}, invalid]})
                )
                .is_err());
            assert!(draft.changed_codes().is_empty());
        }
        assert!(draft.preview_feedback(&json!({})).is_err());
        draft.begin_turn(true);
        for _ in 0..3 {
            draft.preview_feedback(&json!({})).unwrap();
        }
        assert!(draft.preview_feedback(&json!({})).is_err());
        assert!(draft.preview_buffer(&json!({"outline_style": 19})).is_err());
        assert!(draft.call("icy_read_tdf_glyphs", &json!({"start": 126, "count": 2})).is_err());
    }

    #[test]
    fn reading_end_markers_does_not_treat_them_as_extra_artwork_cells() {
        let glyph = Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Char('X'), GlyphPart::EndMarker],
        };
        let data = glyph_data(&glyph).unwrap();
        assert_eq!(data["rows"], json!([[{"char_code": 88}]]));
        assert_eq!(data["end_markers"], json!([{"x": 1, "y": 0}]));
    }
}
