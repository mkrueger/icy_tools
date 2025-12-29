//! Style constants and colors for the BitFont editor
//!
//! Provides a consistent, professional look across all editor components.

use iced::widget::canvas::{self, Frame};
use iced::{Color, Point, Size, Theme};

// ═══════════════════════════════════════════════════════════════════════════
// Layout Constants
// ═══════════════════════════════════════════════════════════════════════════

/// Size of each pixel cell in the glyph editor (calculated to match charset height: 16 * scale = CELL_SIZE + CELL_GAP)
pub const CELL_SIZE: f32 = 30.0;
/// Gap between cells
pub const CELL_GAP: f32 = 2.0;
/// Size of the ruler/header area (hex numbers)
pub const RULER_SIZE: f32 = 24.0;
/// Font size for hex labels in rulers
pub const RULER_FONT_SIZE: f32 = 16.0;

// ═══════════════════════════════════════════════════════════════════════════
// Color Palette - Professional dark theme
// ═══════════════════════════════════════════════════════════════════════════

/// Cursor border width
pub const CURSOR_WIDTH: f32 = 2.0;
/// Selection highlight color (semi-transparent blue)
pub const SELECTION_COLOR: Color = Color::from_rgba(0.2, 0.5, 1.0, 0.35);
/// Selection border color
pub const SELECTION_BORDER: Color = Color::from_rgb(0.3, 0.6, 1.0);
/// Rectangle selection color (different hue for distinction)
#[allow(dead_code)]
pub const RECT_SELECTION_COLOR: Color = Color::from_rgba(0.6, 0.4, 1.0, 0.30);

// ═══════════════════════════════════════════════════════════════════════════
// Special Indicator Colors
// ═══════════════════════════════════════════════════════════════════════════

/// 9-dot mode column indicator (slightly blue tint)
pub const NINE_DOT_COLUMN: Color = Color::from_rgb(0.15, 0.15, 0.22);
/// 9-dot separator line color
pub const NINE_DOT_SEPARATOR: Color = Color::from_rgb(0.35, 0.35, 0.55);
/// Shape preview color (yellow with transparency)
pub const SHAPE_PREVIEW: Color = Color::from_rgba(1.0, 0.9, 0.2, 0.6);
/// Highlighted character in charset (current selection)
pub const CHAR_HIGHLIGHT_BG: Color = Color::from_rgb(0.15, 0.35, 0.55);

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Darken a color by a factor (0.0 = black, 1.0 = unchanged)
pub fn darken(color: Color, factor: f32) -> Color {
    Color::from_rgb(color.r * factor, color.g * factor, color.b * factor)
}

/// Draw corner brackets around a cell (hover effect)
///
/// Draws L-shaped corners at each corner of the cell, creating a subtle
/// highlight effect without obscuring the cell content.
pub fn draw_corner_brackets(frame: &mut Frame, x: f32, y: f32, width: f32, height: f32, color: Color, line_width: f32) {
    use iced::widget::canvas::{Path, Stroke};

    // Corner length as fraction of cell size
    let corner_len = (width.min(height) * 0.3).max(3.0);

    let stroke = Stroke::default().with_color(color).with_width(line_width);

    // Top-left corner
    let tl = Path::new(|b| {
        b.move_to(Point::new(x, y + corner_len));
        b.line_to(Point::new(x, y));
        b.line_to(Point::new(x + corner_len, y));
    });
    frame.stroke(&tl, stroke);

    // Top-right corner
    let tr = Path::new(|b| {
        b.move_to(Point::new(x + width - corner_len, y));
        b.line_to(Point::new(x + width, y));
        b.line_to(Point::new(x + width, y + corner_len));
    });
    frame.stroke(&tr, stroke);

    // Bottom-left corner
    let bl = Path::new(|b| {
        b.move_to(Point::new(x, y + height - corner_len));
        b.line_to(Point::new(x, y + height));
        b.line_to(Point::new(x + corner_len, y + height));
    });
    frame.stroke(&bl, stroke);

    // Bottom-right corner
    let br = Path::new(|b| {
        b.move_to(Point::new(x + width - corner_len, y + height));
        b.line_to(Point::new(x + width, y + height));
        b.line_to(Point::new(x + width, y + height - corner_len));
    });
    frame.stroke(&br, stroke);
}

