//! Character set canvas for BitFont editor

use iced::{
    Color, Point, Rectangle, Size,
    mouse::{self, Cursor},
    widget::canvas::{self, Action, Frame, Path, Stroke},
};
use icy_engine_edit::bitfont::BitFontFocusedPanel;

use super::super::style::*;
use super::super::{BitFontEditor, BitFontEditorMessage};

/// State for CharSetCanvas to track mouse drag for selection
#[derive(Default)]
pub struct CharSetCanvasState {
    /// Currently hovered char code
    pub hovered: Option<u8>,
    /// Whether left mouse button is pressed (for drag selection)
    pub is_dragging: bool,
}

/// Canvas for the entire character set (16x16 grid with hex labels)
pub struct CharSetCanvas<'a> {
    pub editor: &'a BitFontEditor,
    pub fg_color: u32,
    pub bg_color: u32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub label_size: f32,
}

impl<'a> canvas::Program<BitFontEditorMessage> for CharSetCanvas<'a> {
    type State = CharSetCanvasState;

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, theme: &iced::Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Get colors from palette
        let palette = icy_engine::Palette::dos_default();
        let (fg_r, fg_g, fg_b) = palette.get_rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = palette.get_rgb(self.bg_color);
        let fg_iced_color = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg_iced_color = Color::from_rgb8(bg_r, bg_g, bg_b);

        let (width, height) = self.editor.font_size();
        let selected = self.editor.selected_char();
        let selected_code = selected as u32;

        // Check if we're in 9-dot mode
        let use_9dot = self.editor.use_letter_spacing() && width == 8;
        let display_width = if use_9dot { width + 1 } else { width };

        // Fill background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg_iced_color);

        // Get charset cursor for highlighting
        let (cursor_col, cursor_row) = self.editor.charset_cursor();

        // Check focus state
        let is_focused = self.editor.state.focused_panel() == BitFontFocusedPanel::CharSet;

        // Draw rulers using shared implementation
        let ruler_state = RulerState::new(
            is_focused,
            cursor_col,
            cursor_row,
            16,
            16,
            self.label_size,
            self.cell_width,
            self.cell_height,
            bounds.size(),
        );
        draw_rulers(&mut frame, &ruler_state, theme);

        // Draw all 256 characters
        for ch_code in 0..256u32 {
            let row = (ch_code / 16) as i32;
            let col = (ch_code % 16) as i32;
            let x = self.label_size + col as f32 * self.cell_width;
            let y = self.label_size + row as f32 * self.cell_height;

            let ch = char::from_u32(ch_code).unwrap_or(' ');
            let pixels = self.editor.get_glyph_pixels(ch);
            let is_selected = ch_code == selected_code;
            let is_cursor_cell = is_focused && col == cursor_col && row == cursor_row;

            // Cursor uses visible color, not XOR
            let (cell_fg, cell_bg) = if is_cursor_cell {
                theme_cursor_color(&theme)
            } else if is_selected {
                // Highlight selected character with different bg
                (fg_iced_color, CHAR_HIGHLIGHT_BG)
            } else {
                (fg_iced_color, bg_iced_color)
            };

            // Fill cell background
            frame.fill_rectangle(Point::new(x, y), Size::new(self.cell_width, self.cell_height), cell_bg);

            // Calculate pixel scale factor
            let pixel_scale_x = self.cell_width / display_width as f32;
            let pixel_scale_y = self.cell_height / height as f32;

            // Check if this is a box-drawing character (for 9-dot mode)
            let is_box_drawing = (0xC0..=0xDF).contains(&ch_code);

            // Draw pixels (scaled) - use cell_fg for XOR effect
            for py in 0..height as usize {
                for px in 0..width as usize {
                    let is_set = pixels.get(py).and_then(|r| r.get(px)).copied().unwrap_or(false);
                    if is_set {
                        frame.fill_rectangle(
                            Point::new(x + px as f32 * pixel_scale_x, y + py as f32 * pixel_scale_y),
                            Size::new(pixel_scale_x, pixel_scale_y),
                            cell_fg,
                        );
                    }
                }

                // Draw 9th column if in 9-dot mode
                if use_9dot {
                    let pixel_8 = pixels.get(py).and_then(|r| r.get(7)).copied().unwrap_or(false);
                    let is_set = is_box_drawing && pixel_8;
                    if is_set {
                        frame.fill_rectangle(
                            Point::new(x + 8.0 * pixel_scale_x, y + py as f32 * pixel_scale_y),
                            Size::new(pixel_scale_x, pixel_scale_y),
                            cell_fg,
                        );
                    }
                }
            }
        }

