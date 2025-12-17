//! Tool Handler System
//!
//! This module provides a trait-based tool system where each tool handles
//! its own events, UI rendering (toolbar, options, status), and canvas overlay.
//!
//! # Architecture
//!
//! - `ToolHandler`: Main trait that tools implement
//! - `ToolInput`: All input events (mouse, keyboard, lifecycle)
//! - `ToolResult`: Results from tool operations (redraw, commit, switch tool, etc.)
//! - `ToolMessage`: Centralized enum for all tool-specific UI messages
//! - `ToolContext`: Mutable context passed to tools (EditState, resources, etc.)
//!
//! # Example
//!
//! ```ignore
//! impl ToolHandler for PipetteTool {
//!     fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
//!         match event {
//!             ToolInput::MouseDown { pos, button, .. } => {
//!                 if let Some(ch) = ctx.state.get_char(pos) {
//!                     ctx.state.caret.set_foreground(ch.attribute.foreground());
//!                 }
//!                 ToolResult::SwitchTool(Tool::Click)
//!             }
//!             _ => ToolResult::None,
//!         }
//!     }
//! }
//! ```

mod click;
mod fill;
mod font;
mod line;
mod pencil;
mod pipette;
mod select;
mod shape;
mod tag;

pub use click::ClickTool;
pub use fill::{FillMode, FillTool};
pub use font::FontTool;
pub use line::LineTool;
pub use pencil::PencilTool;
pub use pipette::PipetteTool;
pub use select::{SelectDragMode, SelectModifier, SelectTool};
pub use shape::{ShapeTool, ShapeType};
pub use tag::TagTool;

use std::sync::Arc;

use iced::Element;
use iced::widget::{column, text};
use icy_engine::{AttributedChar, KeyModifiers, Position, Rectangle};
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_edit::EditState;
use icy_engine_edit::tools::Tool;
use parking_lot::RwLock;

use crate::SharedFontLibrary;
use crate::ui::Options;

// ============================================================================
// Tool Input Events
// ============================================================================

/// All input events a tool can receive
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ToolInput {
    // === Mouse Events ===
    /// Mouse button pressed
    MouseDown {
        /// Position in buffer coordinates
        pos: Position,
        /// Position including scroll offset
        pos_abs: Position,
        /// Position in half-block coordinates (2x Y resolution)
        pos_half_block: Position,
        /// Which button was pressed
        button: icy_engine::MouseButton,
        /// Keyboard modifiers at time of event
        modifiers: KeyModifiers,
    },
    /// Mouse moved (during drag or hover)
    MouseMove {
        /// Position in buffer coordinates
        pos: Position,
        /// Position including scroll offset
        pos_abs: Position,
        /// Position in half-block coordinates
        pos_half_block: Position,
        /// Whether a drag is in progress
        is_dragging: bool,
        /// Keyboard modifiers
        modifiers: KeyModifiers,
    },
    /// Mouse button released
    MouseUp {
        /// Position in buffer coordinates
        pos: Position,
        /// Position including scroll offset
        pos_abs: Position,
        /// Which button was released
        button: icy_engine::MouseButton,
    },

    // === Keyboard Events ===
    /// Key pressed
    KeyDown {
        /// The key that was pressed
        key: iced::keyboard::Key,
        /// Keyboard modifiers
        modifiers: KeyModifiers,
    },
    /// Key released
    KeyUp {
        /// The key that was released
        key: iced::keyboard::Key,
    },

    // === Lifecycle Events ===
    /// Tool became active
    Activate,
    /// Tool is being deactivated (switching to another tool)
    Deactivate,

    // === UI Message ===
    /// Tool-specific message from UI (toolbar, options panel, etc.)
    Message(ToolMessage),
}

// ============================================================================
// Tool Result
// ============================================================================

/// Result of a tool operation
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
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
    /// Switch to another tool
    SwitchTool(Tool),
    /// Start mouse capture (all mouse events go to this tool until release)
    StartCapture,
    /// End mouse capture
    EndCapture,
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
// Tool Message (Centralized Enum)
// ============================================================================

/// Centralized enum for all tool-specific UI messages.
///
/// This allows type-safe message passing while keeping the trait object-safe.
/// Each tool handles the messages relevant to it and ignores the rest.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ToolMessage {
    // === Shared Brush Settings (Pencil, Line, Shape tools) ===
    /// Set the brush character
    SetBrushChar(char),
    /// Set brush size (1-5)
    SetBrushSize(u8),
    /// Toggle foreground color usage
    ToggleForeground(bool),
    /// Toggle background color usage
    ToggleBackground(bool),
    /// Set color mode (FG only, BG only, both)
    SetColorMode(ColorMode),

    // === Shape Tools ===
    /// Toggle filled vs outline mode
    ToggleFilled(bool),

    // === Font Tool ===
    /// Select font slot (0-9)
    FontSelectSlot(usize),
    /// Open font selector dialog
    FontOpenSelector,
    /// Set outline style
    FontSetOutline(usize),
    /// Open outline selector popup
    FontOpenOutlineSelector,

    // === Tag Tool ===
    /// Edit a tag
    TagEdit(usize),
    /// Delete a tag
    TagDelete(usize),
    /// Clone a tag
    TagClone(usize),
    /// Open tag list dialog
    TagOpenList,
    /// Start adding a new tag
    TagStartAdd,
    /// Delete selected tags
    TagDeleteSelected,

    // === Select Tool ===
    /// Select all
    SelectAll,
    /// Deselect
    SelectNone,
    /// Invert selection
    SelectInvert,

    // === Pipette Tool ===
    /// Take foreground color
    PipetteTakeForeground(bool),
    /// Take background color
    PipetteTakeBackground(bool),
    /// Take character
    PipetteTakeChar(bool),
}

