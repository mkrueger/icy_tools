//! Central constants for the ANSI editor UI
//!
//! All size, padding, and layout constants in one place for easy tuning.

// =============================================================================
// TOOLBAR BUTTONS (top toolbar with undo, redo, etc.)
// =============================================================================

/// Size of toolbar button icons (undo, redo, etc.)
pub const TOOLBAR_BUTTON_ICON_SIZE: f32 = 20.0;

/// Padding inside toolbar buttons
pub const TOOLBAR_BUTTON_PADDING: f32 = 4.0;

/// Spacing between toolbar buttons
pub const TOOLBAR_BUTTON_SPACING: f32 = 2.0;

// =============================================================================
// TOOL PANEL (left side tool selection)
// =============================================================================

/// Size of each tool icon in the tool panel
pub const TOOL_ICON_SIZE: f32 = 42.0;

/// Padding between tool icons
pub const TOOL_ICON_PADDING: f32 = 3.0;

/// Atlas grid dimensions
pub const TOOL_ATLAS_COLS: u32 = 4;
pub const TOOL_ATLAS_ROWS: u32 = 4;

// =============================================================================
// COLOR SWITCHER (FG/BG color rectangles)
// =============================================================================

/// Total size of the color switcher widget (width = height)
pub const COLOR_SWITCHER_SIZE: f32 = LEFT_BAR_WIDTH;

/// Size of the main FG/BG color rectangles
pub const COLOR_SWITCHER_RECT_SIZE: f32 = 26.0;

/// Margin for shadows around rectangles
pub const COLOR_SWITCHER_SHADOW_MARGIN: f32 = 6.0;

/// Size of the swap icon texture
pub const COLOR_SWITCHER_SWAP_ICON_SIZE: u32 = 24;

// =============================================================================
// PALETTE GRID
// =============================================================================

/// Default cell size for 16-color palette
pub const PALETTE_CELL_SIZE_16: f32 = 20.0;

/// Default cell size for 64-color palette
pub const PALETTE_CELL_SIZE_64: f32 = 16.0;

/// Default cell size for 256-color palette
pub const PALETTE_CELL_SIZE_256: f32 = 12.0;

/// Minimum cell size
pub const PALETTE_CELL_SIZE_MIN: f32 = 12.0;

/// Maximum cell size
pub const PALETTE_CELL_SIZE_MAX: f32 = 24.0;

// =============================================================================
// ANIMATION
// =============================================================================

/// Duration for tool state blend animations (seconds)
pub const TOOL_BLEND_ANIMATION_DURATION: f32 = 0.2;

/// Duration for color swap animation (seconds)
pub const COLOR_SWAP_ANIMATION_DURATION: f32 = 0.15;

// =============================================================================
// LEFT SIDEBAR
// =============================================================================

/// Width of the left sidebar
pub const SIDEBAR_WIDTH: f32 = 64.0;

/// Actual width used for the ANSI editor's left bar (slightly thinner).
pub const LEFT_BAR_WIDTH: f32 = SIDEBAR_WIDTH - 12.0;

// =============================================================================
// RIGHT PANEL
// =============================================================================

/// Width of the right panel (minimap, layers)
pub const RIGHT_PANEL_WIDTH: f32 = 200.0;
