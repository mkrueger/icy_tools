//! Edit grid canvas for BitFont editor

use icy_ui::{
    keyboard::{self, Key},
    mouse::{self, Cursor},
    widget::canvas::{self, Action, Path, Stroke},
    Color, Point, Rectangle, Size,
};
use icy_engine_edit::bitfont::{brushes, BitFontFocusedPanel};
use icy_engine_gui::theme::main_area_background;

use super::super::{style::*, ArrowDirection, BitFontEditor, BitFontEditorMessage, BitFontTool, CanvasEvent};

/// State for EditGridCanvas to track hover position
#[derive(Default)]
pub struct EditGridCanvasState {
    /// Currently hovered cell (x, y)
    pub hovered: Option<(i32, i32)>,
}

/// Canvas for the pixel edit grid
pub struct EditGridCanvas<'a> {
    pub editor: &'a BitFontEditor,
    pub fg_color: u32,
    pub bg_color: u32,
}

impl<'a> canvas::Program<BitFontEditorMessage> for EditGridCanvas<'a> {
    type State = EditGridCanvasState;

    fn draw(&self, state: &Self::State, renderer: &icy_ui::Renderer, theme: &icy_ui::Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<canvas::Geometry> {
        // Get colors from palette
        let palette = icy_engine::Palette::dos_default();
        let (fg_r, fg_g, fg_b) = palette.rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = palette.rgb(self.bg_color);
        let fg_iced_color = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg_iced_color = Color::from_rgb8(bg_r, bg_g, bg_b);

        // Get main area background color from theme (before closure)
        let area_bg_color = main_area_background(theme);

        let geometry = self.editor.edit_cache.draw(renderer, bounds.size(), |frame| {
            let (width, height) = self.editor.font_size();
            let selected = self.editor.selected_char();
            let pixels = self.editor.get_glyph_pixels(selected);

            // Check if we're in 9-dot mode and if this is a box-drawing character
            let use_9dot = self.editor.use_letter_spacing() && width == 8;
            let display_width = if use_9dot { width + 1 } else { width };
            let char_code = selected as u32;
            let is_box_drawing = (0xC0..=0xDF).contains(&char_code);

            // Background - use theme's main area background color
            frame.fill_rectangle(Point::ORIGIN, frame.size(), area_bg_color);

            // Get scaled cell dimensions from editor
            let cell_size = self.editor.scaled_edit_cell_size();
            let cell_gap = self.editor.scaled_edit_cell_gap();

            // Get cursor position for highlighting
            let (cursor_x, cursor_y) = self.editor.cursor_pos();

            // Get focus state for ruler highlight
            let is_focused = self.editor.state.focused_panel() == BitFontFocusedPanel::EditGrid;

            // Draw rulers using shared implementation
            let mut ruler_state = RulerState::new(
                is_focused,
                cursor_x,
                cursor_y,
                display_width,
                height,
                RULER_SIZE,
                cell_size + cell_gap,
                cell_size + cell_gap,
                frame.size(),
            );

            // Mark 9th column as special in 9-dot mode
            if use_9dot {
                ruler_state = ruler_state.with_special_col(8);
            }

            draw_rulers(frame, &ruler_state, theme);

            // Calculate cursor colors based on fg/bg for better visibility
            let cursor_color = crate::ui::editor::bitfont::canvas::charset::calculate_cursor_colors(fg_iced_color, bg_iced_color);

            // Draw pixel grid with palette colors
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let cell_x = RULER_SIZE + x as f32 * (cell_size + cell_gap);
                    let cell_y = RULER_SIZE + y as f32 * (cell_size + cell_gap);

                    let is_set = pixels.get(y).and_then(|row| row.get(x)).copied().unwrap_or(false);
                    let is_cursor_cell = is_focused && x as i32 == cursor_x && y as i32 == cursor_y;

                    let color = if is_set {
                        if is_cursor_cell {
                            cursor_color.0
                        } else {
                            fg_iced_color
                        }
                    } else {
                        if is_cursor_cell {
                            cursor_color.1
                        } else {
                            bg_iced_color
                        }
                    };

                    frame.fill_rectangle(Point::new(cell_x, cell_y), Size::new(cell_size, cell_size), color);
                }

                // Draw 9th column if in 9-dot mode
                if use_9dot {
                    let cell_x = RULER_SIZE + 8.0 * (cell_size + cell_gap);
                    let cell_y = RULER_SIZE + y as f32 * (cell_size + cell_gap);

                    // For box-drawing characters, extend the 8th pixel to the 9th
                    // Otherwise, the 9th pixel is always background
                    let pixel_8 = pixels.get(y).and_then(|row| row.get(7)).copied().unwrap_or(false);
                    let is_set = is_box_drawing && pixel_8;

                    // Use a slightly different shade to indicate it's not editable
                    let color = if is_set { darken(fg_iced_color, 0.85) } else { NINE_DOT_COLUMN };

                    frame.fill_rectangle(Point::new(cell_x, cell_y), Size::new(cell_size, cell_size), color);
                }
            }

            // Draw separator line before 9th column if in 9-dot mode
            if use_9dot {
                let sep_x = RULER_SIZE + 8.0 * (cell_size + cell_gap) - cell_gap / 2.0;
                let sep_path = Path::line(
                    Point::new(sep_x, RULER_SIZE),
                    Point::new(sep_x, RULER_SIZE + height as f32 * (cell_size + cell_gap)),
                );
                frame.stroke(&sep_path, Stroke::default().with_color(NINE_DOT_SEPARATOR).with_width(2.0));
            }

            // Draw selection highlight
            if let Some((x1, y1, x2, y2)) = self.editor.selection() {
                let (min_x, max_x) = (x1.min(x2), x1.max(x2));
                let (min_y, max_y) = (y1.min(y2), y1.max(y2));

                let sel_x = RULER_SIZE + min_x as f32 * (cell_size + cell_gap) - 1.0;
                let sel_y = RULER_SIZE + min_y as f32 * (cell_size + cell_gap) - 1.0;
                let sel_w = (max_x - min_x + 1) as f32 * (cell_size + cell_gap) + 2.0;
                let sel_h = (max_y - min_y + 1) as f32 * (cell_size + cell_gap) + 2.0;

                // Fill selection area
                frame.fill_rectangle(Point::new(sel_x, sel_y), Size::new(sel_w, sel_h), SELECTION_COLOR);

                // Draw selection border
                let selection_path = Path::rectangle(Point::new(sel_x, sel_y), Size::new(sel_w, sel_h));
                frame.stroke(&selection_path, Stroke::default().with_color(SELECTION_BORDER).with_width(CURSOR_WIDTH));
            }

            // Draw shape preview while dragging
            if self.editor.is_dragging() {
                if let Some((start_x, start_y)) = self.editor.drag_start() {
                    let (end_x, end_y) = self.editor.cursor_pos();

                    let preview_points: Vec<(i32, i32)> = match self.editor.current_tool {
                        BitFontTool::Line => brushes::bresenham_line(start_x, start_y, end_x, end_y),
                        BitFontTool::RectangleOutline => brushes::rectangle_points(start_x, start_y, end_x, end_y, false),
                        BitFontTool::RectangleFilled => brushes::rectangle_points(start_x, start_y, end_x, end_y, true),
                        _ => Vec::new(),
                    };

                    // Draw preview pixels as semi-transparent overlay
                    for (px, py) in preview_points {
                        if px >= 0 && px < width && py >= 0 && py < height {
                            let cell_x = RULER_SIZE + px as f32 * (cell_size + cell_gap);
                            let cell_y = RULER_SIZE + py as f32 * (cell_size + cell_gap);
                            frame.fill_rectangle(Point::new(cell_x, cell_y), Size::new(cell_size, cell_size), SHAPE_PREVIEW);
                        }
                    }
                }
            }

            // No extra cursor decoration needed - XOR cursor is sufficient
        });

