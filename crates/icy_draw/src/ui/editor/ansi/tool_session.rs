//! Tool session state for the ANSI editor
//!
//! Captures per-tool settings (brush mode, paint char, color filters, brush
//! size, fill mode, selection mode, font slot, shape variant, ...) so they
//! can be persisted alongside the rest of the editor session and restored
//! when the editor is reopened.
//!
//! The state is serialized as a versioned bitcode enum and stored in
//! `AnsiEditorSessionState::tool_state_blob`.

use icy_engine_edit::tools::Tool;
use serde::{Deserialize, Serialize};

use super::tools::BrushSettings;
use super::widget::toolbar::top::{BrushPrimaryMode, SelectionMode};

/// Brush-style tool settings shared by Pencil, Shape and Fill tools.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BrushSessionState {
    pub primary: BrushPrimaryMode,
    pub paint_char: char,
    pub brush_size: u32,
    pub colorize_fg: bool,
    pub colorize_bg: bool,
    /// Fill tool: only fill cells whose character/colors exactly match
    /// (ignored by Pencil/Shape).
    #[serde(default)]
    pub exact: bool,
}

impl Default for BrushSessionState {
    fn default() -> Self {
        let b = BrushSettings::default();
        Self::from(b)
    }
}

impl From<BrushSettings> for BrushSessionState {
    fn from(b: BrushSettings) -> Self {
        Self {
            primary: b.primary,
            paint_char: b.paint_char,
            brush_size: b.brush_size,
            colorize_fg: b.colorize_fg,
            colorize_bg: b.colorize_bg,
            exact: b.exact,
        }
    }
}

impl From<BrushSessionState> for BrushSettings {
    fn from(s: BrushSessionState) -> Self {
        Self {
            primary: s.primary,
            paint_char: s.paint_char,
            brush_size: s.brush_size.max(1),
            colorize_fg: s.colorize_fg,
            colorize_bg: s.colorize_bg,
            exact: s.exact,
        }
    }
}

/// Shape tool extends brush with the chosen shape variant.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ShapeSessionState {
    /// Currently selected shape (Line, RectangleOutline, RectangleFilled,
    /// EllipseOutline, EllipseFilled).
    #[serde(default = "default_shape_tool")]
    pub shape: Tool,
}

fn default_shape_tool() -> Tool {
    Tool::RectangleOutline
}

/// Aggregate tool session state serialized into the editor session blob.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnsiToolSessionState {
    /// Currently active tool.
    #[serde(default)]
    pub selected_tool: Tool,

    /// Single shared brush state covering Pencil/Shape/Fill.
    #[serde(default)]
    pub brush: BrushSessionState,

    #[serde(default)]
    pub shape: ShapeSessionState,

    #[serde(default)]
    pub selection_mode: SelectionMode,

    #[serde(default)]
    pub font_slot: usize,

    /// MRU paint-char history (newest first), capped at 8 entries by the
    /// editor on push. Restored on session reopen so the cycle hotkeys (#9)
    /// keep working across runs.
    #[serde(default)]
    pub recent_chars: Vec<char>,
}

/// Legacy V1 payload used before the explicit session-version wrapper existed.
///
/// V1 did not persist the paint-char MRU history introduced for #9. When a V1
/// blob is loaded, migration seeds `recent_chars` with the current brush char so
/// the new cycle hotkeys have at least one safe entry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnsiToolSessionStateV1 {
    /// Currently active tool.
    #[serde(default)]
    pub selected_tool: Tool,

    /// Single shared brush state covering Pencil/Shape/Fill.
    #[serde(default)]
    pub brush: BrushSessionState,

    #[serde(default)]
    pub shape: ShapeSessionState,

    #[serde(default)]
    pub selection_mode: SelectionMode,

    #[serde(default)]
    pub font_slot: usize,
}

impl From<AnsiToolSessionStateV1> for AnsiToolSessionState {
    fn from(v1: AnsiToolSessionStateV1) -> Self {
        Self {
            selected_tool: v1.selected_tool,
            brush: v1.brush,
            shape: v1.shape,
            selection_mode: v1.selection_mode,
            font_slot: v1.font_slot,
            recent_chars: vec![v1.brush.paint_char],
        }
    }
}

/// Versioned tool-session envelope stored in `tool_state_blob`.
///
/// New code always writes the newest variant. Decoding accepts every historical
/// format we know about:
///
/// 1. `SessionVersion::V2(current)` — current explicit wrapper.
/// 2. `SessionVersion::V1(legacy)` — explicit V1 wrapper, migrated to V2.
/// 3. Bare `AnsiToolSessionState` — short-lived unversioned V2 blobs created
///    before #7.
/// 4. Bare `AnsiToolSessionStateV1` — older unversioned blobs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SessionVersion {
    V1(AnsiToolSessionStateV1),
    V2(AnsiToolSessionState),
}

impl SessionVersion {
    fn into_current(self) -> AnsiToolSessionState {
        match self {
            SessionVersion::V1(v1) => v1.into(),
            SessionVersion::V2(v2) => v2,
        }
    }
}

impl AnsiToolSessionState {
    /// Encode to a versioned bitcode blob suitable for
    /// `AnsiEditorSessionState::tool_state_blob`.
    pub fn encode(&self) -> Vec<u8> {
        bitcode::serialize(&SessionVersion::V2(self.clone())).unwrap_or_default()
    }

