//! Character set canvas for CharFont (TDF) editor
//!
//! Similar to the BitFont editor's charset grid, but displays TDF font characters.

use iced::{
    Color, Point, Rectangle, Size, keyboard,
    mouse::{self, Cursor},
    widget::canvas::{self, Action, Frame, Path, Stroke},
};

use icy_engine_edit::charset::TdfFont;

use super::{ArrowDirection, CharFontEditorMessage};
use crate::ui::bitfont_editor::style::*;

/// State for CharSetCanvas to track mouse drag for selection
#[derive(Default)]
pub struct CharSetCanvasState {
    /// Currently hovered char code
    pub hovered: Option<u8>,
    /// Whether left mouse button is pressed (for drag selection)
    pub is_dragging: bool,
}

/// Canvas for the TDF character set (printable ASCII chars in a 16x6 grid)
pub struct CharSetCanvas<'a> {
    pub font: Option<&'a TdfFont>,
    pub selected_char: Option<char>,
    pub cursor_col: i32,
    pub cursor_row: i32,
    pub is_focused: bool,
    pub selection: Option<(iced::Point<i32>, iced::Point<i32>, bool)>,
    pub cell_width: f32,
    pub cell_height: f32,
    pub label_size: f32,
}

/// Map from grid position to character code
/// Grid is 16 columns x 6 rows for chars '!' (0x21) to '~' (0x7E)
fn grid_to_char(col: i32, row: i32) -> Option<char> {
    if col < 0 || col >= 16 || row < 0 || row >= 6 {
        return None;
    }
    let index = row * 16 + col;
    let ch_code = b'!' + index as u8;
    if ch_code <= b'~' { Some(ch_code as char) } else { None }
}

/// Map from character to grid position
fn char_to_grid(ch: char) -> Option<(i32, i32)> {
    let code = ch as u8;
    if code >= b'!' && code <= b'~' {
        let index = (code - b'!') as i32;
        Some((index % 16, index / 16))
    } else {
        None
    }
}

impl<'a> canvas::Program<CharFontEditorMessage> for CharSetCanvas<'a> {
    type State = CharSetCanvasState;

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, theme: &iced::Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let palette = theme.extended_palette();
        let bg_color = palette.background.base.color;
        let fg_color = palette.background.base.text;

        // Fill background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg_color);

        // Draw rulers using shared implementation
        let ruler_state = RulerState::new(
            self.is_focused,
            self.cursor_col,
            self.cursor_row,
            16,
            6,
            self.label_size,
            self.cell_width,
            self.cell_height,
            bounds.size(),
        );
        draw_rulers(&mut frame, &ruler_state, theme);

        // Get selected char code
        let selected_code = self.selected_char.map(|c| c as u8);

        // Draw all printable characters (! to ~)
        for row in 0..6 {
            for col in 0..16 {
                if let Some(ch) = grid_to_char(col, row) {
                    let x = self.label_size + col as f32 * self.cell_width;
                    let y = self.label_size + row as f32 * self.cell_height;

                    let ch_code = ch as u8;
                    let is_selected = selected_code == Some(ch_code);
                    let is_cursor_cell = self.is_focused && col == self.cursor_col && row == self.cursor_row;

                    // Check if font has this character
                    let has_char = self.font.map(|f| f.has_char(ch)).unwrap_or(false);

                    // Determine cell colors
                    let (cell_fg, cell_bg) = if is_cursor_cell {
                        // Cursor cell - use highlight colors
                        (palette.primary.base.text, palette.primary.base.color)
                    } else if is_selected {
                        // Selected cell
                        (fg_color, CHAR_HIGHLIGHT_BG)
                    } else if has_char {
                        // Has character defined
                        (fg_color, darken(bg_color, 1.1))
                    } else {
                        // Empty cell
                        (darken(fg_color, 0.5), bg_color)
                    };

                    // Fill cell background
                    frame.fill_rectangle(Point::new(x, y), Size::new(self.cell_width, self.cell_height), cell_bg);

                    // Draw character label
                    frame.fill_text(canvas::Text {
                        content: ch.to_string(),
                        position: Point::new(x + self.cell_width / 2.0, y + self.cell_height / 2.0),
                        color: cell_fg,
                        size: iced::Pixels(12.0),
                        align_x: iced::alignment::Horizontal::Center.into(),
                        align_y: iced::alignment::Vertical::Center.into(),
                        ..Default::default()
                    });

                    // Draw small preview if has char and not too small
                    if has_char && self.cell_height > 20.0 {
                        // Draw a small indicator that the char is defined
                        let indicator_size = 4.0;
                        frame.fill_rectangle(
                            Point::new(x + self.cell_width - indicator_size - 2.0, y + 2.0),
                            Size::new(indicator_size, indicator_size),
                            palette.success.base.color,
                        );
                    }
                }
            }
        }

