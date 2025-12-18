//! ANSI Editor Mode
//!
//! This module contains the main ANSI art editor with:
//! - Left sidebar: Color switcher, Palette, Tools
//! - Top toolbar: Tool-specific options
//! - Center: Terminal/Canvas view
//! - Right panel: Minimap, Layers, Channels
//!
//! # Important: Editing Buffer State
//!
//! **Use `icy_engine_edit::EditState` functions for all buffer modifications.**
//! These functions generate proper undo actions.
//!
//! **DO NOT ALTER BUFFER OR SCREEN STATE DIRECTLY!!!**
//!
//! ## Examples:
//! - Font changes: Use `state.set_font()` or `state.set_font_in_slot(slot, font)`
//! - Layer operations: Use `state.add_layer()`, `state.remove_layer()`, etc.
//! - Character changes: Use `state.set_char()`, etc.
//! - Selection operations: Use corresponding EditState methods
//!
//! Direct buffer modifications bypass the undo system and will cause
//! inconsistent state when users try to undo/redo.

pub mod ansi_editor;
pub mod constants;
pub mod dialog;
pub mod main_area;
pub mod selection_drag;
mod shape_points;
pub mod tool_registry;
pub mod tools;
pub mod widget;
pub(crate) use ansi_editor::*;

pub use selection_drag::SelectionDrag;

use dialog::tag::TagDialogMessage;
use dialog::tag_list::TagListDialogMessage;
use icy_engine_gui::TerminalMessage;

pub use dialog::edit_layer::*;
pub use dialog::file_settings::*;
pub use dialog::font_selector::*;
pub use dialog::font_slot_manager::*;
pub use dialog::reference_image::*;
pub use dialog::tdf_font_selector::{TdfFontSelectorDialog, TdfFontSelectorMessage};

pub use main_area::AnsiEditorMainArea;

pub use widget::canvas::*;
pub use widget::char_selector::*;
pub use widget::color_switcher::gpu::*;
pub use widget::fkey_toolbar::gpu::*;
pub use widget::layer_view::*;
pub use widget::minimap::*;
pub use widget::palette_grid::*;
pub use widget::right_panel::*;
pub use widget::toolbar::tool_panel_wrapper::{ToolPanel, ToolPanelMessage};
pub use widget::toolbar::top::*;

/// Target for the character selector popup
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharSelectorTarget {
    /// Editing an F-key slot (0-11)
    FKeySlot(usize),
    /// Editing the brush paint character
    BrushChar,
}

use widget::outline_selector::OutlineSelectorMessage;

/// Core editing messages handled by `AnsiEditorCore`
///
/// These messages deal with buffer operations, tools, and canvas interaction.
/// They don't involve UI dialogs or panel management.
#[derive(Clone, Debug)]
pub enum AnsiEditorCoreMessage {
    // --- Tool/Canvas Interaction ---
    /// Canvas view messages
    Canvas(TerminalMessage),
    /// Top toolbar messages (tool-specific options)
    TopToolbar(TopToolbarMessage),
    /// Tool-owned toolbar/options/status messages
    ToolMessage(tools::ToolMessage),
    /// Char selector popup messages (F-key character selection)
    CharSelector(CharSelectorMessage),
    /// Outline selector popup messages (font tool outline style)
    OutlineSelector(OutlineSelectorMessage),
    /// Tag config dialog messages
    TagDialog(TagDialogMessage),
    /// Open the tag list dialog
    OpenTagListDialog,
    /// Tag list dialog messages
    TagListDialog(TagListDialogMessage),

    // --- Layer Operations ---
    /// Layer selection changed
    SelectLayer(usize),
    /// Toggle layer visibility
    ToggleLayerVisibility(usize),
    /// Add new layer
    AddLayer,
    /// Remove layer
    RemoveLayer(usize),
    /// Move layer up
    MoveLayerUp(usize),
    /// Move layer down
    MoveLayerDown(usize),
    /// Duplicate a layer
    DuplicateLayer(usize),
    /// Merge layer down
    MergeLayerDown(usize),
    /// Clear layer contents
    ClearLayer(usize),
    /// Scroll viewport
    ScrollViewport(f32, f32),

