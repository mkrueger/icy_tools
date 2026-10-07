//! Command-level RIP drafts. Parsing, round-trip checks and rendering reuse the RIP editor.

use icy_draw::rip_document::{RipDocument, RipPreview};
use icy_parser_core::RipCommand;
use serde_json::{json, Value};

const MAX_SOURCE_BYTES: usize = 128 * 1024;
const MAX_COMMANDS: usize = 2048;
const MAX_READ_COMMANDS: usize = 128;
const MAX_READ_BYTES: usize = 64 * 1024;
pub const API: &str = include_str!("../../../data/ai/references/ripscrip.md");

#[derive(Clone)]
pub struct RipDraft {
    pub original: Vec<RipCommand>,
    pub commands: Vec<RipCommand>,
    pub revision: u64,
    pub preserved: usize,
    preserved_source: Option<Vec<u8>>,
}

impl RipDraft {
    pub fn new(document: &RipDocument) -> Self {
        Self {
            original: document.commands().to_vec(),
            commands: document.commands().to_vec(),
            revision: document.revision(),
            preserved: document.preserved_commands(),
            preserved_source: document.preserved_source().map(<[u8]>::to_vec),
        }
    }

    pub fn changed(&self) -> bool {
        self.commands != self.original
    }

    pub fn changes(&self) -> usize {
        let prefix = self.original.iter().zip(&self.commands).take_while(|(a, b)| a == b).count();
        let suffix = self.original[prefix..]
            .iter()
            .rev()
            .zip(self.commands[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        self.original.len().max(self.commands.len()) - prefix - suffix
    }

    pub fn matches(&self, document: &RipDocument) -> bool {
        document.revision() == self.revision && document.commands() == self.original && document.preserved_source() == self.preserved_source.as_deref()
    }

    fn document(&self) -> Result<RipDocument, String> {
        let mut document = match &self.preserved_source {
            Some(source) => RipDocument::from_bytes(source).map_err(|error| error.to_string())?,
            None => RipDocument::new(),
        };
        document
            .replace_editable(self.commands[self.preserved..].to_vec())
            .map_err(|error| error.to_string())?;
        Ok(document)
    }

    pub fn preview(&self) -> Result<RipPreview, String> {
        self.validate()?;
        self.document()?.preview().map_err(|error| error.to_string())
    }

    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<String, String> {
        match tool {
            "icy_rip_info" => Ok(json!({
                "editor": "RIP", "width": 640, "height": 350, "palette_indices": "0..15",
                "commands": self.commands.len(), "preserved_commands": self.preserved,
                "max_commands": MAX_COMMANDS, "max_source_bytes": MAX_SOURCE_BYTES,
                "notes": "Ordered, stateful RIPscrip commands. Indices are zero-based; commands before preserved_commands are read-only. \
                          Read existing commands and icy_rip_api first. All changes are drafts; the user reviews a rendered preview and Apply/Discard.",
            })
            .to_string()),
            "icy_rip_api" => Ok(API.into()),
            "icy_read_rip_commands" => self.read(arguments),
            "icy_replace_rip_commands" => self.replace(arguments),
            _ => Err(format!("Unknown RIP tool {tool}")),
        }
    }

    fn read(&self, arguments: &Value) -> Result<String, String> {
        let start = optional_index(arguments, "start", 0)?;
        let count = optional_index(arguments, "count", 64)?;
        if count == 0 || count > MAX_READ_COMMANDS || start > self.commands.len() {
            return Err(format!("Read 1..{MAX_READ_COMMANDS} commands, starting at 0..{}.", self.commands.len()));
        }
        let end = start.saturating_add(count).min(self.commands.len());
        let commands: Vec<_> = self.commands[start..end].iter().enumerate().map(|(offset, command)| {
            json!({"index": start + offset, "source": format!("!{command}\r\n"), "semantic": format!("{command:?}"), "editable": start + offset >= self.preserved})
        }).collect();
        let output = json!({"total": self.commands.len(), "start": start, "next": end, "commands": commands}).to_string();
        if output.len() > MAX_READ_BYTES {
            return Err("RIP read response exceeds 64 KiB. Read fewer commands.".into());
        }
        Ok(output)
    }

    fn replace(&mut self, arguments: &Value) -> Result<String, String> {
        let start = required_index(arguments, "start")?;
        let delete_count = required_index(arguments, "delete_count")?;
        let end = start.checked_add(delete_count).ok_or("RIP command range overflow")?;
        if start < self.preserved || start > self.commands.len() || end > self.commands.len() {
            return Err(format!(
                "Invalid RIP command range. Editable indices start at {} and end at {} (exclusive).",
                self.preserved,
                self.commands.len()
            ));
        }
        let source = arguments.get("source").and_then(Value::as_str).ok_or("source is required")?;
        if source.len() > MAX_SOURCE_BYTES {
            return Err("RIP replacement source exceeds 128 KiB.".into());
        }
        // Wire strings must remain ASCII: the existing RIP parser consumes individual bytes.
        if !source.is_ascii() {
            return Err("New RIP source must be ASCII. Preserve existing non-ASCII text instead of re-encoding it.".into());
        }
        let parsed = RipDocument::from_bytes(source.as_bytes()).map_err(|error| format!("Invalid RIP source: {error}"))?;
        if parsed.preserved_source().is_some() {
            return Err("Replacement must contain only round-trippable RIP commands, not ANSI/plain text or incomplete commands.".into());
        }
        let mut candidate = self.clone();
        candidate.commands.splice(start..end, parsed.commands().iter().cloned());
        candidate.validate()?;
        self.commands = candidate.commands;
        Ok(format!(
            "Replaced {delete_count} commands with {} commands in the RIP draft. Review the preview before applying.",
            parsed.commands().len()
        ))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.commands.len() > MAX_COMMANDS {
            return Err("RIP draft exceeds 2048 commands.".into());
        }
        for (index, command) in self.commands.iter().enumerate() {
            let error = match command {
                RipCommand::LoadIcon { .. }
                | RipCommand::WriteIcon { .. }
                | RipCommand::ReadScene { .. }
                | RipCommand::FileQuery { .. }
                | RipCommand::EnterBlockMode { .. }
                | RipCommand::Query { .. } => Some("external-file, host-query and transfer commands are unavailable"),
                RipCommand::Unsupported { .. } | RipCommand::NoMore => Some("unsupported commands and stream terminators are unavailable"),
                RipCommand::ButtonStyle { flags, .. } if flags & 128 != 0 => Some("external-icon button styles are unavailable"),
                RipCommand::ButtonStyle {
                    dfore,
                    dback,
                    bright,
                    dark,
                    surface,
                    uline_col,
                    corner_col,
                    ..
                } if [dfore, dback, bright, dark, surface, uline_col, corner_col].iter().any(|value| **value > 15) => Some("button colors must be 0..15"),
                RipCommand::Button { hotkey, .. } if *hotkey > 255 => Some("button hotkey must be a byte code in 0..255"),
                RipCommand::Color { c } if *c > 15 => Some("palette index must be 0..15"),
                RipCommand::FillStyle { color, .. } | RipCommand::FillPattern { col: color, .. } if *color > 15 => Some("fill color must be 0..15"),
                RipCommand::FillPattern {
                    c1,
                    c2,
                    c3,
                    c4,
                    c5,
                    c6,
                    c7,
                    c8,
                    ..
                } if [c1, c2, c3, c4, c5, c6, c7, c8].iter().any(|row| **row > 255) => Some("fill pattern rows must be bytes in 0..255"),
                RipCommand::Fill { border, .. } if *border > 15 => Some("border color must be 0..15"),
                RipCommand::OnePalette { color, value } if *color > 15 || *value > 63 => Some("palette slot must be 0..15 and EGA value 0..63"),
                RipCommand::SetPalette { colors } if colors.len() != 16 || colors.iter().any(|value| *value > 63) => {
                    Some("palette needs exactly 16 EGA values in 0..63")
                }
                RipCommand::FontStyle { font, direction, size, .. } if *font > 10 || *direction > 1 || !(1..=10).contains(size) => {
                    Some("font must be 0..10, direction 0..1 and size 1..10")
                }
                RipCommand::LineStyle { thick, .. } if ![1, 3].contains(thick) => Some("line thickness must be 1 or 3"),
                RipCommand::Bezier { cnt, .. } if !(1..=256).contains(cnt) => Some("Bezier segment count must be 1..256"),
                RipCommand::Polygon { points } | RipCommand::FilledPolygon { points } | RipCommand::PolyLine { points }
                    if points.len() < 4 || points.len() > 512 || points.len() % 2 != 0 =>
                {
                    Some("paths need 2..256 coordinate pairs")
                }
                RipCommand::TextWindow { x0, y0, x1, y1, size, .. } if *x0 > 79 || *x1 > 79 || *y0 > 42 || *y1 > 42 || *size > 4 => {
                    Some("text window must fit 80x43 cells and use size 0..4")
                }
                _ => None,
            };
            if let Some(error) = error {
                return Err(format!("RIP command {index}: {error}."));
            }
        }
        let document = self.document()?;
        if document.to_bytes().map_err(|error| error.to_string())?.len() > MAX_SOURCE_BYTES {
            return Err("RIP draft source exceeds 128 KiB.".into());
        }
        Ok(())
    }
}

fn required_index(arguments: &Value, name: &str) -> Result<usize, String> {
    let value = arguments
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{name} must be a nonnegative integer"))?;
    usize::try_from(value).map_err(|_| format!("{name} is out of range"))
}

fn optional_index(arguments: &Value, name: &str, default: usize) -> Result<usize, String> {
    if arguments.get(name).is_some() {
        required_index(arguments, name)
    } else {
        Ok(default)
    }
}

pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let empty = json!({"type": "object", "properties": {}, "additionalProperties": false});
    vec![
        ("icy_rip_info", "Get RIP draft dimensions, palette and command/editability limits.", empty.clone()),
        ("icy_rip_api", "Read RIPscrip authoring guidance and supported wire syntax before editing.", empty),
        ("icy_read_rip_commands", "Read a bounded page of numbered RIP commands, with wire source and decoded parameters.", json!({
            "type": "object", "properties": {
                "start": {"type": "integer", "minimum": 0, "default": 0},
                "count": {"type": "integer", "minimum": 1, "maximum": MAX_READ_COMMANDS, "default": 64}
            }, "additionalProperties": false
        })),
        ("icy_replace_rip_commands", "Atomically replace/delete a command range or insert a RIPscrip batch. Read existing commands first. Zero-based start; delete_count=0 inserts, start=command count appends. Only changes the reviewed draft.", json!({
            "type": "object", "properties": {
                "start": {"type": "integer", "minimum": 0},
                "delete_count": {"type": "integer", "minimum": 0},
                "source": {"type": "string", "description": "ASCII RIPscrip source, e.g. !|c0B|L00002S2S\\r\\n. Empty deletes. Fixed-width base-36 parameters, not decimal."}
            }, "required": ["start", "delete_count", "source"], "additionalProperties": false
        })),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replace(draft: &mut RipDraft, start: usize, count: usize, source: &str) -> Result<String, String> {
        draft.call("icy_replace_rip_commands", &json!({"start": start, "delete_count": count, "source": source}))
    }

    #[test]
    fn batches_read_render_replace_and_delete_without_touching_the_document() {
        let document = RipDocument::new();
        let mut draft = RipDraft::new(&document);
        assert!(draft.call("icy_rip_api", &json!({})).unwrap().contains("base-36"));
        replace(&mut draft, 0, 0, "!|c0B|L00002S2S\r\n").unwrap();
        assert!(document.commands().is_empty());
        assert!(draft.changed());
        assert_eq!(draft.changes(), 2);
        let read: Value = serde_json::from_str(&draft.call("icy_read_rip_commands", &json!({"start": 1, "count": 1})).unwrap()).unwrap();
        assert_eq!(read["next"], 2);
        assert_eq!(read["commands"][0]["source"], "!|L00002S2S\r\n");
        assert!(read["commands"][0]["semantic"].as_str().unwrap().contains("x1: 100"));
        let preview = draft.preview().unwrap();
        assert_eq!((preview.width(), preview.height()), (640, 350));
        assert_eq!(preview.pixel_index(50, 50), Some(11));
        replace(&mut draft, 0, 1, "!|c0F\n").unwrap();
        assert_eq!(draft.preview().unwrap().pixel_index(50, 50), Some(15));
        replace(&mut draft, 0, 2, "").unwrap();
        assert!(!draft.changed());
        assert_eq!(draft.changes(), 0);
    }

    #[test]
    fn invalid_ranges_sources_and_external_operations_are_atomic() {
        let mut draft = RipDraft::new(&RipDocument::new());
        replace(&mut draft, 0, 0, "!|c0B\n").unwrap();
        let before = draft.commands.clone();
        for source in [
            "!|L00\n",
            "!|L00000000|L\n",
            "plain text\n!|c0F\n",
            "!|@0000é\n",
            "!|c0G\n",
            "!|Y00000000\n",
            "!|#\n",
            "!|2xunknown\n",
            "!|1W\x00picture.icn\n",
        ] {
            assert!(replace(&mut draft, 1, 0, source).is_err(), "{source:?}");
            assert_eq!(draft.commands, before);
        }
        for command in [
            RipCommand::ReadScene {
                res: 0,
                file_name: "screen.rip".into(),
            },
            RipCommand::ButtonStyle {
                wid: 20,
                hgt: 10,
                orient: 0,
                flags: 128,
                bevsize: 1,
                dfore: 15,
                dback: 0,
                bright: 15,
                dark: 0,
                surface: 1,
                grp_no: 0,
                flags2: 0,
                uline_col: 15,
                corner_col: 0,
                res: 0,
            },
        ] {
            assert!(replace(&mut draft, 1, 0, &format!("!{command}\n")).is_err());
            assert_eq!(draft.commands, before);
        }
        for arguments in [
            json!({"start": -1, "delete_count": 0, "source": ""}),
            json!({"start": 0, "delete_count": u64::MAX, "source": ""}),
            json!({"start": 2, "delete_count": 0, "source": ""}),
            json!({"start": 0.5, "delete_count": 0, "source": ""}),
        ] {
            assert!(draft.call("icy_replace_rip_commands", &arguments).is_err());
            assert_eq!(draft.commands, before);
        }
    }

    #[test]
    fn mixed_streams_keep_the_prefix_and_reject_edits_to_preserved_commands() {
        let source = b"ANSI text\r\n!|c07\r\n";
        let document = RipDocument::from_bytes(source).unwrap();
        let mut draft = RipDraft::new(&document);
        assert_eq!(draft.preserved, 1);
        assert!(replace(&mut draft, 0, 1, "!|c0F\n").is_err());
        replace(&mut draft, 1, 0, "!|X0101\n").unwrap();
        assert!(draft.document().unwrap().to_bytes().unwrap().starts_with(source));
        assert_eq!(draft.preview().unwrap().pixel_index(1, 1), Some(7));
        assert!(draft.matches(&document));
    }

    #[test]
    fn exact_command_source_and_read_limits_are_enforced() {
        let mut draft = RipDraft::new(&RipDocument::new());
        replace(&mut draft, 0, 0, &"!|c07\n".repeat(MAX_COMMANDS)).unwrap();
        assert_eq!(draft.commands.len(), MAX_COMMANDS);
        assert!(replace(&mut draft, MAX_COMMANDS, 0, "!|c07\n").is_err());
        assert_eq!(draft.commands.len(), MAX_COMMANDS);
        assert!(draft.call("icy_read_rip_commands", &json!({"count": MAX_READ_COMMANDS})).is_ok());
        for arguments in [
            json!({"count": MAX_READ_COMMANDS + 1}),
            json!({"count": 0}),
            json!({"start": -1}),
            json!({"start": MAX_COMMANDS + 1}),
        ] {
            assert!(draft.call("icy_read_rip_commands", &arguments).is_err());
        }
        let mut draft = RipDraft::new(&RipDocument::new());
        let source = format!("!|!{}\r\n", "x".repeat(MAX_SOURCE_BYTES - 5));
        assert_eq!(source.len(), MAX_SOURCE_BYTES);
        replace(&mut draft, 0, 0, &source).unwrap();
        assert!(draft.call("icy_read_rip_commands", &json!({"count": 1})).unwrap_err().contains("64 KiB"));
        assert!(replace(&mut draft, 1, 0, "!|c07\n").unwrap_err().contains("128 KiB"));
        assert_eq!(draft.commands.len(), 1);
        assert!(replace(&mut draft, 0, 1, &format!("{source} ")).is_err());
    }

    #[test]
    fn original_example_is_valid_and_renders() {
        let example = API.split("```text\n").nth(1).unwrap().split("```").next().unwrap();
        let mut draft = RipDraft::new(&RipDocument::new());
        replace(&mut draft, 0, 0, example).unwrap();
        assert_eq!(draft.preview().unwrap().pixel_index(20, 20), Some(11));
    }
}
