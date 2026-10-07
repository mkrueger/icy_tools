//! Lossless IGS drafts with bounded, static-only rendering.

use icy_draw::igs_document::{IgsDocument, IgsPreview};
use icy_parser_core::{encode_igs_stream_checked, IgsCommand, IgsItem, IgsParameter, LineMarkerStyle, PatternType, TerminalResolution};
use serde_json::{json, Value};

const MAX_ITEMS: usize = 1024;
const MAX_BYTES: usize = 128 * 1024;
pub const API: &str = include_str!("../../../data/ai/references/igs.md");

#[derive(Clone)]
pub struct IgsDraft {
    pub original: Vec<IgsItem>,
    pub items: Vec<IgsItem>,
    revision: u64,
    pub preview_enabled: bool,
    preview_calls: usize,
}

fn index(args: &Value, key: &str, default: Option<usize>) -> Result<usize, String> {
    match args.get(key) {
        None => default.ok_or_else(|| format!("{key} is required")),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("{key} must be a nonnegative integer")),
    }
}

fn parameters(values: &[&IgsParameter], max: i32) -> bool {
    values.iter().all(|value| matches!(value, IgsParameter::Value(n) if (0..=max).contains(n)))
}

/// Only these commands reach the static renderer. Everything else is preserved but not executed.
fn static_command(command: &IgsCommand) -> bool {
    use IgsCommand::*;
    match command {
        Box { x1, y1, x2, y2, .. } | Line { x1, y1, x2, y2 } | RoundedRectangles { x1, y1, x2, y2, .. } | FilledRectangle { x1, y1, x2, y2 } => {
            parameters(&[x1, y1, x2, y2], 10000)
        }
        LineDrawTo { x, y } | FloodFill { x, y } | PolymarkerPlot { x, y } | SetDrawtoBegin { x, y } => parameters(&[x, y], 10000),
        Circle { x, y, radius } => parameters(&[x, y], 10000) && parameters(&[radius], 640),
        Ellipse { x, y, x_radius, y_radius } => parameters(&[x, y], 10000) && parameters(&[x_radius, y_radius], 640),
        Arc {
            x,
            y,
            radius,
            start_angle,
            end_angle,
        }
        | PieSlice {
            x,
            y,
            radius,
            start_angle,
            end_angle,
        } => parameters(&[x, y], 10000) && parameters(&[radius], 640) && parameters(&[start_angle, end_angle], 360),
        EllipticalArc {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        }
        | EllipticalPieSlice {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        } => parameters(&[x, y], 10000) && parameters(&[x_radius, y_radius], 640) && parameters(&[start_angle, end_angle], 360),
        PolyLine { points } | PolyFill { points } => {
            (4..=256).contains(&points.len()) && points.len() % 2 == 0 && points.iter().all(|p| parameters(&[p], 10000))
        }
        ColorSet { color, .. } | SetTextColor { color, .. } => *color <= 15,
        SetPenColor { pen, red, green, blue } => *pen <= 15 && [red, green, blue].iter().all(|v| **v <= 7),
        AttributeForFills { pattern_type, .. } => match pattern_type {
            PatternType::Pattern(n) => (1..=24).contains(n),
            PatternType::Hatch(n) => (1..=12).contains(n),
            PatternType::UserDefined(n) => *n <= 7,
            _ => true,
        },
        SetLineOrMarkerStyle { style } => match style {
            LineMarkerStyle::PolyMarkerSize(_, n) => (1..=8).contains(n),
            LineMarkerStyle::LineThickness(_, n) => (1..=41).contains(n),
            LineMarkerStyle::LineEndpoints(..) => true,
        },
        WriteText { x, y, text } => parameters(&[x, y], 10000) && text.len() <= 1024 && text.iter().all(|b| (32..=126).contains(b) && *b != b'@'),
        TextEffects { size, .. } => (1..=48).contains(size),
        LoadFillPattern { pattern, data } => *pattern <= 7 && data.len() == 16,
        PositionCursor { x, y } => parameters(&[x, y], 255),
        CursorMotion { count, .. } => (0..=80).contains(count),
        DeleteLine { count } | InsertLine { count, .. } => *count <= 50,
        ClearLine { mode } => *mode <= 2,
        RememberCursor { value } => *value <= 1,
        DrawingMode { .. }
        | HollowSet { .. }
        | GraphicScaling { .. }
        | Initialize { .. }
        | ScreenClear { .. }
        | SetResolution { .. }
        | Cursor { .. }
        | InverseVideo { .. }
        | LineWrap { .. } => true,
        _ => false,
    }
}