// ═══════════════════════════════════════════════════════════════════════════
// Ruler Drawing - Shared implementation for both editors
// ═══════════════════════════════════════════════════════════════════════════

/// State information needed to draw rulers
pub struct RulerState {
    /// Is the panel focused?
    pub is_focused: bool,
    /// Current cursor column (0-based)
    pub cursor_col: i32,
    /// Current cursor row (0-based)
    pub cursor_row: i32,
    /// Number of columns to display
    pub num_cols: i32,
    /// Number of rows to display
    pub num_rows: i32,
    /// Size of the ruler area (header)
    pub ruler_size: f32,
    /// Width of each cell
    pub cell_width: f32,
    /// Height of each cell
    pub cell_height: f32,
    /// Total bounds size
    pub bounds_size: Size,
    /// Special column to highlight differently (e.g., 9-dot mode column 8)
    pub special_col: Option<i32>,
}

impl RulerState {
    /// Create a new RulerState with the given parameters
    pub fn new(
        is_focused: bool,
        cursor_col: i32,
        cursor_row: i32,
        num_cols: i32,
        num_rows: i32,
        ruler_size: f32,
        cell_width: f32,
        cell_height: f32,
        bounds_size: Size,
    ) -> Self {
        Self {
            is_focused,
            cursor_col,
            cursor_row,
            num_cols,
            num_rows,
            ruler_size,
            cell_width,
            cell_height,
            bounds_size,
            special_col: None,
        }
    }

    /// Set a special column to highlight (e.g., 9-dot mode)
    pub fn with_special_col(mut self, col: i32) -> Self {
        self.special_col = Some(col);
        self
    }
}

/// Draw rulers (column and row headers with hex numbers)
///
/// This provides a consistent look across both the edit grid and charset grid.
/// Uses theme colors for better integration with the UI.
pub fn draw_rulers(frame: &mut Frame, state: &RulerState, theme: &Theme) {
    // Ruler background color depends on focus
    // Focused: use primary color, Unfocused: use secondary/background
    let ruler_bg = if state.is_focused { theme.accent.selected } else { theme.primary.divider };

    // Draw ruler backgrounds
    frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(state.ruler_size, state.bounds_size.height), ruler_bg);
    frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(state.bounds_size.width, state.ruler_size), ruler_bg);

    // Text colors from theme
    let text_normal = if state.is_focused { theme.button.on } else { theme.background.base };
    let text_highlight = if state.is_focused { theme.background.on } else { theme.secondary.on };
    // for the 9px column
    let text_special = text_normal;

    // Draw column rulers (hex: 0-F)
    for col in 0..state.num_cols {
        let cell_x = state.ruler_size + (col as f32 + 0.5) * state.cell_width;

        // Determine text color based on state
        let ruler_color = if state.special_col == Some(col) {
            text_special
        } else if col == state.cursor_col {
            text_highlight
        } else {
            text_normal
        };

        frame.fill_text(canvas::Text {
            content: format!("{:X}", col),
            position: Point::new(cell_x, state.ruler_size / 2.0),
            color: ruler_color,
            size: iced::Pixels(RULER_FONT_SIZE),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center.into(),
            ..Default::default()
        });
    }

    // Draw row rulers (hex: 0-F)
    for row in 0..state.num_rows {
        let cell_y = state.ruler_size + (row as f32 + 0.5) * state.cell_height;

        let ruler_color = if row == state.cursor_row { text_highlight } else { text_normal };

        frame.fill_text(canvas::Text {
            content: format!("{:X}", row),
            position: Point::new(state.ruler_size / 2.0, cell_y),
            color: ruler_color,
            size: iced::Pixels(RULER_FONT_SIZE),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center.into(),
            ..Default::default()
        });
    }
}
