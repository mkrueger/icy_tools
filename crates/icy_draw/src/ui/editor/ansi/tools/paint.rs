//! Shared paint helpers for tool handlers.

use std::sync::Arc;

use icy_engine::{MouseButton, Palette, Position, TextPane};
use icy_engine_edit::brushes::{BrushMode as EngineBrushMode, ColorMode as EngineColorMode, DrawContext, PointRole};
use icy_engine_edit::{AtomicUndoGuard, AttributedChar, EditState};
use parking_lot::RwLock;

use crate::ui::editor::ansi::widget::toolbar::top::BrushPrimaryMode;

/// Default FG color index when filter is disabled (light gray)
pub const DEFAULT_FG: u32 = 7;
/// Default BG color index when filter is disabled (black)
pub const DEFAULT_BG: u32 = 0;

/// Unified brush settings shared by Pencil / Shape / Fill / TopToolbar.
///
/// Every brush-style tool reads from and writes to a single `SharedBrush`
/// owned by the editor (`AnsiEditorCore`/`CharFontEditor`), so the toolbar UI
/// and the tool views can no longer disagree about brush mode, paint char,
/// brush size, color filters or the Fill-specific exact-match flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrushSettings {
    pub primary: BrushPrimaryMode,
    pub paint_char: char,
    pub brush_size: u32,
    pub colorize_fg: bool,
    pub colorize_bg: bool,
    /// Fill-tool: only fill cells whose character/colors exactly match the
    /// hovered cell. Ignored by Pencil/Shape.
    pub exact: bool,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            primary: BrushPrimaryMode::HalfBlock,
            paint_char: '\u{00B0}', // Light shade block (░)
            brush_size: 1,
            colorize_fg: true,
            colorize_bg: true,
            exact: false,
        }
    }
}

/// Editor-owned brush state shared between the toolbar and every brush tool.
pub type SharedBrush = Arc<RwLock<BrushSettings>>;

/// Construct a fresh shared brush state with default settings.
pub fn new_shared_brush() -> SharedBrush {
    Arc::new(RwLock::new(BrushSettings::default()))
}

/// Maximum number of recently-used paint characters tracked for the cycle
/// keyboard shortcut (#9: "quick keyboard cycle through the last 8 chars").
pub const RECENT_CHARS_CAPACITY: usize = 8;

/// Most-recently-used paint characters, newest first.
///
/// Maintained as a simple LRU `Vec<char>` capped at [`RECENT_CHARS_CAPACITY`]:
/// inserting an existing char promotes it to the front rather than duplicating
/// it, so the list always reflects distinct chars in MRU order.
#[derive(Clone, Debug, Default)]
pub struct RecentChars {
    items: Vec<char>,
}

impl RecentChars {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Snapshot the current MRU list (newest first).
    pub fn as_slice(&self) -> &[char] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Promote `ch` to the front. Removes any existing copy first so the list
    /// stays distinct, then trims to [`RECENT_CHARS_CAPACITY`].
    pub fn push(&mut self, ch: char) {
        self.items.retain(|c| *c != ch);
        self.items.insert(0, ch);
        if self.items.len() > RECENT_CHARS_CAPACITY {
            self.items.truncate(RECENT_CHARS_CAPACITY);
        }
    }

    /// Replace the entire list (used when restoring from a saved session).
    pub fn restore(&mut self, items: Vec<char>) {
        self.items = items;
        if self.items.len() > RECENT_CHARS_CAPACITY {
            self.items.truncate(RECENT_CHARS_CAPACITY);
        }
    }

    /// Cycle relative to `current`. `delta = 1` returns the next-older entry,
    /// `delta = -1` the more-recent one. If `current` is not in the list,
    /// the first (most-recent) entry is returned. Returns `None` only when
    /// the list is empty.
    pub fn cycle_from(&self, current: char, delta: i32) -> Option<char> {
        if self.items.is_empty() {
            return None;
        }
        let len = self.items.len() as i32;
        let idx = self.items.iter().position(|c| *c == current).map(|i| i as i32).unwrap_or(-1);
        let next = ((idx + delta).rem_euclid(len)) as usize;
        Some(self.items[next])
    }
}