        // Draw selection if present
        if let Some((anchor, lead, is_rectangle)) = self.selection {
            if is_rectangle {
                // Rectangle mode
                let min_x = anchor.x.min(lead.x);
                let max_x = anchor.x.max(lead.x);
                let min_y = anchor.y.min(lead.y);
                let max_y = anchor.y.max(lead.y);

                for sel_row in min_y..=max_y {
                    for sel_col in min_x..=max_x {
                        if sel_col >= 0 && sel_col < 16 && sel_row >= 0 && sel_row < 6 {
                            let char_x = self.label_size + sel_col as f32 * self.cell_width;
                            let char_y = self.label_size + sel_row as f32 * self.cell_height;
                            frame.fill_rectangle(Point::new(char_x, char_y), Size::new(self.cell_width, self.cell_height), RECT_SELECTION_COLOR);
                        }
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
                // Linear mode
                let anchor_code = anchor.y * 16 + anchor.x;
                let lead_code = lead.y * 16 + lead.x;
                let (start_code, end_code) = if anchor_code <= lead_code {
                    (anchor_code, lead_code)
                } else {
                    (lead_code, anchor_code)
                };

                // Draw background for all selected cells
                for code in start_code..=end_code {
                    let sel_col = code % 16;
                    let sel_row = code / 16;
                    if sel_row < 6 {
                        let char_x = self.label_size + sel_col as f32 * self.cell_width;
                        let char_y = self.label_size + sel_row as f32 * self.cell_height;
                        frame.fill_rectangle(Point::new(char_x, char_y), Size::new(self.cell_width, self.cell_height), SELECTION_COLOR);
                    }
                }

                // Draw border segments only on outer edges
                for code in start_code..=end_code {
                    let sel_col = code % 16;
                    let sel_row = code / 16;
                    if sel_row >= 6 {
                        continue;
                    }
                    let char_x = self.label_size + sel_col as f32 * self.cell_width;
                    let char_y = self.label_size + sel_row as f32 * self.cell_height;

                    let is_selected = |c: i32, r: i32| -> bool {
                        if c < 0 || c > 15 || r < 0 || r > 5 {
                            return false;
                        }
                        let cell_code = r * 16 + c;
                        cell_code >= start_code && cell_code <= end_code
                    };

                    // Top edge
                    if !is_selected(sel_col, sel_row - 1) {
                        let path = Path::line(Point::new(char_x, char_y), Point::new(char_x + self.cell_width, char_y));
                        frame.stroke(&path, Stroke::default().with_color(SELECTION_BORDER).with_width(1.5));
                    }

                    // Bottom edge
                    if !is_selected(sel_col, sel_row + 1) {
                        let path = Path::line(
                            Point::new(char_x, char_y + self.cell_height),
                            Point::new(char_x + self.cell_width, char_y + self.cell_height),
                        );
                        frame.stroke(&path, Stroke::default().with_color(SELECTION_BORDER).with_width(1.5));
                    }

                    // Left edge
                    if !is_selected(sel_col - 1, sel_row) {
                        let path = Path::line(Point::new(char_x, char_y), Point::new(char_x, char_y + self.cell_height));
                        frame.stroke(&path, Stroke::default().with_color(SELECTION_BORDER).with_width(1.5));
                    }

                    // Right edge
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
            if let Some((hover_col, hover_row)) = char_to_grid(hovered_code as char) {
                // Don't draw hover on cursor cell
                let is_cursor_cell = self.is_focused && hover_col == self.cursor_col && hover_row == self.cursor_row;

                if !is_cursor_cell {
                    let hover_x = self.label_size + hover_col as f32 * self.cell_width;
                    let hover_y = self.label_size + hover_row as f32 * self.cell_height;

                    let hover_color = palette.primary.base.color;
                    draw_corner_brackets(&mut frame, hover_x, hover_y, self.cell_width, self.cell_height, hover_color, 1.5);
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<CharFontEditorMessage>> {
        let cursor_pos = cursor.position_in(bounds);

        let old_hovered = state.hovered;

        if let Some(pos) = cursor_pos {
            // Calculate which character is under cursor
            let col = ((pos.x - self.label_size) / self.cell_width) as i32;
            let row = ((pos.y - self.label_size) / self.cell_height) as i32;

            if let Some(ch) = grid_to_char(col, row) {
                state.hovered = Some(ch as u8);

                match event {
                    iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                        state.is_dragging = true;
                        return Some(Action::publish(CharFontEditorMessage::SelectCharAt(ch, col, row)));
                    }
                    iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                        state.is_dragging = false;
                    }
                    iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                        if state.is_dragging {
                            let is_rectangle = icy_engine_gui::is_alt_pressed();
                            return Some(Action::publish(CharFontEditorMessage::SetCharsetSelectionLead(col, row, is_rectangle)));
                        }
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
                if old_hovered.is_some() {
                    return Some(Action::request_redraw());
                }
            }
        } else {
            state.hovered = None;
            if let iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
                state.is_dragging = false;
            }
            if old_hovered.is_some() {
                return Some(Action::request_redraw());
            }
        }

        // Handle keyboard events when focused
        if self.is_focused {
            if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
                match key {
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleArrow(ArrowDirection::Up, *modifiers)));
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleArrow(ArrowDirection::Down, *modifiers)));
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleArrow(ArrowDirection::Left, *modifiers)));
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleArrow(ArrowDirection::Right, *modifiers)));
                    }
                    keyboard::Key::Named(keyboard::key::Named::Home) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleHome));
                    }
                    keyboard::Key::Named(keyboard::key::Named::End) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleEnd));
                    }
                    keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                        return Some(Action::publish(CharFontEditorMessage::HandlePageUp));
                    }
                    keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                        return Some(Action::publish(CharFontEditorMessage::HandlePageDown));
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter | keyboard::key::Named::Space) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleConfirm));
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        return Some(Action::publish(CharFontEditorMessage::HandleCancel));
                    }
                    keyboard::Key::Named(keyboard::key::Named::Tab) => {
                        return Some(Action::publish(CharFontEditorMessage::FocusNextPanel));
                    }
                    _ => {}
                }
            }
        }

        None
    }
}
