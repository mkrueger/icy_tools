//! Tool Handler System
//!
//! This module provides a trait-based tool system where each tool handles
//! its own events, UI rendering (toolbar, options, status), and canvas overlay.
//!
//! # Architecture
//!
//! - `ToolHandler`: Main trait that tools implement
//! - Tools receive raw `iced::Event` (keyboard/window) and `TerminalMessage` (mouse)
//! - `ToolResult`: Results from tool operations (redraw, commit, switch tool, etc.)
//! - `ToolMessage`: Centralized enum for all tool-specific UI messages
//! - `ToolContext`: Mutable context passed to tools (EditState, resources, etc.)
//!
//! # Example
//!
//! ```ignore
//! impl ToolHandler for PipetteTool {
//!     fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
//!         match msg {
//!             TerminalMessage::Press(evt) => {
//!                 if let Some(pos) = evt.text_position {
//!                     let ch = ctx.state.get_buffer().char_at(pos);
//!                     ctx.state.set_caret_foreground(ch.attribute.foreground());
//!                 }
//!                 ToolResult::SwitchTool(ToolId::Tool(Tool::Click))
//!             }
//!             _ => ToolResult::None,
//!         }
//!     }
//! }
//! ```

// NOTE: Some types and methods are reserved for future UI integration
// (tool-specific toolbars, options panels, status text).
#![allow(dead_code)]
#![allow(unused_imports)]

mod click;
mod fill;
mod font;
mod paint;
mod paste;
mod pencil;
mod pipette;
mod select;
mod shape;
mod tag;

pub use click::ClickTool;
pub use fill::{FillSettings, FillTool};
pub use font::FontTool;
pub use paint::BrushSettings;
pub use paste::{PasteAction, PasteTool};
pub use pencil::PencilTool;
pub use pipette::PipetteTool;
pub use select::SelectTool;
pub use shape::ShapeTool;
pub use tag::{TagTool, TagToolState};

use iced::Element;
use iced::widget::{column, text};
use icy_engine::Position;
use icy_engine::{BitFont, Palette};
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_edit::EditState;
use icy_engine_edit::tools::Tool;
use icy_engine_gui::TerminalMessage;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::ui::FKeySets;
use crate::ui::Options;
use crate::ui::editor::ansi::FKeyToolbarMessage;
use crate::ui::editor::ansi::widget::toolbar::top::SelectedTagInfo;
use crate::ui::editor::ansi::widget::toolbar::top::{BrushPrimaryMode, SelectionMode};

#[derive(Clone, Copy, Debug, Default)]
pub struct HalfBlockMapper {
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub viewport_x: f32,
    pub viewport_y: f32,
    pub display_scale: f32,
    pub scan_lines: bool,
    pub font_width: f32,
    pub font_height: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    /// Layer offset in half-block coordinates (Y is doubled).
    pub layer_offset: Position,
}

impl HalfBlockMapper {
    /// Map widget-local pixel position to layer-local half-block coordinates.
    /// Y has 2x resolution (upper/lower half of each cell).
    pub fn pixel_to_layer_half_block(&self, pixel_position: (f32, f32)) -> Position {
        // Convert widget-local to screen coordinates.
        let screen_x = self.bounds_x + pixel_position.0;
        let screen_y = self.bounds_y + pixel_position.1;

        // Convert to widget-local coordinates.
        let local_x = screen_x - self.bounds_x;
        let local_y = screen_y - self.bounds_y;

        // Position relative to viewport.
        let vp_local_x = local_x - self.viewport_x;
        let vp_local_y = local_y - self.viewport_y;

        // Convert from screen pixels to terminal pixels.
        let scale = self.display_scale.max(0.001);
        let term_x = vp_local_x / scale;
        let mut term_y = vp_local_y / scale;

        if self.scan_lines {
            term_y /= 2.0;
        }

        let font_width = self.font_width.max(1.0);
        let font_height = self.font_height.max(1.0);

        // Half-block: divide by half the font height for 2x Y resolution.
        let half_font_height = font_height / 2.0;

        let cell_x = (term_x / font_width).floor() as i32;
        let half_block_y = (term_y / half_font_height).floor() as i32;

        // Viewport scroll offsets (in content pixels).
        let scroll_offset_cols = (self.scroll_x / font_width).floor() as i32;
        let scroll_offset_half_lines = (self.scroll_y / font_height * 2.0).floor() as i32;

        let abs_half_block = Position::new(cell_x + scroll_offset_cols, half_block_y + scroll_offset_half_lines);
        abs_half_block - self.layer_offset
    }
}