/// Editor-owned recent-paint-chars history, shared with the UI strip / hotkeys.
pub type SharedRecentChars = Arc<RwLock<RecentChars>>;

/// Construct a fresh shared recent-chars history.
pub fn new_shared_recent_chars() -> SharedRecentChars {
    Arc::new(RwLock::new(RecentChars::new()))
}

pub fn begin_paint_undo(state: &mut EditState, desc: String) -> AtomicUndoGuard {
    state.begin_atomic_undo(desc)
}

pub fn apply_stamp_at_doc_pos(state: &mut EditState, settings: BrushSettings, doc_pos: Position, half_block_is_top: bool, button: MouseButton) {
    let (offset, layer_w, layer_h) = if let Some(layer) = state.get_cur_layer() {
        (layer.offset(), layer.width(), layer.height())
    } else {
        return;
    };

    let use_selection = state.is_something_selected();

    let swap_colors = button == MouseButton::Right;

    let caret_attr = state.get_caret().attribute;

    // Shapes use Shift as erase/clear, so we don't use Shift-swap here.
    let shift_swap = false;

    let swap_for_colors = (swap_colors || shift_swap) && !matches!(settings.primary, BrushPrimaryMode::Shading | BrushPrimaryMode::Char);
    let (fg, bg) = if swap_for_colors {
        (caret_attr.background(), caret_attr.foreground())
    } else {
        (caret_attr.foreground(), caret_attr.background())
    };

    let effective_brush_size = if matches!(settings.primary, BrushPrimaryMode::HalfBlock) {
        1
    } else {
        settings.brush_size.max(1)
    };
    let brush_size_i = effective_brush_size as i32;

    let center = doc_pos - offset;
    let half = brush_size_i / 2;

    for dy in 0..brush_size_i {
        for dx in 0..brush_size_i {
            let layer_pos = Position::new(center.x + dx - half, center.y + dy - half);

            if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                continue;
            }

            if use_selection {
                let doc_cell = layer_pos + offset;
                if !state.is_selected(doc_cell) {
                    continue;
                }
            }

            let brush_mode = match settings.primary {
                BrushPrimaryMode::Char => {
                    if swap_colors {
                        EngineBrushMode::Char(' ')
                    } else {
                        EngineBrushMode::Char(settings.paint_char)
                    }
                }
                BrushPrimaryMode::HalfBlock => EngineBrushMode::HalfBlock,
                BrushPrimaryMode::Shading => {
                    if swap_colors {
                        EngineBrushMode::ShadeDown
                    } else {
                        EngineBrushMode::Shade
                    }
                }
                BrushPrimaryMode::Replace => EngineBrushMode::Replace(settings.paint_char),
                BrushPrimaryMode::Blink => EngineBrushMode::Blink(!swap_colors),
                BrushPrimaryMode::Colorize => EngineBrushMode::Colorize,
            };

            // FG/BG filter logic (like old icy_draw):
            // - If FG filter is ON: use caret FG, otherwise keep existing cell's FG
            // - If BG filter is ON: use caret BG, otherwise keep existing cell's BG
            // This applies to ALL drawing modes.
            let existing_attr = state.get_cur_layer().map(|l| l.char_at(layer_pos).attribute).unwrap_or(caret_attr);

            let effective_fg = if settings.colorize_fg { fg } else { existing_attr.foreground() };
            let effective_bg = if settings.colorize_bg { bg } else { existing_attr.background() };

            // ColorMode for engine: Both means it will apply both colors from template
            let color_mode = EngineColorMode::Both;

            struct LayerTarget<'a> {
                state: &'a mut EditState,
                width: i32,
                height: i32,
            }
            impl<'a> icy_engine_edit::brushes::DrawTarget for LayerTarget<'a> {
                fn width(&self) -> i32 {
                    self.width
                }
                fn height(&self) -> i32 {
                    self.height
                }
                fn char_at(&self, pos: icy_engine_edit::Position) -> Option<icy_engine_edit::AttributedChar> {
                    self.state.get_cur_layer().map(|l| l.char_at(pos))
                }
                fn set_char(&mut self, pos: icy_engine_edit::Position, ch: icy_engine_edit::AttributedChar) {
                    let _ = self.state.set_char_in_atomic(pos, ch);
                }
            }

            let mut template = caret_attr;
            template.attr &= !icy_engine::attribute::INVISIBLE;
            template.set_foreground(effective_fg);
            template.set_background(effective_bg);

            let ctx = DrawContext::default()
                .with_brush_mode(brush_mode)
                .with_color_mode(color_mode)
                .with_foreground(effective_fg)
                .with_background(effective_bg)
                .with_template_attribute(template)
                .with_half_block_is_top(half_block_is_top);

            let mut target = LayerTarget {
                state,
                width: layer_w,
                height: layer_h,
            };

            ctx.plot_point(&mut target, layer_pos, PointRole::Fill);
        }
    }
}