/// Color mode for brush tools
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ColorMode {
    #[default]
    Both,
    ForegroundOnly,
    BackgroundOnly,
}

// ============================================================================
// Tool Overlay
// ============================================================================

/// Overlay data for preview rendering on the canvas.
///
/// Tools can return this to show previews (shape outlines, selection rects, etc.)
/// without modifying the actual buffer.
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct ToolOverlay {
    /// Characters to render as overlay (position -> char)
    pub chars: Vec<(Position, AttributedChar)>,
    /// Optional selection rectangle to show
    pub selection_rect: Option<Rectangle>,
    /// Optional shape preview points (for line/rect/ellipse tools)
    pub shape_preview: Option<ShapePreview>,
}

/// Shape preview for line/rect/ellipse tools
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ShapePreview {
    /// Line from start to end (half-block coordinates)
    Line { start: Position, end: Position },
    /// Rectangle outline
    Rectangle { rect: Rectangle },
    /// Filled rectangle
    RectangleFilled { rect: Rectangle },
    /// Ellipse outline
    Ellipse { rect: Rectangle },
    /// Filled ellipse
    EllipseFilled { rect: Rectangle },
}

// ============================================================================
// Tool Context
// ============================================================================

/// Shared resources that tools might need
#[allow(dead_code)]
pub struct ToolResources {
    /// Shared font library for TDF/Figlet fonts
    pub font_library: SharedFontLibrary,
    /// Application options
    pub options: Arc<RwLock<Options>>,
}

/// Drag position state for tracking mouse drags
#[derive(Clone, Debug, Default)]
pub struct DragState {
    /// Start position of drag (buffer coordinates)
    pub start: Position,
    /// Current position of drag (buffer coordinates)
    pub cur: Position,
    /// Start position including layer offset (absolute)
    pub start_abs: Position,
    /// Current position including layer offset (absolute)
    pub cur_abs: Position,
    /// Start position in half-block coordinates
    pub start_half_block: Position,
    /// Current position in half-block coordinates
    pub cur_half_block: Position,
}

/// Context passed to tool handlers.
///
/// Contains mutable references to all state a tool might need.
#[allow(dead_code)]
pub struct ToolContext<'a> {
    /// The edit state (buffer, caret, selection, undo stack, etc.)
    pub state: &'a mut EditState,
    /// Atomic undo guard for multi-step operations
    /// Set by tool during MouseDown, cleared on MouseUp/Commit
    pub undo_guard: &'a mut Option<AtomicUndoGuard>,
    /// Shared resources (fonts, options, etc.)
    pub resources: &'a mut ToolResources,
    /// Whether a drag is currently in progress
    pub is_dragging: bool,
    /// Current drag positions (start, current, half-block variants)
    pub drag_pos: DragState,
}

// ============================================================================
// Tool Handler Trait
// ============================================================================

/// Trait for tool-specific behavior.
///
/// Each tool implements this trait. The editor dispatches events to the active
/// tool's `handle_event` method and renders the tool's UI components.
#[allow(dead_code)]
pub trait ToolHandler: Send + Sync {
    // === Event Handling ===

    /// Handle any input event.
    ///
    /// This is the main entry point for tool logic. The tool receives all
    /// mouse, keyboard, lifecycle, and UI message events here.
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult;

    // === UI Rendering ===

    /// Render tool-specific toolbar options (top bar).
    ///
    /// Returns an Element that sends `ToolMessage` when interacted with.
    /// Default: empty row.
    fn view_toolbar<'a>(&'a self, ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let _ = ctx;
        column![].into()
    }

    /// Render tool-specific sidebar options (left panel, under tool icons).
    ///
    /// Default: empty column.
    fn view_options<'a>(&'a self, ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let _ = ctx;
        column![].into()
    }

    /// Render status bar content for this tool.
    ///
    /// Default: empty text.
    fn view_status<'a>(&'a self, ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let _ = ctx;
        text("").into()
    }

    // === Canvas Overlay ===

    /// Get overlay to render on canvas (preview, guides, selection, etc.).
    ///
    /// Called during canvas rendering. Return None for no overlay.
    fn get_overlay(&self) -> Option<ToolOverlay> {
        None
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

// ============================================================================
// Tool Registry
// ============================================================================

/// Create a new tool handler for the given tool type
#[allow(dead_code)]
pub fn create_tool_handler(tool: Tool) -> Box<dyn ToolHandler> {
    match tool {
        Tool::Pipette => Box::new(PipetteTool::new()),
        // TODO: Implement other tools
        _ => Box::new(DefaultTool),
    }
}

/// Default/fallback tool that does nothing.
/// Used as placeholder until tools are implemented.
#[allow(dead_code)]
struct DefaultTool;

impl ToolHandler for DefaultTool {
    fn handle_event(&mut self, _ctx: &mut ToolContext, _event: ToolInput) -> ToolResult {
        ToolResult::None
    }
}