        // Draw charset selection if present
        //
        // SELECTION DRAWING STRATEGY:
        // - Linear mode: Characters are selected in reading order (left-to-right, top-to-bottom).
        //   We draw a filled background on each selected cell, then draw border segments only
        //   on the OUTER edges of the selection (where adjacent cells are NOT selected).
        //   This creates a continuous outline around the entire selection region.
        //
        // - Rectangle mode: All characters in the bounding box are selected.
        //   We draw filled backgrounds on each cell, then a single outer rectangle border.
        //
        if let Some((anchor, lead, is_rectangle)) = self.editor.charset_selection() {
            if is_rectangle {
                // Rectangle mode: highlight all characters in the bounding box
                let min_x = anchor.x.min(lead.x);
                let max_x = anchor.x.max(lead.x);
                let min_y = anchor.y.min(lead.y);
                let max_y = anchor.y.max(lead.y);

                for sel_row in min_y..=max_y {
                    for sel_col in min_x..=max_x {
                        let char_x = self.label_size + sel_col as f32 * self.cell_width;
                        let char_y = self.label_size + sel_row as f32 * self.cell_height;

                        // Draw selection highlight background (rectangle mode uses different color)
                        frame.fill_rectangle(Point::new(char_x, char_y), Size::new(self.cell_width, self.cell_height), RECT_SELECTION_COLOR);
                    }
                }

                // Draw outer rectangle border
                let sel_x = self.label_size + min_x as f32 * self.cell_width - 1.0;
                let sel_y = self.label_size + min_y as f32 * self.cell_height - 1.0;
                let sel_w = (max_x - min_x + 1) as f32 * self.cell_width + 2.0;
                let sel_h = (max_y - min_y + 1) as f32 * self.cell_height + 2.0;
                let selection_path = Path::rectangle(Point::new(sel_x, sel_y), Size::new(sel_w, sel_h));
                frame.stroke(
                    &selection_path,
                    Stroke::default().with_color(Color::from_rgb(0.6, 0.4, 1.0)).with_width(CURSOR_WIDTH),
                );
            } else {
                // Linear mode: highlight characters from anchor_code to lead_code (reading order)
                let anchor_code = anchor.y * 16 + anchor.x;
                let lead_code = lead.y * 16 + lead.x;
                let (start_code, end_code) = if anchor_code <= lead_code {
                    (anchor_code, lead_code)
                } else {
                    (lead_code, anchor_code)
                };

                // First pass: draw background for all selected cells
                for code in start_code..=end_code {
                    let sel_col = code % 16;
                    let sel_row = code / 16;
                    let char_x = self.label_size + sel_col as f32 * self.cell_width;
                    let char_y = self.label_size + sel_row as f32 * self.cell_height;

                    frame.fill_rectangle(Point::new(char_x, char_y), Size::new(self.cell_width, self.cell_height), SELECTION_COLOR);
                }

                // Second pass: draw border segments only on outer edges
                // An edge is "outer" if the adjacent cell (in that direction) is NOT selected.
                // For linear selection, we must consider that selection wraps at column 15->0.
                for code in start_code..=end_code {
                    let sel_col = code % 16;
                    let sel_row = code / 16;
                    let char_x = self.label_size + sel_col as f32 * self.cell_width;
                    let char_y = self.label_size + sel_row as f32 * self.cell_height;

                    // Helper: check if a cell at (col, row) is selected
                    let is_selected = |c: i32, r: i32| -> bool {
                        if c < 0 || c > 15 || r < 0 || r > 15 {
                            return false;
                        }
                        let cell_code = r * 16 + c;
                        cell_code >= start_code && cell_code <= end_code
                    };

                    // Top edge: draw if cell above is not selected
                    if !is_selected(sel_col, sel_row - 1) {
                        let path = Path::line(Point::new(char_x, char_y), Point::new(char_x + self.cell_width, char_y));
                        frame.stroke(&path, Stroke::default().with_color(SELECTION_BORDER).with_width(1.5));
                    }

                    // Bottom edge: draw if cell below is not selected
                    if !is_selected(sel_col, sel_row + 1) {
                        let path = Path::line(
                            Point::new(char_x, char_y + self.cell_height),
                            Point::new(char_x + self.cell_width, char_y + self.cell_height),
                        );
                        frame.stroke(&path, Stroke::default().with_color(SELECTION_BORDER).with_width(1.5));
                    }

                    // Left edge: draw if cell to left is not selected
                    if !is_selected(sel_col - 1, sel_row) {
                        let path = Path::line(Point::new(char_x, char_y), Point::new(char_x, char_y + self.cell_height));
                        frame.stroke(&path, Stroke::default().with_color(SELECTION_BORDER).with_width(1.5));
                    }

                    // Right edge: draw if cell to right is not selected
                    if !is_selected(sel_col + 1, sel_row) {
                        let path = Path::line(
                            Point::new(char_x + self.cell_width, char_y),
                            Point::new(char_x + self.cell_width, char_y + self.cell_height),
                        );
                        frame.stroke(&path, Stroke::default().with_color(SELECTION_BORDER).with_width(1.5));
                    }
                }
            }
        }

