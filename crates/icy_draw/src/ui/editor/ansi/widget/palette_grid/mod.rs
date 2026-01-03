//! Palette grid component
//!
//! Shows all palette colors in a grid with FG/BG markers.
//! Supports 16, 64, and 256 color palettes.
//! Ported from egui palette_editor_16.

use icy_engine::{FontMode, IceMode, Palette};
use icy_ui::{
    mouse,
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Program, Stroke},
    Color, Element, Length, Point, Rectangle, Size, Theme,
};

/// Messages from the palette grid
#[derive(Clone, Debug)]
pub enum PaletteGridMessage {
    /// Set foreground color (left click)
    SetForeground(u32),
    /// Set background color (right click)
    SetBackground(u32),
}

/// Palette grid state
pub struct PaletteGrid {
    /// Current foreground color index
    foreground: u32,
    /// Current background color index
    background: u32,
    /// ICE mode for high background colors
    ice_mode: IceMode,
    /// Font mode for high foreground colors
    font_mode: FontMode,
    /// Cached palette for rendering
    cached_palette: Palette,
}

impl Default for PaletteGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl PaletteGrid {
    pub fn new() -> Self {
        Self {
            foreground: 7,
            background: 0,
            ice_mode: IceMode::Unlimited,
            font_mode: FontMode::Unlimited,
            cached_palette: Palette::dos_default(),
        }
    }

    /// Set foreground color
    pub fn set_foreground(&mut self, color: u32) {
        self.foreground = color;
    }

    /// Set background color
    pub fn set_background(&mut self, color: u32) {
        self.background = color;
    }

    /// Set ice mode (for high background color availability)
    pub fn set_ice_mode(&mut self, mode: IceMode) {
        self.ice_mode = mode;
    }

    /// Set font mode (for high foreground color availability)
    pub fn set_font_mode(&mut self, mode: FontMode) {
        self.font_mode = mode;
    }

    /// Sync palette from edit state.
    ///
    /// `color_limit` is only used to clamp the stored FG/BG marker indices.
    /// The palette itself is kept unmodified so switching format modes can
    /// immediately show more colors again.
    pub fn sync_palette(&mut self, palette: &Palette, color_limit: Option<usize>) {
        if let Some(limit) = color_limit {
            let max_index = limit.saturating_sub(1) as u32;
            self.foreground = self.foreground.min(max_index);
            self.background = self.background.min(max_index);
        }

        // Only clone if different
        if self.cached_palette != *palette {
            self.cached_palette = palette.clone();
        }
    }

    /// Calculate the optimal layout for the palette based on available width
    /// Returns (items_per_row, cell_size, total_width, total_height)
    fn calculate_layout_for_width(palette_len: usize, available_width: f32) -> (usize, f32, f32, f32) {
        // Target cell size - we want reasonably sized color cells
        let target_cell_size = 16.0;
        let min_cell_size = 12.0;
        let max_cell_size = 24.0;

        // Calculate how many columns fit with target cell size
        let cols_at_target = (available_width / target_cell_size).floor() as usize;
        let cols = cols_at_target.max(2).min(palette_len); // At least 2, at most palette_len

        // Calculate actual cell size to fill the width
        let cell_size = (available_width / cols as f32).clamp(min_cell_size, max_cell_size);

        // Recalculate cols with clamped cell size
        let cols = (available_width / cell_size).floor() as usize;
        let cols = cols.max(2).min(palette_len);

        let rows = (palette_len as f32 / cols as f32).ceil() as usize;
        let width = cell_size * cols as f32;
        let height = cell_size * rows as f32;

        (cols, cell_size, width, height)
    }

    /// Render the palette grid with a specific available width.
    ///
    /// `color_limit` restricts display/selection to the first N colors.
    pub fn view_with_width(&self, available_width: f32, color_limit: Option<usize>) -> Element<'_, PaletteGridMessage> {
        let palette_len = self.cached_palette.len();
        let visible_len = color_limit.map(|l| l.min(palette_len)).unwrap_or(palette_len);

        // Special layouts:
        // - 16 colors: 8 rows × 2 columns (vertical), square cells, fill full available width
        // -  8 colors: 8 rows × 1 column (vertical), same total height as 16 colors
        let (items_per_row, cell_width, cell_height, width, height, x_offset) = if visible_len == 16 || visible_len == 8 {
            let items_per_row = if visible_len == 16 { 2 } else { 1 };

            // For 8 colors we still base the cell size on a 2-column layout so
            // the overall height matches the 16-color palette.
            let sizing_cols = 2.0;
            let cell_size = (available_width / sizing_cols).max(1.0);

            let used_width = cell_size * items_per_row as f32;
            let x_offset = ((available_width - used_width) / 2.0).max(0.0);

            (items_per_row, cell_size, cell_size, available_width, cell_size * 8.0, x_offset)
        } else {
            let (items_per_row, cell_size, width, height) = Self::calculate_layout_for_width(visible_len, available_width);
            (items_per_row, cell_size, cell_size, width, height, 0.0)
        };

        Canvas::new(PaletteGridProgram {
            foreground: self.foreground,
            background: self.background,
            ice_mode: self.ice_mode,
            font_mode: self.font_mode,
            cached_palette: self.cached_palette.clone(),
            visible_len,
            items_per_row,
            cell_width,
            cell_height,
            x_offset,
        })
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
    }
}