fn static_text(text: &icy_parser_core::IgsText) -> bool {
    if text.invalid || text.bytes.len() > 4096 {
        return false;
    }
    let mut i = 0;
    while i < text.bytes.len() {
        let byte = text.bytes[i];
        i += 1;
        if byte == 27 {
            let Some(&code) = text.bytes.get(i) else {
                return false;
            };
            i += 1;
            let extra = match code {
                b'Y' => 2,
                b'b' | b'c' => 1,
                b'A' | b'B' | b'C' | b'D' | b'E' | b'H' | b'I' | b'J' | b'K' | b'L' | b'M' | b'e' | b'f' | b'j' | b'k' | b'l' | b'o' | b'p' | b'q' | b'v'
                | b'w' => 0,
                _ => return false,
            };
            if i + extra > text.bytes.len() || text.bytes[i..i + extra].iter().any(|b| !(32..=127).contains(b)) {
                return false;
            }
            i += extra;
        } else if !(32..=255).contains(&byte) && !matches!(byte, 8 | 9 | 10 | 13) {
            return false;
        }
    }
    true
}

fn editable(item: &IgsItem) -> bool {
    match item {
        IgsItem::Command(item) => static_command(item.command()),
        IgsItem::Text(text) => static_text(text),
    }
}

impl IgsDraft {
    pub fn new(document: &IgsDocument) -> Self {
        Self {
            original: document.items().to_vec(),
            items: document.items().to_vec(),
            revision: document.revision(),
            preview_enabled: true,
            preview_calls: 0,
        }
    }
    pub fn begin_turn(&mut self, preview_enabled: bool) {
        self.preview_enabled = preview_enabled;
        self.preview_calls = 0;
    }
    pub fn changed(&self) -> bool {
        self.items != self.original
    }
    pub fn changes(&self) -> usize {
        let prefix = self.original.iter().zip(&self.items).take_while(|(a, b)| a == b).count();
        let suffix = self.original[prefix..]
            .iter()
            .rev()
            .zip(self.items[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        self.original.len().max(self.items.len()) - prefix - suffix
    }
    pub fn matches(&self, document: &IgsDocument) -> bool {
        document.revision() == self.revision && document.items() == self.original
    }
    pub fn omitted(&self) -> usize {
        self.items.iter().filter(|item| !editable(item)).count()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.items.len() > MAX_ITEMS {
            return Err("IGS draft exceeds 1024 items".into());
        }
        let protected: Vec<_> = self.original.iter().filter(|item| !editable(item)).collect();
        let remaining: Vec<_> = self.items.iter().filter(|item| !editable(item)).collect();
        if protected != remaining {
            return Err("Runtime, unsafe or malformed IGS items are read-only and must remain unchanged and ordered".into());
        }
        let bytes = encode_igs_stream_checked(&self.items).map_err(|e| format!("IGS round-trip validation failed: {e}"))?;
        if bytes.len() > MAX_BYTES {
            return Err("IGS draft exceeds 128 KiB".into());
        }
        Ok(())
    }
    pub fn preview(&self) -> Result<IgsPreview, String> {
        self.validate()?;
        let items: Vec<_> = self.items.iter().filter(|item| editable(item)).cloned().collect();
        IgsDocument::render(&items).map_err(|e| format!("Cannot render static IGS preview: {e}"))
    }
    pub fn preview_image(&mut self) -> Result<super::image_attachment::ReferenceImage, String> {
        if !self.preview_enabled {
            return Err("Select an image-capable model for IGS preview feedback".into());
        }
        if self.preview_calls >= 3 {
            return Err("IGS preview limit reached (3 per turn)".into());
        }
        let preview = self.preview()?;
        let image = image::RgbaImage::from_raw(preview.width() as u32, preview.height() as u32, preview.rgba()).ok_or("Invalid IGS preview dimensions")?;
        let mut bytes = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, image::ImageFormat::Png)
            .map_err(|e| format!("Cannot encode IGS preview: {e}"))?;
        let result = super::image_attachment::prepare("igs-static-preview.png", &bytes.into_inner())?;
        self.preview_calls += 1;
        Ok(result)
    }
    pub fn call(&mut self, tool: &str, args: &Value) -> Result<String, String> {
        match tool {
            "icy_igs_info" => {
                let mut state = icy_draw::igs_document::IgsDrawState::default();
                for command in self.items.iter().filter_map(IgsItem::command) {
                    state.apply(command);
                }
                let (w, h, pens) = match state.resolution {
                    TerminalResolution::Low => (320, 200, 16),
                    TerminalResolution::Medium => (640, 200, 4),
                    TerminalResolution::High => (640, 400, 2),
                };
                Ok(json!({"editor":"IGS", "resolution":format!("{:?}",state.resolution), "width":w, "height":h, "pens":pens,
                    "items":self.items.len(), "omitted_static_items":self.omitted(), "state":format!("{state:?}"),
                    "limits":{"items":MAX_ITEMS,"source_bytes":MAX_BYTES,"read_items":64,"previews_per_turn":3},
                    "notes":"Ordered IGS graphics/state and mixed VT52 text, not ANSI cells. Runtime/unsafe/malformed items are read-only. Static previews OMIT them, including loops, timing, sound and input; actual playback can differ. Read icy_igs_api and item pages before editing."}).to_string())
            }
            "icy_igs_api" => Ok(API.into()),
            "icy_read_igs_items" => {
                let start = index(args, "start", Some(0))?;
                let count = index(args, "count", Some(32))?;
                if count == 0 || count > 64 || start > self.items.len() {
                    return Err("Read 1..64 items from a valid zero-based start".into());
                }
                let end = start.saturating_add(count).min(self.items.len());
                let mut items = Vec::new();
                for (offset, item) in self.items[start..end].iter().enumerate() {
                    let bytes = match item {
                        IgsItem::Text(text) => text.bytes.clone(),
                        IgsItem::Command(item) => {
                            let mut bytes = item.source().map(<[u8]>::to_vec).unwrap_or_else(|| item.command().to_string().into_bytes());
                            bytes.extend_from_slice(item.trailing());
                            bytes
                        }
                    };
                    items.push(json!({"index":start+offset,"editable":editable(item),"source_hex":hex(&bytes),
                        "canonical_source":item.command().map(ToString::to_string),"semantic":format!("{item:?}")}));
                }
                let result = json!({"total":self.items.len(),"start":start,"next":end,"items":items}).to_string();
                if result.len() > 64 * 1024 {
                    return Err("IGS read exceeds 64 KiB; read fewer items".into());
                }
                Ok(result)
            }
            "icy_replace_igs_items" => self.replace(args),
            _ => Err(format!("Unknown IGS tool {tool}")),
        }
    }
    fn replace(&mut self, args: &Value) -> Result<String, String> {
        let start = index(args, "start", None)?;
        let count = index(args, "delete_count", None)?;
        let end = start.checked_add(count).ok_or("IGS range overflow")?;
        if start > self.items.len() || end > self.items.len() {
            return Err("IGS replacement range is outside the draft".into());
        }
        if self.items[start..end].iter().any(|item| !editable(item)) {
            return Err("Selected IGS range contains read-only runtime/unsafe/malformed items".into());
        }
        let bytes = match (args.get("source"), args.get("source_hex")) {
            (Some(source), None) => {
                let source = source.as_str().ok_or("source must be a string")?;
                if !source.is_ascii() {
                    return Err("Use ASCII source or source_hex for native byte text".into());
                }
                if source.len() > MAX_BYTES {
                    return Err("IGS replacement exceeds 128 KiB".into());
                }
                source.as_bytes().to_vec()
            }
            (None, Some(source)) => unhex(source.as_str().ok_or("source_hex must be a string")?)?,
            _ => return Err("Specify exactly one of source or source_hex".into()),
        };
        let parsed = if bytes.is_empty() {
            Vec::new()
        } else {
            IgsDocument::parse_source(&bytes).map_err(|e| format!("Invalid IGS source: {e}"))?
        };
        if parsed.iter().any(|item| !editable(item)) {
            return Err(
                "New items must be bounded static graphics/state or safe VT52 text; loops, timing/audio, host/input and file operations are unavailable".into(),
            );
        }
        let mut state = icy_draw::igs_document::IgsDrawState::default();
        for command in self.items[..start].iter().filter_map(IgsItem::command) {
            state.apply(command);
        }
        for command in parsed.iter().filter_map(IgsItem::command) {
            let pen = match command {
                IgsCommand::ColorSet { color, .. } | IgsCommand::SetTextColor { color, .. } => Some(*color),
                IgsCommand::SetPenColor { pen, .. } => Some(*pen),
                _ => None,
            };
            if pen.is_some_and(|pen| u32::from(pen) >= state.resolution.max_colors()) {
                return Err(format!(
                    "Pen is outside the active {:?} resolution (0..{})",
                    state.resolution,
                    state.resolution.max_colors() - 1
                ));
            }
            state.apply(command);
        }
        let inserted = parsed.len();
        let mut candidate = self.clone();
        candidate.items.splice(start..end, parsed);
        candidate.validate()?;
        self.items = candidate.items;
        Ok(format!(
            "Replaced {count} items with {inserted} in the IGS draft. Inspect the static preview before Apply."
        ))
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}
pub(super) fn unhex(source: &str) -> Result<Vec<u8>, String> {
    if source.len() > MAX_BYTES * 2 || !source.is_ascii() || source.len() % 2 != 0 {
        return Err("source_hex must contain at most 128 KiB of complete byte pairs".into());
    }
    source
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let hi = (pair[0] as char).to_digit(16).ok_or("Invalid source_hex digit")?;
            let lo = (pair[1] as char).to_digit(16).ok_or("Invalid source_hex digit")?;
            Ok((hi * 16 + lo) as u8)
        })
        .collect()
}

pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("icy_igs_info","Describe the IGS draft, resolution, pen count, ordered state, protected items and static-preview limits. Call first.",json!({"type":"object","properties":{}})),
        ("icy_igs_api","Read implementation-backed IGS authoring syntax, drawing/state tips and static-preview safeguards.",json!({"type":"object","properties":{}})),
        ("icy_read_igs_items","Read up to 64 ordered lossless IGS items, native source hex, canonical command syntax and editability.",json!({"type":"object","properties":{"start":{"type":"integer","minimum":0},"count":{"type":"integer","minimum":1,"maximum":64}}})),
        ("icy_replace_igs_items","Atomically replace/delete/insert ordered IGS items in the draft. start/delete_count are zero-based. Provide ASCII source OR exact source_hex. Read-only runtime items cannot be replaced. Empty source deletes; delete_count=0 inserts. Nothing is applied automatically.",json!({"type":"object","properties":{"start":{"type":"integer","minimum":0},"delete_count":{"type":"integer","minimum":0},"source":{"type":"string"},"source_hex":{"type":"string"}},"required":["start","delete_count"]})),
        ("icy_preview_igs","Return a rendered PNG of the STATIC IGS draft. Runtime/unsafe/malformed items including loops, timing/audio and input are omitted, never executed. At most 3 previews per turn; requires image-capable model.",json!({"type":"object","properties":{}})),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> IgsDraft {
        IgsDraft::new(&IgsDocument::new(TerminalResolution::Low))
    }
    fn append(draft: &mut IgsDraft, source: &str) -> Result<String, String> {
        draft.call("icy_replace_igs_items", &json!({"start":draft.items.len(),"delete_count":0,"source":source}))
    }

    #[test]
    fn static_graphics_render_in_all_resolutions_and_tool_reads_are_paged() {
        for (resolution, w, h, pens) in [
            (TerminalResolution::Low, 320, 200, 16),
            (TerminalResolution::Medium, 640, 200, 4),
            (TerminalResolution::High, 640, 400, 2),
        ] {
            let mut draft = IgsDraft::new(&IgsDocument::new(resolution));
            let before = draft.preview().unwrap().rgba();
            append(&mut draft, "G#C>1,1:\nG#L>10,10,100,100:\nG#C>3,1:\nG#W>20,40,Hello@\n").unwrap();
            let info: Value = serde_json::from_str(&draft.call("icy_igs_info", &json!({})).unwrap()).unwrap();
            assert_eq!(info["width"], w);
            assert_eq!(info["height"], h);
            assert_eq!(info["pens"], pens);
            let preview = draft.preview().unwrap();
            assert_eq!((preview.width(), preview.height()), (w as usize, h as usize));
            assert_ne!(preview.rgba(), before);
            assert_eq!(preview.rgba(), draft.preview().unwrap().rgba());
            let read: Value = serde_json::from_str(&draft.call("icy_read_igs_items", &json!({"start":2,"count":2})).unwrap()).unwrap();
            assert_eq!(read["next"], 4);
            assert!(read["items"][0]["editable"].as_bool().unwrap());
            assert!(read["items"][0]["canonical_source"].as_str().unwrap().starts_with("G#C"));
            assert!(draft.call("icy_read_igs_items", &json!({"count":65})).is_err());
        }
    }

    #[test]
    fn runtime_items_are_preserved_but_never_rendered_or_replaced() {
        let source = b"G#R>0,0:\nG#q>9999:\nG#b>0:\nG#&>10,50,10,2,L,4,x,20,x,40:\nG#C>1,1:\nG#L>10,10,100,100:\n";
        let document = IgsDocument::from_bytes(source).unwrap();
        let mut draft = IgsDraft::new(&document);
        assert!(draft.omitted() >= 3);
        let protected: Vec<_> = draft.items.iter().filter(|item| !editable(item)).cloned().collect();
        let baseline = draft.preview().unwrap().rgba();
        let static_only: Vec<_> = draft.items.iter().filter(|item| editable(item)).cloned().collect();
        assert_eq!(baseline, IgsDocument::render(&static_only).unwrap().rgba());
        for (i, item) in draft.items.clone().iter().enumerate() {
            if !editable(item) {
                assert!(draft.call("icy_replace_igs_items", &json!({"start":i,"delete_count":1,"source":""})).is_err());
            }
        }
        append(&mut draft, "G#L>20,100,100,20:\n").unwrap();
        assert_eq!(protected, draft.items.iter().filter(|item| !editable(item)).cloned().collect::<Vec<_>>());
        assert!(draft.matches(&document));
        let encoded = encode_igs_stream_checked(&draft.items).unwrap();
        assert!(encoded.windows(b"G#q>9999:".len()).any(|bytes| bytes == b"G#q>9999:"));
        assert!(encoded
            .windows(b"G#&>10,50,10,2,L,4,x,20,x,40:".len())
            .any(|bytes| bytes == b"G#&>10,50,10,2,L,4,x,20,x,40:"));
    }

    #[test]
    fn mixed_vt52_native_bytes_round_trip_and_can_be_edited_explicitly() {
        let document = IgsDocument::from_bytes(b"\x1bE\x1bY!!Hello\x82\nG#L>1,1,20,20:\n").unwrap();
        let mut draft = IgsDraft::new(&document);
        append(&mut draft, "G#C>1,1:\n").unwrap();
        assert!(encode_igs_stream_checked(&draft.items).unwrap().starts_with(b"\x1bE\x1bY!!Hello\x82\n"));
        let mut empty = self::draft();
        empty
            .call(
                "icy_replace_igs_items",
                &json!({"start":2,"delete_count":0,"source_hex":hex(b"\x1bE\x1bY!!Hello\x82\n")}),
            )
            .unwrap();
        assert!(empty.preview().is_ok());
    }

    #[test]
    fn invalid_or_runtime_replacements_fail_atomically() {
        let mut draft = draft();
        for source in [
            "G#q>10:",
            "G#b>0:",
            "G#L>r,0,10,10:",
            "G#O>10,10,9999:",
            "G#W>1,1,unfinished",
            "G#S>0,9,0,0:",
            "G#E>0,99,0:",
            "G#?>1:",
            "\x1b]bad\x07",
        ] {
            let before = draft.items.clone();
            assert!(append(&mut draft, source).is_err(), "{source:?}");
            assert_eq!(draft.items, before);
        }
        for args in [
            json!({"start":-1,"delete_count":0,"source":""}),
            json!({"start":0,"delete_count":999,"source":""}),
            json!({"start":0,"delete_count":0,"source":"é"}),
            json!({"start":0,"delete_count":0,"source_hex":"0"}),
            json!({"start":0,"delete_count":0,"source_hex":"ZZ"}),
            json!({"start":0,"delete_count":0,"source":"","source_hex":""}),
        ] {
            assert!(draft.call("icy_replace_igs_items", &args).is_err());
        }
        assert!(!draft.changed());
    }

    #[test]
    fn insert_delete_replace_stale_revision_and_preview_limits() {
        let mut document = IgsDocument::new(TerminalResolution::Low);
        let mut draft = IgsDraft::new(&document);
        append(&mut draft, "G#C>1,1:\nG#L>1,1,20,20:\n").unwrap();
        assert!(draft.changed() && draft.matches(&document));
        draft
            .call("icy_replace_igs_items", &json!({"start":2,"delete_count":1,"source":"G#C>1,2:\n"}))
            .unwrap();
        draft.call("icy_replace_igs_items", &json!({"start":3,"delete_count":1,"source":""})).unwrap();
        for _ in 0..3 {
            assert!(draft.preview_image().is_ok());
        }
        assert!(draft.preview_image().is_err());
        draft.begin_turn(false);
        assert!(draft.preview_image().is_err());
        draft.begin_turn(true);
        assert!(draft.preview_image().is_ok());
        document.replace_items(draft.items.clone()).unwrap();
        assert!(!draft.matches(&document));
        document.undo();
        assert!(!draft.matches(&document), "undo does not restore revision identity");
    }

    #[test]
    fn item_and_source_limits_are_checked_before_mutation() {
        let mut draft = draft();
        let before = draft.items.clone();
        assert!(append(&mut draft, &"G#L>1,1,2,2:\n".repeat(MAX_ITEMS + 1)).is_err());
        assert!(append(&mut draft, &" ".repeat(MAX_BYTES + 1)).is_err());
        assert_eq!(draft.items, before);
    }

    #[test]
    fn pen_writes_follow_resolution_and_malformed_bytes_remain_read_only() {
        let mut medium = IgsDraft::new(&IgsDocument::new(TerminalResolution::Medium));
        assert!(append(&mut medium, "G#C>1,4:\n").unwrap_err().contains("resolution"));
        assert!(append(&mut medium, "G#S>4,7,0,0:\n").is_err());
        append(&mut medium, "G#R>0,2:\nG#C>1,15:\n").unwrap();
        let document = IgsDocument::from_bytes(b"G#R>0,0:\nG#L>1,1,20,20:\nG#W>1,1,unfinished").unwrap();
        let mut draft = IgsDraft::new(&document);
        assert!(draft.omitted() > 0);
        let bytes = encode_igs_stream_checked(&draft.items).unwrap();
        assert_eq!(bytes, b"G#R>0,0:\nG#L>1,1,20,20:\nG#W>1,1,unfinished");
        draft
            .call("icy_replace_igs_items", &json!({"start":1,"delete_count":1,"source":"G#L>2,2,25,25:\n"}))
            .unwrap();
        assert!(encode_igs_stream_checked(&draft.items).unwrap().ends_with(b"G#W>1,1,unfinished"));
        assert!(draft.preview().is_ok(), "malformed original fragments are omitted rather than executed");
    }
}
