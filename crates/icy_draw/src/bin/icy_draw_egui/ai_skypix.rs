//! Lossless SkyPix drafts with bounded graphics and static-only feedback.

use icy_draw::skypix_document::{SkypixDocument, SkypixItem, SkypixPreview};
use icy_parser_core::{DisplayMode, SkypixCommand};
use serde_json::{json, Value};

const MAX_ITEMS: usize = 1024;
const MAX_BYTES: usize = 128 * 1024;
pub const API: &str = include_str!("../../../data/ai/references/skypix.md");

pub struct SkypixDraft {
    document: SkypixDocument,
    original: Vec<SkypixItem>,
    revision: u64,
    pub preview_enabled: bool,
    preview_calls: usize,
}

impl Clone for SkypixDraft {
    fn clone(&self) -> Self {
        Self {
            document: self.document.draft_copy(),
            original: self.original.clone(),
            revision: self.revision,
            preview_enabled: self.preview_enabled,
            preview_calls: self.preview_calls,
        }
    }
}

fn index(args: &Value, key: &str, default: Option<usize>) -> Result<usize, String> {
    match args.get(key) {
        None => default.ok_or_else(|| format!("{key} is required")),
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| format!("{key} must be a nonnegative integer")),
    }
}

fn safe_ansi(bytes: &[u8]) -> bool {
    if bytes.len() < 3 || !bytes.starts_with(b"\x1b[") {
        return false;
    }
    let Some((&last, params)) = bytes[2..].split_last() else { return false };
    if !params.iter().all(|b| b.is_ascii_digit() || *b == b';') || params.len() > 64 {
        return false;
    }
    let Ok(params) = std::str::from_utf8(params) else { return false };
    let mut values = Vec::new();
    for value in params.split(';') {
        let number = if value.is_empty() {
            0
        } else {
            match value.parse::<u32>() {
                Ok(n) => n,
                Err(_) => return false,
            }
        };
        values.push(number);
    }
    match last {
        b'm' => {
            values.len() <= 16
                && values
                    .iter()
                    .all(|n| matches!(n, 0..=9 | 22..=29 | 30..=37 | 39 | 40..=47 | 49 | 90..=97 | 100..=107))
        }
        b'H' | b'f' => values.len() <= 2 && values.first().is_some_and(|n| *n <= 25) && values.get(1).is_none_or(|n| *n <= 80),
        b'A' | b'B' | b'C' | b'D' | b'E' | b'F' | b'G' => values.len() == 1 && values[0] <= 80,
        b'J' | b'K' => values.len() == 1 && values[0] <= 2,
        b's' | b'u' => params.is_empty(),
        _ => false,
    }
}

fn editable(item: &SkypixItem) -> bool {
    use SkypixCommand::*;
    let point = |x: i32, y: i32| (0..640).contains(&x) && (0..200).contains(&y);
    match item {
        SkypixItem::Text(bytes) => bytes.len() <= 4096 && bytes.iter().all(|b| *b >= 32 && *b != 127 || matches!(b, 8 | 9 | 10 | 12 | 13)),
        SkypixItem::Raw(bytes) => safe_ansi(bytes),
        SkypixItem::Command(command) => match command {
            SetPixel { x, y } | DrawLine { x, y } | MovePen { x, y } | PositionCursor { x, y } | AreaFill { x, y, .. } => point(*x, *y),
            RectangleFill { x1, y1, x2, y2 } => point(*x1, *y1) && point(*x2, *y2),
            Ellipse { x, y, a, b } | FilledEllipse { x, y, a, b } => point(*x, *y) && (0..=640).contains(a) && (0..=200).contains(b),
            SetPenA { color } | SetPenB { color } => (0..16).contains(color),
            NewPalette { colors } => colors.len() == 16 && colors.iter().all(|c| (0..=4095).contains(c)),
            SetFont { size, name } => name.len() <= 64 && (1..=48).contains(size) && icy_engine::get_amiga_font_by_name(name, *size).is_some(),
            Comment { text } => text.len() <= 1024 && text.is_ascii() && text.bytes().all(|b| (32..=126).contains(&b) && b != b'!'),
            GrabBrush { x1, y1, width, height } => {
                point(*x1, *y1) && *width > 0 && *height > 0 && i64::from(*x1) + i64::from(*width) <= 640 && i64::from(*y1) + i64::from(*height) <= 200
            }
            UseBrush {
                src_x,
                src_y,
                dst_x,
                dst_y,
                width,
                height,
                minterm,
                mask,
            } => {
                point(*src_x, *src_y)
                    && point(*dst_x, *dst_y)
                    && *width > 0
                    && *height > 0
                    && i64::from(*src_x) + i64::from(*width) <= 640
                    && i64::from(*src_y) + i64::from(*height) <= 200
                    && i64::from(*dst_x) + i64::from(*width) <= 640
                    && i64::from(*dst_y) + i64::from(*height) <= 200
                    && *minterm == 192
                    && *mask == 255
            }
            ResetFont | ResetPalette | SetDisplayMode { .. } => true,
            _ => false,
        },
    }
}

