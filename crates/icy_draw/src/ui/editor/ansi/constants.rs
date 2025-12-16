//! Central constants for the ANSI editor UI
//!
//! All size, padding, and layout constants in one place for easy tuning.

// =============================================================================
// TOOL PANEL (left side tool selection)
// =============================================================================

/// Size of each tool icon in the tool panel
pub const TOOL_ICON_SIZE: f32 = 42.0;

/// Padding between tool icons
pub const TOOL_ICON_PADDING: f32 = 3.0;

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
// TOP TOOLBAR CONTROLS (FKey toolbar, SegmentedControl)
// =============================================================================

/// Height of the content area inside toolbar controls (FKey toolbar, SegmentedControl)
pub const TOP_CONTROL_HEIGHT: f32 = 36.0;

/// Shadow padding around toolbar controls
pub const TOP_CONTROL_SHADOW_PADDING: f32 = 6.0;

/// Total height of toolbar controls (content + shadow padding)
pub const TOP_CONTROL_TOTAL_HEIGHT: f32 = TOP_CONTROL_HEIGHT + TOP_CONTROL_SHADOW_PADDING * 2.0 + 4.0;