/// Canvas program for drawing the palette grid
struct PaletteGridProgram {
    foreground: u32,
    background: u32,
    ice_mode: IceMode,
    font_mode: FontMode,
    cached_palette: Palette,
    visible_len: usize,
    items_per_row: usize,
    cell_width: f32,
    cell_height: f32,
    x_offset: f32,
}

impl PaletteGridProgram {
    /// Convert (col, row) grid position to color index.
    /// For 16-color palette with 2 columns: lo colors (0-7) left, hi colors (8-15) right.
    fn grid_to_color(&self, col: u32, row: u32) -> u32 {
        if self.visible_len == 16 && self.items_per_row == 2 {
            // Column 0: colors 0-7, Column 1: colors 8-15
            col * 8 + row
        } else {
            col + row * self.items_per_row as u32
        }
    }

    /// Convert color index to (col, row) grid position.
    fn color_to_grid(&self, color: u32) -> (usize, usize) {
        if self.visible_len == 16 && self.items_per_row == 2 {
            // Column 0: colors 0-7, Column 1: colors 8-15
            let col = if color >= 8 { 1 } else { 0 };
            let row = (color % 8) as usize;
            (col, row)
        } else {
            let col = color as usize % self.items_per_row;
            let row = color as usize / self.items_per_row;
            (col, row)
        }
    }
}

impl Program<PaletteGridMessage> for PaletteGridProgram {
    type State = Option<u32>; // Hovered color index

    fn draw(&self, _state: &Self::State, renderer: &icy_ui::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let cell_width = self.cell_width;
        let cell_height = self.cell_height;
        let upper_limit = ((self.visible_len as f32 / self.items_per_row as f32).ceil() as usize) * self.items_per_row;

        // Draw color cells
        for i in 0..upper_limit.min(self.visible_len) {
            let (col, row) = self.color_to_grid(i as u32);
            let x = self.x_offset + col as f32 * cell_width;
            let y = row as f32 * cell_height;

            let (r, g, b) = self.cached_palette.rgb(i as u32);
            frame.fill_rectangle(Point::new(x, y), Size::new(cell_width, cell_height), Color::from_rgb8(r, g, b));
        }

        // Draw foreground marker (triangle top-left)
        let marker_len = cell_width.min(cell_height) / 3.0;
        let max_index = self.visible_len.saturating_sub(1) as u32;
        let foreground = self.foreground.min(max_index);
        let background = self.background.min(max_index);

        let (fg_col, fg_row) = self.color_to_grid(foreground);
        let fg_origin = Point::new(self.x_offset + fg_col as f32 * cell_width, fg_row as f32 * cell_height);

        let fg_marker = Path::new(|builder| {
            builder.move_to(fg_origin);
            builder.line_to(Point::new(fg_origin.x + marker_len, fg_origin.y));
            builder.line_to(Point::new(fg_origin.x, fg_origin.y + marker_len));
            builder.close();
        });

        frame.fill(&fg_marker, Color::WHITE);
        frame.stroke(&fg_marker, Stroke::default().with_color(Color::from_rgb8(128, 128, 128)).with_width(1.0));

        // Draw background marker (triangle bottom-right)
        let (bg_col, bg_row) = self.color_to_grid(background);
        let bg_origin = Point::new(self.x_offset + (bg_col + 1) as f32 * cell_width, (bg_row + 1) as f32 * cell_height);

        let bg_marker = Path::new(|builder| {
            builder.move_to(bg_origin);
            builder.line_to(Point::new(bg_origin.x - marker_len, bg_origin.y));
            builder.line_to(Point::new(bg_origin.x, bg_origin.y - marker_len));
            builder.close();
        });

        frame.fill(&bg_marker, Color::WHITE);
        frame.stroke(&bg_marker, Stroke::default().with_color(Color::from_rgb8(128, 128, 128)).with_width(1.0));

        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut Self::State, event: &icy_ui::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<PaletteGridMessage>> {
        let cell_width = self.cell_width;
        let cell_height = self.cell_height;
        let used_width = cell_width * self.items_per_row as f32;

        match event {
            icy_ui::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let local_x = pos.x - self.x_offset;
                    if local_x < 0.0 || local_x >= used_width {
                        *state = None;
                        return None;
                    }

                    let col = (local_x / cell_width) as u32;
                    let row = (pos.y / cell_height) as u32;
                    let color = self.grid_to_color(col, row);
                    if color < self.visible_len as u32 {
                        *state = Some(color);
                    } else {
                        *state = None;
                    }
                }
                None
            }
            icy_ui::Event::Mouse(mouse::Event::ButtonPressed { button, .. }) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let local_x = pos.x - self.x_offset;
                    if local_x < 0.0 || local_x >= used_width {
                        return None;
                    }

                    let col = (local_x / cell_width) as u32;
                    let row = (pos.y / cell_height) as u32;
                    let color = self.grid_to_color(col, row);

                    if color < self.visible_len as u32 {
                        match button {
                            mouse::Button::Left => {
                                // Check if high foreground color is allowed
                                if color < 8 || self.font_mode.has_high_fg_colors() || self.visible_len > 16 {
                                    return Some(canvas::Action::publish(PaletteGridMessage::SetForeground(color)));
                                }
                            }
                            mouse::Button::Right => {
                                // Check if high background color is allowed
                                if color < 8 || self.ice_mode.has_high_bg_colors() || self.visible_len > 16 {
                                    return Some(canvas::Action::publish(PaletteGridMessage::SetBackground(color)));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}
