//! Tool Handler System
//!
//! This module provides a trait-based tool system where each tool handles
//! its own events, UI rendering (toolbar, options, status), and canvas overlay.
//!
//! # Architecture
//!
//! - `ToolHandler`: Main trait that tools implement
//! - Tools receive raw `icy_ui::Event` (keyboard/window) and `TerminalMessage` (mouse)
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
mod outline_click;
pub mod paint;
mod paste;
mod pencil;
mod pipette;
mod select;
mod shape;
mod tag;

pub use click::ClickTool;
pub use fill::{FillSettings, FillTool};
pub use font::FontTool;
pub use outline_click::OutlineClickTool;
pub use paint::BrushSettings;
pub use paste::{PasteAction, PasteTool};
pub use pencil::PencilTool;
pub use pipette::PipetteTool;
pub use select::SelectTool;
pub use shape::ShapeTool;
pub use tag::{TagTool, TagToolState};

use icy_engine::Position;
use icy_engine::{BitFont, Palette, TextPane};
use icy_engine_edit::tools::Tool;
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_edit::EditState;
use icy_engine_gui::TerminalMessage;
use icy_ui::widget::{column, text};
use icy_ui::Element;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::ui::editor::ansi::widget::toolbar::top::SelectedTagInfo;
use crate::ui::editor::ansi::widget::toolbar::top::{BrushPrimaryMode, SelectionMode};
use crate::ui::editor::ansi::FKeyToolbarMessage;
use crate::ui::FKeySets;
use crate::Settings;

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
    /// Request lightweight selection rect update only (no mask regeneration)
    /// Use this during drag operations for better performance
    RedrawSelectionRect,
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
    SetCursorIcon(Option<icy_ui::mouse::Interaction>),
    /// Request a UI action owned by the editor (open dialogs/popups, etc.)
    Ui(UiAction),
    /// Multiple results (processed in order)
    Multi(Vec<ToolResult>),
    /// Send paste-as-selection to collaboration (floating layer blocks)
    CollabPasteAsSelection,
    /// Send operation position update to collaboration (x, y)
    CollabOperation(i32, i32),
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

    /// Returns true if this result contains a Redraw, StartCapture, or anything
    /// that indicates the overlay should be updated.
    pub fn needs_redraw(&self) -> bool {
        match self {
            ToolResult::Redraw | ToolResult::RedrawSelectionRect | ToolResult::StartCapture | ToolResult::EndCapture | ToolResult::Commit(_) => true,
            ToolResult::Multi(results) => results.iter().any(|r| r.needs_redraw()),
            _ => false,
        }
    }

    /// Returns true if this result requires a full selection mask update.
    /// RedrawSelectionRect only updates the rect, not the mask.
    pub fn needs_selection_mask_update(&self) -> bool {
        match self {
            ToolResult::Redraw | ToolResult::Commit(_) => true,
            ToolResult::Multi(results) => results.iter().any(|r| r.needs_selection_mask_update()),
            _ => false,
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
    pub theme: icy_ui::Theme,
    pub fkeys: FKeySets,
    pub font: Option<BitFont>,
    pub palette: Palette,
    pub caret_fg: u32,
    pub caret_bg: u32,

    // Tag toolbar info (computed by editor, rendered by TagTool)
    pub tag_add_mode: bool,
    pub selected_tag: Option<SelectedTagInfo>,
    pub tag_selection_count: usize,

    // Paste mode: whether the current layer is an image layer
    pub is_image_layer: bool,
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
    pub options: Option<&'a Arc<RwLock<Settings>>>,
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

// ============================================================================
// Shared Navigation/Selection Helpers
// ============================================================================

/// Result of handling a navigation key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavResult {
    /// Event was not handled
    NotHandled,
    /// Event was handled, redraw needed
    Redraw,
    /// Event was handled, commit with message
    Commit(&'static str),
}

fn clamp_caret_to_current_layer(ctx: &mut ToolContext) {
    let Some(layer) = ctx.state.get_cur_layer() else {
        return;
    };

    let size = layer.size();
    let max_x = (size.width.saturating_sub(1)).max(0);
    let max_y = (size.height.saturating_sub(1)).max(0);

    let pos = ctx.state.get_caret().position();
    let x = pos.x.clamp(0, max_x);
    let y = pos.y.clamp(0, max_y);

    if x != pos.x || y != pos.y {
        ctx.state.set_caret_position(Position::new(x, y));
    }
}

fn current_layer_max_pos(ctx: &mut ToolContext) -> Option<Position> {
    let layer = ctx.state.get_cur_layer()?;
    let size = layer.size();
    let max_x = (size.width.saturating_sub(1)).max(0);
    let max_y = (size.height.saturating_sub(1)).max(0);
    Some(Position::new(max_x, max_y))
}

impl NavResult {
    /// Convert to ToolResult
    pub fn to_tool_result(self) -> ToolResult {
        match self {
            NavResult::NotHandled => ToolResult::None,
            NavResult::Redraw => ToolResult::Redraw,
            NavResult::Commit(msg) => ToolResult::Commit(msg.to_string()),
        }
    }

    /// Check if the event was handled
    pub fn is_handled(self) -> bool {
        !matches!(self, NavResult::NotHandled)
    }
}

/// Handle common navigation and selection keyboard events.
///
/// This covers:
/// - Arrow keys (with Shift for selection extension)
/// - Home/End (with Shift for selection)
/// - PageUp/PageDown (with Shift for selection)
/// - Delete key
/// - Tab/Shift+Tab
/// - Insert (toggle insert mode)
///
/// Does NOT handle: Backspace, Enter, Space, character input (tool-specific).
pub fn handle_navigation_key(ctx: &mut ToolContext, key: &icy_ui::keyboard::Key, modifiers: &icy_ui::keyboard::Modifiers) -> NavResult {
    use icy_ui::keyboard::key::Named;

    let icy_ui::keyboard::Key::Named(named) = key else {
        return NavResult::NotHandled;
    };

    let result = match named {
        Named::ArrowUp => {
            if modifiers.shift() {
                ctx.state.extend_selection(0, -1);
            } else {
                ctx.state.move_caret_up(1);
            }
            NavResult::Redraw
        }
        Named::ArrowDown => {
            if modifiers.shift() {
                ctx.state.extend_selection(0, 1);
            } else {
                ctx.state.move_caret_down(1);
            }
            NavResult::Redraw
        }
        Named::ArrowLeft => {
            if modifiers.shift() {
                ctx.state.extend_selection(-1, 0);
            } else {
                ctx.state.move_caret_left(1);
            }
            NavResult::Redraw
        }
        Named::ArrowRight => {
            if modifiers.shift() {
                ctx.state.extend_selection(1, 0);
            } else {
                ctx.state.move_caret_right(1);
            }
            NavResult::Redraw
        }
        Named::Home => {
            if modifiers.control() {
                let cur = ctx.state.get_caret().position();
                let target = Position::new(0, 0);
                if modifiers.shift() {
                    ctx.state.extend_selection(target.x - cur.x, target.y - cur.y);
                } else {
                    ctx.state.set_caret_position(target);
                }
            } else if modifiers.shift() {
                let cur_x = ctx.state.get_caret().x;
                ctx.state.extend_selection(-cur_x, 0);
            } else {
                ctx.state.set_caret_x(0);
            }
            NavResult::Redraw
        }
        Named::End => {
            if modifiers.control() {
                let cur = ctx.state.get_caret().position();
                let target = current_layer_max_pos(ctx).unwrap_or(cur);
                if modifiers.shift() {
                    ctx.state.extend_selection(target.x - cur.x, target.y - cur.y);
                } else {
                    ctx.state.set_caret_position(target);
                }
            } else {
                let width = ctx.state.get_buffer().width();
                if modifiers.shift() {
                    let cur_x = ctx.state.get_caret().x;
                    let dx = width.saturating_sub(1) - cur_x;
                    ctx.state.extend_selection(dx, 0);
                } else {
                    ctx.state.set_caret_x(width.saturating_sub(1));
                }
            }
            NavResult::Redraw
        }
        Named::PageUp => {
            if modifiers.shift() {
                ctx.state.extend_selection(0, -24);
            } else {
                ctx.state.move_caret_up(24);
            }
            NavResult::Redraw
        }
        Named::PageDown => {
            if modifiers.shift() {
                ctx.state.extend_selection(0, 24);
            } else {
                ctx.state.move_caret_down(24);
            }
            NavResult::Redraw
        }
        Named::Delete => {
            let _ = if ctx.state.is_something_selected() {
                ctx.state.erase_selection()
            } else {
                ctx.state.delete_key()
            };
            NavResult::Commit("Delete")
        }
        Named::Tab => {
            if modifiers.shift() {
                ctx.state.handle_reverse_tab();
            } else {
                ctx.state.handle_tab();
            }
            NavResult::Redraw
        }
        Named::Insert => {
            ctx.state.toggle_insert_mode();
            NavResult::Redraw
        }
        _ => NavResult::NotHandled,
    };

    if result.is_handled() {
        clamp_caret_to_current_layer(ctx);
    }

    result
}

// ============================================================================
// Shared Selection Mouse State
// ============================================================================

use crate::ui::editor::ansi::selection_drag::{compute_dragged_selection, hit_test_selection, DragParameters, SelectionDrag};
use icy_engine::Selection;

/// Shared state for mouse-based selection handling.
///
/// Used by Click tool and Font tool for consistent selection behavior.
#[derive(Default)]
pub struct SelectionMouseState {
    /// Current selection drag mode
    pub selection_drag: SelectionDrag,
    /// Hover drag mode (for cursor icon)
    pub hover_drag: SelectionDrag,
    /// Start position of selection drag
    pub selection_start_pos: Option<Position>,
    /// Current position during selection drag
    pub selection_cur_pos: Option<Position>,
    /// Selection rectangle at start of drag
    pub selection_start_rect: Option<icy_engine::Rectangle>,
    /// Atomic undo guard for selection operations
    pub selection_undo: Option<AtomicUndoGuard>,
}

impl SelectionMouseState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all selection drag state.
    pub fn cancel(&mut self) {
        self.selection_drag = SelectionDrag::None;
        self.hover_drag = SelectionDrag::None;
        self.selection_start_pos = None;
        self.selection_cur_pos = None;
        self.selection_start_rect = None;
        self.selection_undo = None;
    }

    /// Handle mouse move for hover cursor updates.
    pub fn handle_move(&mut self, selection: Option<Selection>, pos: Position) {
        self.hover_drag = hit_test_selection(selection, pos);
    }

    /// Handle mouse press to start selection drag.
    ///
    /// Returns `true` if a selection operation was started.
    pub fn handle_press(&mut self, ctx: &mut ToolContext, pos: Position) -> bool {
        let current_selection = ctx.state.selection();
        let hit = hit_test_selection(current_selection, pos);

        if hit != SelectionDrag::None {
            // Start move/resize of existing selection
            self.selection_drag = hit;
            self.selection_start_rect = current_selection.map(|s| s.as_rectangle());
        } else {
            // Start new selection + position caret
            let _ = ctx.state.clear_selection();
            ctx.state.set_caret_from_document_position(pos);
            self.selection_drag = SelectionDrag::Create;
            self.selection_start_rect = None;
        }

        self.selection_start_pos = Some(pos);
        self.selection_cur_pos = Some(pos);
        self.hover_drag = SelectionDrag::None;
        self.selection_undo = Some(ctx.state.begin_atomic_undo("Selection"));

        true
    }

    /// Handle mouse drag to update selection.
    ///
    /// Returns `true` if the selection was updated.
    pub fn handle_drag(&mut self, ctx: &mut ToolContext, pos: Position) -> bool {
        if self.selection_drag == SelectionDrag::None {
            return false;
        }

        self.selection_cur_pos = Some(pos);

        let Some(start_pos) = self.selection_start_pos else {
            return false;
        };

        if self.selection_drag == SelectionDrag::Create {
            let selection = Selection {
                anchor: start_pos,
                lead: pos,
                locked: false,
                shape: icy_engine::Shape::Rectangle,
                add_type: icy_engine::AddType::Default,
            };
            let _ = ctx.state.set_selection(selection);
            return true;
        }

        let Some(start_rect) = self.selection_start_rect else {
            return false;
        };

        let params = DragParameters {
            start_rect,
            start_pos,
            cur_pos: pos,
        };

        if let Some(new_rect) = compute_dragged_selection(self.selection_drag, params) {
            let mut selection = Selection::from(new_rect);
            selection.add_type = icy_engine::AddType::Default;
            let _ = ctx.state.set_selection(selection);
            return true;
        }

        false
    }

    /// Handle mouse release to finalize selection.
    ///
    /// Returns `true` if a selection was finalized or cleared.
    pub fn handle_release(&mut self, ctx: &mut ToolContext, end_pos: Option<Position>) -> bool {
        if self.selection_drag == SelectionDrag::None {
            return false;
        }

        // For Create mode: click without drag clears selection
        if self.selection_drag == SelectionDrag::Create {
            if let (Some(start), Some(end)) = (self.selection_start_pos, end_pos) {
                if start == end {
                    let _ = ctx.state.clear_selection();
                }
            }
        }

        self.cancel();
        true
    }

    /// Get the cursor interaction for the current state.
    pub fn cursor(&self) -> Option<icy_ui::mouse::Interaction> {
        if self.selection_drag != SelectionDrag::None {
            self.selection_drag.to_cursor_interaction()
        } else if self.hover_drag != SelectionDrag::None {
            self.hover_drag.to_cursor_interaction()
        } else {
            None
        }
    }

    /// Check if a selection drag is active.
    pub fn is_dragging(&self) -> bool {
        self.selection_drag != SelectionDrag::None
    }
}

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
    fn handle_event(&mut self, _ctx: &mut ToolContext, _event: &icy_ui::Event) -> ToolResult {
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

    /// Cancel any active mouse capture/drag operation.
    ///
    /// Called when the mouse is released outside the terminal widget, window loses focus,
    /// or cursor leaves the window. Tools should reset their drag state without committing changes.
    fn cancel_capture(&mut self) {
        // Default: no-op
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

    // === Appearance ===

    /// Get cursor style for this tool.
    ///
    /// Default: Crosshair.
    fn cursor(&self) -> icy_ui::mouse::Interaction {
        icy_ui::mouse::Interaction::Crosshair
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

    /// Check if this tool handler also handles the given tool variant.
    ///
    /// This is used to avoid registry swaps when switching between variants
    /// of the same handler (e.g., switching from Line to Rectangle within ShapeTool).
    /// Default: false (most tools handle only one variant).
    fn is_same_handler(&self, _other: Tool) -> bool {
        false
    }
}