    /// Decode from a versioned bitcode blob, with legacy unversioned fallback.
    /// Returns `None` if the blob is empty or invalid.
    pub fn decode(blob: &[u8]) -> Option<Self> {
        if blob.is_empty() {
            return None;
        }

        if let Ok(versioned) = bitcode::deserialize::<SessionVersion>(blob) {
            return Some(versioned.into_current());
        }

        // Backward compatibility for unversioned blobs produced before #7.
        // Try the richer short-lived V2 shape first so #9 `recent_chars` data
        // is preserved if present, then fall back to the older V1 shape.
        if let Ok(current) = bitcode::deserialize::<AnsiToolSessionState>(blob) {
            return Some(current);
        }

        bitcode::deserialize::<AnsiToolSessionStateV1>(blob).ok().map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_v2() -> AnsiToolSessionState {
        AnsiToolSessionState {
            selected_tool: Tool::Pencil,
            brush: BrushSessionState {
                primary: BrushPrimaryMode::Char,
                paint_char: '█',
                brush_size: 3,
                colorize_fg: true,
                colorize_bg: false,
                exact: true,
            },
            shape: ShapeSessionState { shape: Tool::EllipseFilled },
            selection_mode: SelectionMode::Character,
            font_slot: 1,
            recent_chars: vec!['█', '▓', '▒'],
        }
    }

    fn sample_v1() -> AnsiToolSessionStateV1 {
        AnsiToolSessionStateV1 {
            selected_tool: Tool::Fill,
            brush: BrushSessionState {
                primary: BrushPrimaryMode::Char,
                paint_char: '▓',
                brush_size: 2,
                colorize_fg: false,
                colorize_bg: true,
                exact: false,
            },
            shape: ShapeSessionState { shape: Tool::RectangleFilled },
            selection_mode: SelectionMode::Foreground,
            font_slot: 2,
        }
    }

    fn assert_same_core_fields(actual: &AnsiToolSessionState, expected: &AnsiToolSessionState) {
        assert_eq!(actual.selected_tool, expected.selected_tool);
        assert_eq!(actual.brush.primary, expected.brush.primary);
        assert_eq!(actual.brush.paint_char, expected.brush.paint_char);
        assert_eq!(actual.brush.brush_size, expected.brush.brush_size);
        assert_eq!(actual.brush.colorize_fg, expected.brush.colorize_fg);
        assert_eq!(actual.brush.colorize_bg, expected.brush.colorize_bg);
        assert_eq!(actual.brush.exact, expected.brush.exact);
        assert_eq!(actual.shape.shape, expected.shape.shape);
        assert_eq!(actual.selection_mode, expected.selection_mode);
        assert_eq!(actual.font_slot, expected.font_slot);
    }

    #[test]
    fn encode_writes_explicit_v2_envelope() {
        let state = sample_v2();
        let blob = state.encode();

        let versioned = bitcode::deserialize::<SessionVersion>(&blob).expect("expected versioned envelope");
        match versioned {
            SessionVersion::V2(decoded) => {
                assert_same_core_fields(&decoded, &state);
                assert_eq!(decoded.recent_chars, state.recent_chars);
            }
            SessionVersion::V1(_) => panic!("new encoder must not write V1"),
        }
    }

    #[test]
    fn decode_current_v2_round_trips_recent_chars() {
        let state = sample_v2();
        let decoded = AnsiToolSessionState::decode(&state.encode()).expect("decode current v2");

        assert_same_core_fields(&decoded, &state);
        assert_eq!(decoded.recent_chars, vec!['█', '▓', '▒']);
    }

    #[test]
    fn decode_explicit_v1_migrates_to_current() {
        let legacy = sample_v1();
        let blob = bitcode::serialize(&SessionVersion::V1(legacy.clone())).expect("serialize explicit v1");

        let decoded = AnsiToolSessionState::decode(&blob).expect("decode migrated v1");

        assert_eq!(decoded.selected_tool, legacy.selected_tool);
        assert_eq!(decoded.brush.paint_char, '▓');
        assert_eq!(decoded.shape.shape, legacy.shape.shape);
        assert_eq!(decoded.selection_mode, legacy.selection_mode);
        assert_eq!(decoded.font_slot, legacy.font_slot);
        assert_eq!(decoded.recent_chars, vec!['▓']);
    }

    #[test]
    fn decode_unversioned_v2_preserves_recent_chars() {
        let state = sample_v2();
        let blob = bitcode::serialize(&state).expect("serialize legacy bare v2");

        let decoded = AnsiToolSessionState::decode(&blob).expect("decode legacy bare v2");

        assert_same_core_fields(&decoded, &state);
        assert_eq!(decoded.recent_chars, state.recent_chars);
    }

    #[test]
    fn decode_unversioned_v1_migrates_to_current() {
        let legacy = sample_v1();
        let blob = bitcode::serialize(&legacy).expect("serialize legacy bare v1");

        let decoded = AnsiToolSessionState::decode(&blob).expect("decode legacy bare v1");

        assert_eq!(decoded.selected_tool, legacy.selected_tool);
        assert_eq!(decoded.brush.paint_char, legacy.brush.paint_char);
        assert_eq!(decoded.shape.shape, legacy.shape.shape);
        assert_eq!(decoded.selection_mode, legacy.selection_mode);
        assert_eq!(decoded.font_slot, legacy.font_slot);
        assert_eq!(decoded.recent_chars, vec![legacy.brush.paint_char]);
    }

    #[test]
    fn decode_empty_or_invalid_blob_returns_none() {
        assert!(AnsiToolSessionState::decode(&[]).is_none());
        assert!(AnsiToolSessionState::decode(&[0xFF, 0x00, 0xAA, 0x55]).is_none());
    }
}
