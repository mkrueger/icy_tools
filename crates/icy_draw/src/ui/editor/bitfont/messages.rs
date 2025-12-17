//! Messages for BitFont editor

use iced::{Point, keyboard::Modifiers};
use icy_engine_edit::bitfont::BitFontFocusedPanel;

use super::{BitFontTool, BitFontToolPanelMessage, BitFontTopToolbarMessage};
use crate::ui::editor::ansi::PaletteGridMessage;
use icy_engine_gui::terminal::view::TerminalMessage;

/// Direction for arrow key navigation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Messages for the BitFont editor
#[derive(Clone, Debug)]
pub enum BitFontEditorMessage {
    /// Select a glyph by character code
    SelectGlyph(char),
    /// Select a glyph at a specific grid position (also sets focus and cursor)
    SelectGlyphAt(char, i32, i32),
    /// Set or clear a pixel at (x, y) - true = set, false = clear
    SetPixel(i32, i32, bool),
    /// Clear the selected glyph (or selection)
    Clear,
    /// Inverse the selected glyph (or selection)
    Inverse,
    /// Move glyph up
    MoveUp,
    /// Move glyph down
    MoveDown,
    /// Move glyph left
    MoveLeft,
    /// Move glyph right
    MoveRight,
    /// Flip glyph horizontally
    FlipX,
    /// Flip glyph vertically
    FlipY,
    /// Set new font width
    SetWidth(i32),
    /// Set new font height
    SetHeight(i32),
    /// Apply resize
    ApplyResize,
    /// Undo last operation
    Undo,
    /// Redo last undone operation
    Redo,
    /// Canvas interaction
    CanvasEvent(CanvasEvent),
    /// Terminal message while preview is open
    PreviewTerminal(TerminalMessage),

    // ═══════════════════════════════════════════════════════════════════════
    // Tool & Cursor messages
    // ═══════════════════════════════════════════════════════════════════════
    /// Select a tool
    SelectTool(BitFontTool),
    /// Toggle rectangle fill mode
    ToggleRectFilled,
    /// Move cursor by delta
    MoveCursor(i32, i32),
    /// Toggle pixel at cursor position
    TogglePixelAtCursor,
    /// Set pixel at cursor (true = on, false = off)
    SetPixelAtCursor(bool),
    /// Extend selection with shift+arrows (edit grid)
    ExtendSelection(i32, i32),
    /// Extend charset selection with shift+arrows (anchor/lead mode)
    /// Second bool is is_rectangle: true = Alt held (rectangle mode)
    ExtendCharsetSelection(i32, i32, bool),
    /// Set charset selection lead position directly (for mouse drag, anchor/lead mode)
    /// bool is is_rectangle: true = Alt held (rectangle mode)
    SetCharsetSelectionLead(i32, i32, bool),
    /// Clear current edit selection
    ClearSelection,
    /// Clear charset selection
    ClearCharsetSelection,
    /// Select all pixels in glyph
    SelectAll,
    /// Fill selection with pixels
    FillSelection,
    /// Erase selection (clear pixels)
    EraseSelection,
    /// Inverse selection pixels
    InverseSelection,
    /// Go to next character (+)
    NextChar,
    /// Go to previous character (-)
    PrevChar,
    /// Toggle 8/9-dot cell mode (letter spacing)
    ToggleLetterSpacing,
    /// Insert a line at cursor position (shifts all glyphs down, increases height)
    InsertLine,
    /// Delete line at cursor position (shifts all glyphs up, decreases height)
    DeleteLine,
    /// Insert a column at cursor position (shifts all glyphs right, increases width)
    InsertColumn,
    /// Delete column at cursor position (shifts all glyphs left, decreases width)
    DeleteColumn,
    /// Duplicate line at cursor position (copies current line, increases height)
    DuplicateLine,
    /// Swap the selected char with the char at charset cursor
    SwapChars,
    /// Slide pixels up (rotate vertically, Ctrl+Up)
    SlideUp,
    /// Slide pixels down (rotate vertically, Ctrl+Down)
    SlideDown,
    /// Slide pixels left (rotate horizontally, Ctrl+Left)
    SlideLeft,
    /// Slide pixels right (rotate horizontally, Ctrl+Right)
    SlideRight,
    /// Switch focus to next panel (Tab)
    FocusNextPanel,
    /// Set focus to a specific panel
    SetFocusedPanel(BitFontFocusedPanel),
    /// Move charset cursor by delta
    MoveCharsetCursor(i32, i32),
    /// Set charset cursor to absolute position
    SetCharsetCursor(i32, i32),
    /// Select character at charset cursor (Space/Enter)
    SelectCharAtCursor,
    /// Show font preview screen
    ShowPreview,
    /// Hide font preview screen (any key)
    HidePreview,
    /// Show font size dialog
    ShowFontSizeDialog,
    /// Font size dialog message
    FontSizeDialog(super::FontSizeDialogMessage),
    /// Tool panel message
    ToolPanel(BitFontToolPanelMessage),
    /// Palette grid message
    PaletteGrid(PaletteGridMessage),
    /// Top toolbar message
    TopToolbar(BitFontTopToolbarMessage),

    // ═══════════════════════════════════════════════════════════════════════
    // Generic keyboard events (panel-agnostic)
    // ═══════════════════════════════════════════════════════════════════════
    /// Arrow key pressed with modifiers - editor decides action based on focused panel
    HandleArrow(ArrowDirection, Modifiers),
    /// Home key - go to beginning of line
    HandleHome,
    /// End key - go to end of line
    HandleEnd,
    /// PageUp key - go to top
    HandlePageUp,
    /// PageDown key - go to bottom
    HandlePageDown,
    /// Confirm action (Space/Enter) - context-dependent
    HandleConfirm,
    /// Cancel action (Escape) - context-dependent
    HandleCancel,
}

/// Canvas interaction events
#[derive(Clone, Debug)]
pub enum CanvasEvent {
    LeftPressed(Point),
    RightPressed(Point),
    MiddlePressed,
    LeftReleased,
    RightReleased,
    CursorMoved(Point),
}