// ============================================================================
// Tool Result
// ============================================================================

/// Result of a tool operation
#[derive(Clone, Debug, Default)]

pub enum ToolResult {
    /// No action needed
    #[default]
    None,
    /// Request canvas redraw (e.g., overlay changed)
    Redraw,
    /// Operation completed - commit to undo stack with description
    Commit(String),
    /// Update status bar text
    Status(String),
    /// Request updating layer bounds overlay/UI (e.g. paste mode moving floating layer)
    UpdateLayerBounds,
    /// Switch to another tool
    SwitchTool(ToolId),
    /// Start mouse capture (all mouse events go to this tool until release)
    StartCapture,
    /// End mouse capture
    EndCapture,
    /// Set the mouse cursor icon (UI-only)
    SetCursorIcon(Option<iced::mouse::Interaction>),
    /// Request a UI action owned by the editor (open dialogs/popups, etc.)
    Ui(UiAction),
    /// Multiple results (processed in order)
    Multi(Vec<ToolResult>),
}

impl ToolResult {
    /// Combine with another result
    pub fn and(self, other: ToolResult) -> ToolResult {
        match (self, other) {
            (ToolResult::None, other) => other,
            (this, ToolResult::None) => this,
            (ToolResult::Multi(mut v), ToolResult::Multi(v2)) => {
                v.extend(v2);
                ToolResult::Multi(v)
            }
            (ToolResult::Multi(mut v), other) => {
                v.push(other);
                ToolResult::Multi(v)
            }
            (this, ToolResult::Multi(mut v)) => {
                v.insert(0, this);
                ToolResult::Multi(v)
            }
            (this, other) => ToolResult::Multi(vec![this, other]),
        }
    }
}

// ============================================================================
// Tool Id + UI Actions
// ============================================================================

/// Identifier for the currently active editor tool.
///
/// This extends the engine-level `Tool` with editor-only modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolId {
    Tool(Tool),
    Paste,
}

/// UI actions that must be performed by the editor (outside the tool object).
#[derive(Clone, Debug)]
pub enum UiAction {
    OpenCharSelectorForFKey(usize),
    OpenCharSelectorForBrush,
    OpenTdfFontSelector,
    OpenFontDirectory,
}

// ============================================================================
// Tool Message (Centralized Enum)
// ============================================================================

/// Centralized enum for all tool-specific UI messages.
///
/// This allows type-safe message passing while keeping the trait object-safe.
/// Each tool handles the messages relevant to it and ignores the rest.
#[derive(Clone, Debug)]

pub enum ToolMessage {
    // === Shared Brush Settings (Pencil, Line, Shape tools) ===
    /// Set the primary brush mode (exclusive)
    SetBrushPrimary(BrushPrimaryMode),
    /// Request opening the brush character selector popup
    BrushOpenCharSelector,
    /// Set the brush character
    SetBrushChar(char),
    /// Set brush size (1-5)
    SetBrushSize(u8),
    /// Toggle foreground color usage
    ToggleForeground(bool),
    /// Toggle background color usage
    ToggleBackground(bool),

    // === Shape Tools ===
    /// Toggle filled vs outline mode
    ToggleFilled(bool),

    // === Fill Tool ===
    /// Toggle exact matching
    FillToggleExact(bool),

    // === Font Tool ===
    /// Select font slot (0-9)
    FontSelectSlot(usize),
    /// Open font selector dialog
    FontOpenSelector,
    /// Open the font directory in the system file manager
    FontOpenDirectory,
    /// Set outline style
    FontSetOutline(usize),
    /// Open outline selector popup
    FontOpenOutlineSelector,

    // === Click Tool / F-Key Toolbar ===
    ClickFKeyToolbar(FKeyToolbarMessage),

    // === Tag Tool ===
    /// Edit a tag
    TagEdit(usize),
    /// Delete a tag
    TagDelete(usize),
    /// Clone a tag
    TagClone(usize),
    /// Close the tag context menu overlay
    TagContextMenuClose,
    /// Open tag list dialog
    TagOpenList,
    /// Start adding a new tag
    TagStartAdd,
    /// Edit currently selected tag (editor resolves selection)
    TagEditSelected,
    /// Delete selected tags
    TagDeleteSelected,