impl SkypixDraft {
    pub fn new(document: &SkypixDocument) -> Self {
        Self {
            document: document.draft_copy(),
            original: document.items().to_vec(),
            revision: document.revision(),
            preview_enabled: true,
            preview_calls: 0,
        }
    }
    pub fn items(&self) -> &[SkypixItem] {
        self.document.items()
    }
    pub fn begin_turn(&mut self, enabled: bool) {
        self.preview_enabled = enabled;
        self.preview_calls = 0;
    }
    pub fn changed(&self) -> bool {
        self.items() != self.original
    }
    pub fn changes(&self) -> usize {
        let prefix = self.original.iter().zip(self.items()).take_while(|(a, b)| a == b).count();
        let suffix = self.original[prefix..]
            .iter()
            .rev()
            .zip(self.items()[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        self.original.len().max(self.items().len()) - prefix - suffix
    }
    pub fn matches(&self, document: &SkypixDocument) -> bool {
        document.revision() == self.revision && document.items() == self.original
    }
    pub fn omitted(&self) -> usize {
        self.items().iter().filter(|item| !editable(item)).count()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.items().len() > MAX_ITEMS {
            return Err("SkyPix draft exceeds 1024 items".into());
        }
        let before: Vec<_> = self.original.iter().filter(|item| !editable(item)).collect();
        let after: Vec<_> = self.items().iter().filter(|item| !editable(item)).collect();
        if before != after {
            return Err("Runtime, external and unsupported SkyPix items must stay unchanged and ordered".into());
        }
        if self.document.to_bytes().map_err(|e| e.to_string())?.len() > MAX_BYTES {
            return Err("SkyPix draft exceeds 128 KiB".into());
        }
        Ok(())
    }
    pub fn preview(&self) -> Result<SkypixPreview, String> {
        self.validate()?;
        self.document
            .preview_with_items(self.items().iter().filter(|item| editable(item)).cloned().collect())
            .map_err(|e| e.to_string())
    }
    pub fn preview_image(&mut self) -> Result<super::image_attachment::ReferenceImage, String> {
        if !self.preview_enabled {
            return Err("Select an image-capable model for SkyPix feedback".into());
        }
        if self.preview_calls >= 3 {
            return Err("SkyPix preview limit reached (3 per turn)".into());
        }
        let preview = self.preview()?;
        let raw = image::RgbaImage::from_raw(preview.width() as u32, preview.height() as u32, preview.rgba()).ok_or("Invalid SkyPix preview dimensions")?;
        // SkyPix's 640x200 display is shown at 2:1 vertical pixel correction by the editor.
        let image = image::imageops::resize(&raw, 640, 400, image::imageops::FilterType::Nearest);
        let mut bytes = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, image::ImageFormat::Png)
            .map_err(|e| format!("Cannot encode SkyPix preview: {e}"))?;
        let result = super::image_attachment::prepare("skypix-static-preview.png", &bytes.into_inner())?;
        self.preview_calls += 1;
        Ok(result)
    }
    pub fn call(&mut self, tool: &str, args: &Value) -> Result<String, String> {
        match tool {
            "icy_skypix_info" => {
                let mut colors = 16;
                for command in self.items().iter().filter_map(SkypixItem::as_command) {
                    if let SkypixCommand::SetDisplayMode { mode } = command {
                        colors = if *mode == DisplayMode::EightColors { 8 } else { 16 };
                    }
                }
                let fonts: Vec<_> = icy_engine::skypix::load_amiga_fonts()
                    .into_iter()
                    .map(|(name, size, _, _)| json!({"name":name,"size":size}))
                    .collect();
                Ok(json!({"editor":"SkyPix","width":640,"height":200,"display_aspect_scale_y":2,"colors":colors,
                    "items":self.items().len(),"omitted_static_items":self.omitted(),"bundled_fonts":fonts,
                    "limits":{"items":MAX_ITEMS,"source_bytes":MAX_BYTES,"read_items":64,"previews_per_turn":3},
                    "notes":"Ordered pixel graphics/state plus CP437 terminal text and safe ANSI. Not ANSI cells, RIP or IGS. Runtime/external/unsupported items are read-only and omitted from static previews; playback may differ. Read icy_skypix_api and item pages first."}).to_string())
            }
            "icy_skypix_api" => Ok(API.into()),
            "icy_read_skypix_items" => {
                let start = index(args, "start", Some(0))?;
                let count = index(args, "count", Some(32))?;
                if count == 0 || count > 64 || start > self.items().len() {
                    return Err("Read 1..64 SkyPix items from a valid start".into());
                }
                let end = start.saturating_add(count).min(self.items().len());
                let mut items = Vec::new();
                for i in start..end {
                    let source = self.document.item_source(i).map_err(|e| e.to_string())?;
                    items.push(
                        json!({"index":i,"editable":editable(&self.items()[i]),"source_hex":super::igs_tools::hex(&source),
                        "semantic":format!("{:?}",self.items()[i]),"text":self.items()[i].decoded_text()}),
                    );
                }
                let result = json!({"total":self.items().len(),"start":start,"next":end,"items":items}).to_string();
                if result.len() > 64 * 1024 {
                    return Err("SkyPix read exceeds 64 KiB; read fewer items".into());
                }
                Ok(result)
            }
            "icy_replace_skypix_items" => self.replace(args),
            _ => Err(format!("Unknown SkyPix tool {tool}")),
        }
    }
    fn replace(&mut self, args: &Value) -> Result<String, String> {
        let start = index(args, "start", None)?;
        let count = index(args, "delete_count", None)?;
        let end = start.checked_add(count).ok_or("SkyPix range overflow")?;
        if start > self.items().len() || end > self.items().len() {
            return Err("SkyPix replacement range is outside the draft".into());
        }
        if self.items()[start..end].iter().any(|item| !editable(item)) {
            return Err("Selected range contains read-only runtime/external/unsupported SkyPix items".into());
        }
        let bytes = match (args.get("source"), args.get("source_hex")) {
            (Some(source), None) => {
                let source = source.as_str().ok_or("source must be an ASCII string")?;
                if !source.is_ascii() || source.len() > MAX_BYTES {
                    return Err("Use ASCII source <=128 KiB or source_hex for native bytes".into());
                }
                source.as_bytes().to_vec()
            }
            (None, Some(source)) => super::igs_tools::unhex(source.as_str().ok_or("source_hex must be a string")?)?,
            _ => return Err("Specify exactly one of source or source_hex".into()),
        };
        let parsed = SkypixDocument::from_bytes(&bytes).map_err(|e| e.to_string())?;
        if parsed.items().iter().any(|item| !editable(item)) {
            return Err(
                "New SkyPix items must be bounded static graphics/state or safe terminal text; runtime, external and unsupported operations are unavailable"
                    .into(),
            );
        }
        let mut colors = 16;
        let mut brush = None;
        for command in self.items()[..start].iter().filter_map(SkypixItem::as_command) {
            update_state(command, &mut colors, &mut brush);
        }
        for command in parsed.items().iter().filter_map(SkypixItem::as_command) {
            match command {
                SkypixCommand::SetPenA { color } | SkypixCommand::SetPenB { color } if *color >= colors => {
                    return Err(format!("Pen must be in 0..{} for the active display mode", colors - 1))
                }
                SkypixCommand::UseBrush {
                    src_x, src_y, width, height, ..
                } => {
                    let Some((bw, bh)) = brush else {
                        return Err("Capture a local brush before using it; transferred brushes are unavailable".into());
                    };
                    if src_x + width > bw || src_y + height > bh {
                        return Err("Source region is outside the captured brush".into());
                    }
                }
                _ => {}
            }
            update_state(command, &mut colors, &mut brush);
        }
        let mut items = self.items().to_vec();
        let inserted = parsed.items().len();
        items.splice(start..end, parsed.items().iter().cloned());
        let mut candidate = self.clone();
        candidate
            .document
            .set_items(items)
            .map_err(|e| format!("SkyPix round-trip validation failed: {e}"))?;
        if state_errors(candidate.items()) != state_errors(self.items()) {
            return Err("Replacement creates an invalid pen for the active color mode or an unavailable/out-of-bounds brush dependency".into());
        }
        candidate.validate()?;
        self.document = candidate.document;
        Ok(format!(
            "Replaced {count} items with {inserted} in the SkyPix draft. Review the static preview before Apply."
        ))
    }
}

fn update_state(command: &SkypixCommand, colors: &mut i32, brush: &mut Option<(i32, i32)>) {
    match command {
        SkypixCommand::SetDisplayMode { mode } => *colors = if *mode == DisplayMode::EightColors { 8 } else { 16 },
        SkypixCommand::GrabBrush { width, height, .. } if editable(&SkypixItem::Command(command.clone())) => *brush = Some((*width, *height)),
        SkypixCommand::GrabBrush { .. } => *brush = None,
        SkypixCommand::CrcTransfer { .. } => *brush = None,
        _ => {}
    }
}

fn state_errors(items: &[SkypixItem]) -> Vec<SkypixItem> {
    let mut colors = 16;
    let mut brush = None;
    let mut errors = Vec::new();
    for item in items {
        let Some(command) = item.as_command() else { continue };
        let invalid = match command {
            SkypixCommand::SetPenA { color } | SkypixCommand::SetPenB { color } => *color >= colors,
            SkypixCommand::UseBrush {
                src_x, src_y, width, height, ..
            } if editable(item) => {
                brush.is_none_or(|(bw, bh)| i64::from(*src_x) + i64::from(*width) > i64::from(bw) || i64::from(*src_y) + i64::from(*height) > i64::from(bh))
            }
            _ => false,
        };
        if invalid {
            errors.push(item.clone());
        }
        update_state(command, &mut colors, &mut brush);
    }
    errors
}

pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("icy_skypix_info","Describe the SkyPix draft, fixed pixel/display dimensions, color depth, bundled fonts, protected items and limits. Call first.",json!({"type":"object","properties":{}})),
        ("icy_skypix_api","Read SkyPix wire syntax, drawing/state and text tips, supported commands and static-preview rules.",json!({"type":"object","properties":{}})),
        ("icy_read_skypix_items","Read up to 64 ordered SkyPix items with exact native source hex, semantic parameters and editability.",json!({"type":"object","properties":{"start":{"type":"integer","minimum":0},"count":{"type":"integer","minimum":1,"maximum":64}}})),
        ("icy_replace_skypix_items","Atomically insert/delete/replace zero-based ordered SkyPix draft items. Provide ASCII source (including actual ESC) OR source_hex. start/delete_count required. Empty source deletes; delete_count=0 inserts. Preserves untouched source bytes and read-only runtime items. User Apply required.",json!({"type":"object","properties":{"start":{"type":"integer","minimum":0},"delete_count":{"type":"integer","minimum":0},"source":{"type":"string"},"source_hex":{"type":"string"}},"required":["start","delete_count"]})),
        ("icy_preview_skypix","Return an aspect-corrected rendered PNG of the STATIC SkyPix draft. Audio/delays/transfers/controller/gadgets and unsafe/unsupported fragments are omitted, never executed. At most 3 previews/turn; image-capable model required.",json!({"type":"object","properties":{}})),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn insert(draft: &mut SkypixDraft, start: usize, source: &str) -> Result<String, String> {
        draft.call("icy_replace_skypix_items", &json!({"start":start,"delete_count":0,"source":source}))
    }
    #[test]
    fn mixed_source_remains_lossless_runtime_is_protected_and_graphics_render() {
        let source = b"\x1b[015;3!\x1b[14;999999!\x1b[31mHELLO\x82\x1b[99!";
        let document = SkypixDocument::from_bytes(source).unwrap();
        let mut draft = SkypixDraft::new(&document);
        assert_eq!(draft.document.to_bytes().unwrap(), source);
        let baseline = draft.preview().unwrap().rgba();
        insert(&mut draft, 1, "\x1b[4;10;10;50;50!").unwrap();
        assert!(draft.changed());
        assert_eq!(draft.omitted(), 2);
        assert_ne!(draft.preview().unwrap().rgba(), baseline);
        let bytes = draft.document.to_bytes().unwrap();
        assert!(bytes.starts_with(b"\x1b[015;3!"));
        assert!(bytes.ends_with(&source[b"\x1b[015;3!".len()..]));
        let read: Value = serde_json::from_str(&draft.call("icy_read_skypix_items", &json!({"start":0,"count":64})).unwrap()).unwrap();
        assert_eq!(read["items"][0]["source_hex"], super::super::igs_tools::hex(b"\x1b[015;3!"));
        assert_eq!(read["items"][2]["editable"], false);
        let snapshot = bytes.clone();
        assert!(draft
            .call("icy_replace_skypix_items", &json!({"start":2,"delete_count":1,"source":""}))
            .is_err());
        assert_eq!(draft.document.to_bytes().unwrap(), snapshot);
        assert!(draft.matches(&document));
        let mut live = document.draft_copy();
        live.set_items(draft.items().to_vec()).unwrap();
        assert_eq!(live.to_bytes().unwrap(), bytes);
        assert!(live.undo());
        assert_eq!(live.to_bytes().unwrap(), source);
    }
    #[test]
    fn unsafe_or_invalid_sources_fail_atomically() {
        let mut draft = SkypixDraft::new(&SkypixDocument::new());
        for source in [
            "\x1b[14;999!",
            "\x1b[9;1;0;100;1!",
            "\x1b[16;0;1;1!file!",
            "\x1b[21;1;0;0!",
            "\x1b[22;1;1;0;0;10;10!",
            "\x1b[99!",
            "\x1b[10;8!missing.font!",
            "\x1b]52;c;PAYLOAD\x07",
            "\x1b[6n",
            "\x07",
            "\x1b[1;640;0!",
            "\x1b[13;0;0;99999;99999!",
            "\x1b[15;16!",
            "\x1b[7;0;0;0;0;1;1;192;255!",
            "\x1b[7;0;0;0;0;1;1;0;255!",
            "\x1b[",
        ] {
            assert!(insert(&mut draft, 0, source).is_err(), "{source:?}");
            assert!(draft.items().is_empty());
        }
        assert!(draft
            .call("icy_replace_skypix_items", &json!({"start":0,"delete_count":0,"source":"","source_hex":""}))
            .is_err());
        assert!(draft
            .call("icy_replace_skypix_items", &json!({"start":0,"delete_count":0,"source_hex":"ZZ"}))
            .is_err());
    }
    #[test]
    fn native_text_palette_font_and_brush_copy_render_real_pixels() {
        let mut draft = SkypixDraft::new(&SkypixDocument::new());
        let palette = std::iter::once("0").chain(std::iter::repeat_n("15", 15)).collect::<Vec<_>>().join(";");
        insert(
            &mut draft,
            0,
            &format!(
                "\x1b[11;{palette}!\x1b[15;1!\x1b[4;10;10;30;30!\x1b[6;10;10;10;10!\x1b[7;0;0;100;100;10;10;192;255!\x1b[10;8!Topaz.font!\x1b[19;10;60!LABEL"
            ),
        )
        .unwrap();
        let image = draft.preview().unwrap();
        let rgba = image.rgba();
        for (x, y) in [(20, 20), (100, 100)] {
            assert_eq!(&rgba[(y * 640 + x) * 4..(y * 640 + x) * 4 + 4], &[255, 0, 0, 255]);
        }
        let start = draft.items().len();
        insert(&mut draft, start, "\x1b[10;0!").unwrap();
        let start = draft.items().len();
        draft
            .call("icy_replace_skypix_items", &json!({"start":start,"delete_count":0,"source_hex":"82"}))
            .unwrap();
        assert!(draft.items().iter().any(|item| item.as_text().is_some_and(|bytes| bytes.contains(&0x82))));
        let info: Value = serde_json::from_str(&draft.call("icy_skypix_info", &json!({})).unwrap()).unwrap();
        assert_eq!(info["width"], 640);
        assert!(info["bundled_fonts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|font| font["name"] == "Topaz.font" && font["size"] == 8));
    }
    #[test]
    fn color_mode_and_brush_dependencies_survive_replacement() {
        let mut draft = SkypixDraft::new(&SkypixDocument::new());
        insert(&mut draft, 0, "\x1b[17;1!").unwrap();
        assert!(insert(&mut draft, 1, "\x1b[15;8!").is_err());
        insert(&mut draft, 1, "\x1b[15;7!").unwrap();
        let mut draft = SkypixDraft::new(&SkypixDocument::from_bytes(b"\x1b[15;15!").unwrap());
        assert!(
            insert(&mut draft, 0, "\x1b[17;1!").is_err(),
            "mode change must also validate dependent existing commands"
        );
        let mut draft = SkypixDraft::new(&SkypixDocument::new());
        insert(&mut draft, 0, "\x1b[6;0;0;10;10!\x1b[7;0;0;20;20;10;10;192;255!").unwrap();
        assert!(draft
            .call("icy_replace_skypix_items", &json!({"start":0,"delete_count":1,"source":""}))
            .is_err());
    }
    #[test]
    fn source_read_and_preview_limits_are_enforced() {
        let mut draft = SkypixDraft::new(&SkypixDocument::new());
        assert!(insert(&mut draft, 0, &"\x1b[1;1;1!".repeat(MAX_ITEMS + 1)).is_err());
        assert!(insert(&mut draft, 0, &"X".repeat(MAX_BYTES + 1)).is_err());
        assert!(draft.call("icy_read_skypix_items", &json!({"count":65})).is_err());
        assert!(draft.call("icy_read_skypix_items", &json!({"start":1})).is_err());
        draft.begin_turn(false);
        assert!(draft.preview_image().is_err());
        draft.begin_turn(true);
        for _ in 0..3 {
            assert!(draft.preview_image().is_ok());
        }
        assert!(draft.preview_image().is_err());
        draft.begin_turn(true);
        assert!(draft.preview_image().is_ok());
    }
}