    // --- Marker/Guide Messages ---
    SetGuide(i32, i32),
    ClearGuide,
    SetRaster(i32, i32),
    ClearRaster,
    ToggleGuide,
    ToggleRaster,
    ToggleLineNumbers,
    ToggleLayerBorders,
    ToggleMirrorMode,

    // --- Area Operations ---
    JustifyLineLeft,
    JustifyLineRight,
    JustifyLineCenter,
    InsertRow,
    DeleteRow,
    InsertColumn,
    DeleteColumn,
    EraseRow,
    EraseRowToStart,
    EraseRowToEnd,
    EraseColumn,
    EraseColumnToStart,
    EraseColumnToEnd,
    ScrollAreaUp,
    ScrollAreaDown,
    ScrollAreaLeft,
    ScrollAreaRight,

    // --- Reference Image ---
    ApplyReferenceImage(std::path::PathBuf, f32),
    ClearReferenceImage,
    ToggleReferenceImage,

    // --- Transform Operations ---
    FlipX,
    FlipY,
    Crop,
    JustifyCenter,
    JustifyLeft,
    JustifyRight,

    // --- Color Operations ---
    NextFgColor,
    PrevFgColor,
    NextBgColor,
    PrevBgColor,
    PickAttributeUnderCaret,
    ToggleColor,
    SwitchToDefaultColor,

    // --- Font Apply Operations ---
    ApplyFontSelection(FontSelectorResult),
    ApplyFontSlotChange(FontSlotManagerResult),

    // --- Selection Operations ---
    Deselect,
    DeleteSelection,
}

/// UI messages handled by `AnsiEditorMainArea`
///
/// These messages deal with panels, dialogs, tool switching, and layout.
#[derive(Clone, Debug)]
pub enum AnsiEditorMessage {
    /// Forward a core message to AnsiEditorCore
    Core(AnsiEditorCoreMessage),

    // --- Panel Widgets ---
    /// Tool panel messages
    ToolPanel(ToolPanelMessage),
    /// Right panel messages (minimap, layers, etc.)
    RightPanel(RightPanelMessage),
    /// Color switcher messages
    ColorSwitcher(ColorSwitcherMessage),
    /// Palette grid messages
    PaletteGrid(PaletteGridMessage),

    // --- Tool Switching ---
    SelectTool(usize),
    SwitchTool(tools::ToolId),

    // --- Layer Dialog ---
    EditLayer(usize),
    ShowEditLayerDialog(usize),
    EditLayerDialog(EditLayerDialogMessage),
    ApplyEditLayer(EditLayerResult),

    // --- Palette Dialog ---
    EditPalette,
    PaletteEditorDialog(crate::ui::editor::palette::PaletteEditorMessage),
    PaletteEditorApplied(icy_engine::Palette),

    // --- Reference Image Dialog ---
    ShowReferenceImageDialog,
    ReferenceImageDialog(ReferenceImageDialogMessage),

    // --- Font Dialogs ---
    SwitchFontSlot(usize),
    OpenFontSelector,
    OpenFontSelectorForSlot(usize),
    FontSelector(FontSelectorMessage),
    OpenFontSlotManager,
    FontSlotManager(FontSlotManagerMessage),
    TdfFontSelector(TdfFontSelectorMessage),

    // --- Plugins ---
    RunPlugin(usize),

    // --- Selection with UI ---
    InverseSelection,
    PasteAsNewImage,

    // --- Export ---
    ExportFile,
}

// CanvasMouseEvent removed - use TerminalMessage directly from icy_engine_gui

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseCaptureTarget {
    Tool,
    Paste,
}

/// Status bar information for the ANSI editor
#[derive(Clone, Debug)]
pub struct AnsiStatusInfo {
    pub cursor_position: (i32, i32),
    pub buffer_size: (i32, i32),
    pub current_layer: usize,
    pub total_layers: usize,
    pub current_tool: String,
    pub insert_mode: bool,
    pub font_name: String,
    /// Current format mode
    pub format_mode: icy_engine_edit::FormatMode,
    /// Currently active font slot (0 or 1 for XBinExtended)
    pub current_font_slot: usize,
    /// Font names for slots (only set for XBinExtended)
    pub slot_fonts: Option<[Option<String>; 2]>,
}