    // === Select Tool ===
    /// Set selection mode
    SelectSetMode(SelectionMode),
    /// Select all
    SelectAll,
    /// Deselect
    SelectNone,
    /// Invert selection
    SelectInvert,

    // === Paste Tool (floating layer) ===
    PasteStamp,
    PasteRotate,
    PasteFlipX,
    PasteFlipY,
    PasteToggleTransparent,
    PasteAnchor,
    PasteCancel,

    // === Pipette Tool ===
    /// Take foreground color
    PipetteTakeForeground(bool),
    /// Take background color
    PipetteTakeBackground(bool),
    /// Take character
    PipetteTakeChar(bool),
}

// ============================================================================
// Tool View Context (UI-only)
// ============================================================================

/// Read-only context for tool UI rendering.
///
/// Important: this must not borrow the `EditState` behind a mutex lock.
///
/// This is intentionally **owned** so tool rendering can be object-safe
/// (`Element<'static, _>`).
#[derive(Clone)]
pub struct ToolViewContext {
    pub theme: iced::Theme,
    pub fkeys: FKeySets,
    pub font: Option<BitFont>,
    pub palette: Palette,
    pub caret_fg: u32,
    pub caret_bg: u32,

    // Tag toolbar info (computed by editor, rendered by TagTool)
    pub tag_add_mode: bool,
    pub selected_tag: Option<SelectedTagInfo>,
    pub tag_selection_count: usize,
}

// ============================================================================
// Tool Context
// ============================================================================

/// Context passed to tool handlers.
///
/// Contains mutable references to all state a tool might need.
pub struct ToolContext<'a> {
    /// The edit state (buffer, caret, selection, undo stack, etc.)
    pub state: &'a mut EditState,

    /// Shared UI/editor options (read-mostly, may be updated by some tools)
    pub options: Option<&'a Arc<RwLock<Options>>>,
    /// Atomic undo guard for multi-step operations
    /// Set by tool during MouseDown, cleared on MouseUp/Commit
    pub undo_guard: &'a mut Option<AtomicUndoGuard>,

    /// Optional pixel→half-block mapper (layer-local).
    /// Used by tools that need 2x Y resolution (e.g. HalfBlock fill/paint).
    pub half_block_mapper: Option<HalfBlockMapper>,
}

// ============================================================================
// Tool Handler Trait
// ============================================================================

/// Trait for tool-specific behavior.
///
/// Each tool implements this trait. The editor dispatches events to the active
/// tool's `handle_event` method and renders the tool's UI components.

pub trait ToolHandler: Send + Sync {
    /// Tool identifier (used for routing/editor decisions).
    fn id(&self) -> ToolId;

    /// Downcasting support (used by editor/tool registry glue code).
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    // === Event Handling ===

    /// Handle non-terminal Iced events.
    ///
    /// Intended for keyboard/window lifecycle events.
    fn handle_event(&mut self, _ctx: &mut ToolContext, _event: &iced::Event) -> ToolResult {
        ToolResult::None
    }

    /// Handle terminal widget messages.
    ///
    /// Intended for mouse input from the terminal widget with correct coordinate/modifier state.
    fn handle_terminal_message(&mut self, _ctx: &mut ToolContext, _msg: &TerminalMessage) -> ToolResult {
        ToolResult::None
    }

    /// Handle tool-specific UI messages (toolbars/options/status widgets).
    fn handle_message(&mut self, _ctx: &mut ToolContext, _msg: &ToolMessage) -> ToolResult {
        ToolResult::None
    }

    // === UI Rendering ===

    /// Render tool-specific toolbar options (top bar).
    ///
    /// Returns an Element that sends `ToolMessage` when interacted with.
    /// Default: empty row.
    fn view_toolbar(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let _ = ctx;
        column![].into()
    }

    /// Render tool-specific sidebar options (left panel, under tool icons).
    ///
    /// Default: empty column.
    fn view_options(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let _ = ctx;
        column![].into()
    }

    /// Render status bar content for this tool.
    ///
    /// Default: empty text.
    fn view_status(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let _ = ctx;
        text("").into()
    }

    // === Appearance ===

    /// Get cursor style for this tool.
    ///
    /// Default: Crosshair.
    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    /// Whether the caret should be visible with this tool.
    ///
    /// Default: true (most tools show caret).
    fn show_caret(&self) -> bool {
        true
    }

    /// Whether selection rectangle should be shown.
    ///
    /// Default: true.
    fn show_selection(&self) -> bool {
        true
    }
}