pub fn clear_at_doc_pos(state: &mut EditState, doc_pos: Position) {
    let (offset, layer_w, layer_h) = if let Some(layer) = state.get_cur_layer() {
        (layer.offset(), layer.width(), layer.height())
    } else {
        return;
    };

    let use_selection = state.is_something_selected();

    let layer_pos = doc_pos - offset;
    if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
        return;
    }

    if use_selection && !state.is_selected(doc_pos) {
        return;
    }

    let _ = state.set_char_in_atomic(layer_pos, AttributedChar::invisible());
}

/// Compute the preview color for overlay rendering based on current brush settings.
/// Returns (r, g, b) tuple for display.
///
/// This ensures that the overlay preview uses the same color logic as actual painting.
pub fn compute_preview_color(settings: &BrushSettings, caret_fg: u32, caret_bg: u32, palette: &Palette, button: MouseButton) -> (u8, u8, u8) {
    let swap_colors = button == MouseButton::Right;

    // For shape preview, use FG color (what will be painted)
    // In modes that swap colors on right-click, show the swapped color
    let swap_for_colors = swap_colors && !matches!(settings.primary, BrushPrimaryMode::Shading | BrushPrimaryMode::Char);

    let (fg_idx, bg_idx) = if swap_for_colors { (caret_bg, caret_fg) } else { (caret_fg, caret_bg) };

    // Preview uses the effective FG color (respecting FG filter)
    // If FG filter is off, we'd use existing cell's color, but for preview we just show caret FG
    // For Colorize with only BG selected, show BG color instead
    let preview_idx = if !settings.colorize_fg && settings.colorize_bg { bg_idx } else { fg_idx };

    palette.rgb(preview_idx)
}

#[cfg(test)]
mod recent_chars_tests {
    use super::{RecentChars, RECENT_CHARS_CAPACITY};

    #[test]
    fn push_promotes_existing_to_front() {
        let mut r = RecentChars::new();
        r.push('a');
        r.push('b');
        r.push('c');
        assert_eq!(r.as_slice(), &['c', 'b', 'a']);
        r.push('a');
        assert_eq!(r.as_slice(), &['a', 'c', 'b']);
    }

    #[test]
    fn push_caps_at_capacity() {
        let mut r = RecentChars::new();
        for ch in ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'] {
            r.push(ch);
        }
        assert_eq!(r.as_slice().len(), RECENT_CHARS_CAPACITY);
        assert_eq!(r.as_slice()[0], 'j');
    }

    #[test]
    fn cycle_steps_forward_and_back() {
        let mut r = RecentChars::new();
        for ch in ['a', 'b', 'c'] {
            r.push(ch);
        }
        // List is [c, b, a]
        assert_eq!(r.cycle_from('c', 1), Some('b'));
        assert_eq!(r.cycle_from('b', 1), Some('a'));
        assert_eq!(r.cycle_from('a', 1), Some('c'));
        assert_eq!(r.cycle_from('c', -1), Some('a'));
    }

    #[test]
    fn cycle_with_unknown_current_returns_first() {
        let mut r = RecentChars::new();
        r.push('a');
        r.push('b');
        // current 'z' is not in list -> should wrap to first (most-recent) on +1.
        assert_eq!(r.cycle_from('z', 1), Some('b'));
    }

    #[test]
    fn cycle_on_empty_returns_none() {
        let r = RecentChars::new();
        assert_eq!(r.cycle_from('a', 1), None);
    }
}