        // Draw hover effect as separate geometry (not cached, updates on mouse move)
        let mut geometries = vec![geometry];

        if let Some((hover_x, hover_y)) = state.hovered {
            let (width, height) = self.editor.font_size();
            let (cursor_x, cursor_y) = self.editor.cursor_pos();
            let is_focused = self.editor.state.focused_panel() == BitFontFocusedPanel::EditGrid;

            // Don't draw hover on cursor cell (it's already highlighted)
            let is_cursor_cell = is_focused && hover_x == cursor_x && hover_y == cursor_y;

            if !is_cursor_cell && hover_x >= 0 && hover_x < width && hover_y >= 0 && hover_y < height {
                // Get scaled dimensions
                let cell_size = self.editor.scaled_edit_cell_size();
                let cell_gap = self.editor.scaled_edit_cell_gap();

                let hover_geometry = icy_ui::widget::canvas::Cache::new().draw(renderer, bounds.size(), |frame| {
                    let cell_x = RULER_SIZE + hover_x as f32 * (cell_size + cell_gap);
                    let cell_y = RULER_SIZE + hover_y as f32 * (cell_size + cell_gap);

                    // Use theme color for hover
                    let hover_color = theme.accent.base;

                    draw_corner_brackets(frame, cell_x, cell_y, cell_size, cell_size, hover_color, 2.0);
                });
                geometries.push(hover_geometry);
            }
        }