        // Draw hover effect with corner brackets
        if let Some(hovered_code) = state.hovered {
            let hover_col = (hovered_code % 16) as i32;
            let hover_row = (hovered_code / 16) as i32;

            // Don't draw hover on cursor cell (it's already highlighted)
            let (cursor_col, cursor_row) = self.editor.charset_cursor();
            let is_cursor_cell = is_focused && hover_col == cursor_col && hover_row == cursor_row;

            if !is_cursor_cell {
                let hover_x = self.label_size + hover_col as f32 * self.cell_width;
                let hover_y = self.label_size + hover_row as f32 * self.cell_height;

                // Use theme color for hover
                let iced_palette = theme.extended_palette();
                let hover_color = iced_palette.primary.base.color;

                draw_corner_brackets(&mut frame, hover_x, hover_y, self.cell_width, self.cell_height, hover_color, 1.5);
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<BitFontEditorMessage>> {
        let cursor_pos = cursor.position_in(bounds);

        // Update hover state first
        let old_hovered = state.hovered;

        if let Some(pos) = cursor_pos {
            // Calculate which character is under cursor
            let col = ((pos.x - self.label_size) / self.cell_width) as i32;
            let row = ((pos.y - self.label_size) / self.cell_height) as i32;

            if col >= 0 && col < 16 && row >= 0 && row < 16 {
                let ch_code = (row * 16 + col) as u8;
                state.hovered = Some(ch_code);

                match event {
                    iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                        state.is_dragging = true;
                        let ch = char::from_u32(ch_code as u32).unwrap_or(' ');
                        // SelectGlyph sets anchor position and focus to CharSet
                        return Some(Action::publish(BitFontEditorMessage::SelectGlyphAt(ch, col, row)));
                    }
                    iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                        state.is_dragging = false;
                    }
                    iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                        if state.is_dragging {
                            // Extend charset selection while dragging
                            // Uses anchor/lead: anchor stays fixed, lead follows cursor
                            // Alt key toggles rectangle mode (uses global modifier state)
                            let is_rectangle = icy_engine_gui::is_alt_pressed();
                            return Some(Action::publish(BitFontEditorMessage::SetCharsetSelectionLead(col, row, is_rectangle)));
                        }
                        // Request redraw if hover changed
                        if old_hovered != state.hovered {
                            return Some(Action::request_redraw());
                        }
                    }
                    _ => {}
                }
            } else {
                state.hovered = None;
                if let iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
                    state.is_dragging = false;
                }
                // Request redraw if hover changed (mouse outside grid cells)
                if old_hovered.is_some() {
                    return Some(Action::request_redraw());
                }
            }
        } else {
            // Cursor left the canvas entirely
            state.hovered = None;
            if let iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
                state.is_dragging = false;
            }
            // Request redraw if hover changed (mouse left canvas)
            if old_hovered.is_some() {
                return Some(Action::request_redraw());
            }
        }

        None
    }
}

pub fn theme_cursor_color(theme: &iced::Theme) -> (Color, Color) {
    (theme.extended_palette().success.base.text, theme.extended_palette().success.base.color)
}