        geometries
    }

    fn update(&self, state: &mut Self::State, event: &icy_ui::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<BitFontEditorMessage>> {
        // Handle keyboard events
        if let icy_ui::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
            // Tab to switch focus
            if matches!(key, Key::Named(keyboard::key::Named::Tab)) {
                return Some(Action::publish(BitFontEditorMessage::FocusNextPanel));
            }

            // Arrow keys - delegate to editor which decides based on focused panel
            match key {
                Key::Named(keyboard::key::Named::ArrowUp) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleArrow(ArrowDirection::Up, *modifiers)));
                }
                Key::Named(keyboard::key::Named::ArrowDown) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleArrow(ArrowDirection::Down, *modifiers)));
                }
                Key::Named(keyboard::key::Named::ArrowLeft) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleArrow(ArrowDirection::Left, *modifiers)));
                }
                Key::Named(keyboard::key::Named::ArrowRight) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleArrow(ArrowDirection::Right, *modifiers)));
                }
                // Space/Enter - context-dependent confirm action
                Key::Named(keyboard::key::Named::Space) | Key::Named(keyboard::key::Named::Enter) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleConfirm));
                }
                // Escape - context-dependent cancel action
                Key::Named(keyboard::key::Named::Escape) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleCancel));
                }
                // Home - go to beginning of line
                Key::Named(keyboard::key::Named::Home) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleHome));
                }
                // End - go to end of line
                Key::Named(keyboard::key::Named::End) => {
                    return Some(Action::publish(BitFontEditorMessage::HandleEnd));
                }
                // PageUp - go to top
                Key::Named(keyboard::key::Named::PageUp) => {
                    return Some(Action::publish(BitFontEditorMessage::HandlePageUp));
                }
                // PageDown - go to bottom
                Key::Named(keyboard::key::Named::PageDown) => {
                    return Some(Action::publish(BitFontEditorMessage::HandlePageDown));
                }
                _ => {}
            }

            // Plus/Minus - next/prev character (works in both modes)
            match key {
                Key::Character(c) if c.as_str() == "+" || c.as_str() == "=" => {
                    return Some(Action::publish(BitFontEditorMessage::NextChar));
                }
                Key::Character(c) if c.as_str() == "-" => {
                    return Some(Action::publish(BitFontEditorMessage::PrevChar));
                }
                _ => {}
            }
        }

        // Handle mouse events - check if cursor is over the canvas
        let cursor_pos = cursor.position_in(bounds);

        // Get scaled dimensions for hover calculations
        let cell_size = self.editor.scaled_edit_cell_size();
        let cell_gap = self.editor.scaled_edit_cell_gap();

        // Update hover state
        let old_hovered = state.hovered;
        if let Some(pos) = cursor_pos {
            // Calculate hover position for corner bracket effect
            let (width, height) = self.editor.font_size();
            let hover_x = ((pos.x - RULER_SIZE) / (cell_size + cell_gap)) as i32;
            let hover_y = ((pos.y - RULER_SIZE) / (cell_size + cell_gap)) as i32;

            if hover_x >= 0 && hover_x < width && hover_y >= 0 && hover_y < height {
                state.hovered = Some((hover_x, hover_y));
            } else {
                state.hovered = None;
            }
        } else {
            // Cursor left the canvas
            state.hovered = None;
        }

        // Request redraw if hover changed
        if old_hovered != state.hovered {
            // For CursorLeft events, we need to redraw even without a position
            if cursor_pos.is_none() {
                return Some(Action::request_redraw());
            }
        }

        // Early return if cursor is not over the canvas
        let cursor_pos = cursor_pos?;

        match event {
            icy_ui::Event::Mouse(mouse::Event::ButtonPressed {
                button: mouse::Button::Left, ..
            }) => Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::LeftPressed(cursor_pos)))),
            icy_ui::Event::Mouse(mouse::Event::ButtonPressed {
                button: mouse::Button::Right, ..
            }) => Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::RightPressed(cursor_pos)))),
            icy_ui::Event::Mouse(mouse::Event::ButtonPressed {
                button: mouse::Button::Middle, ..
            }) => Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::MiddlePressed))),
            icy_ui::Event::Mouse(mouse::Event::ButtonReleased {
                button: mouse::Button::Left, ..
            }) => Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::LeftReleased))),
            icy_ui::Event::Mouse(mouse::Event::ButtonReleased {
                button: mouse::Button::Right, ..
            }) => Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::RightReleased))),
            icy_ui::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Request redraw if hover changed
                if old_hovered != state.hovered {
                    return Some(Action::request_redraw());
                }
                Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::CursorMoved(cursor_pos))))
            }
            _ => None,
        }
    }
}
