//! Remote Cursors Overlay Widget
//!
//! Renders cursor positions of remote users in a collaboration session.
//! Each remote user gets a distinct color and their nickname is displayed
//! near the cursor. Uses display_scale from RenderInfo for correct zoom.
//! Supports different cursor modes: Editing, Selection, Operation.

use crate::ui::editor::ansi::AnsiEditorCoreMessage;
use icy_ui::{widget::canvas, Color, Element, Length, Renderer, Theme};
use icy_engine_gui::RenderInfo;
use parking_lot::RwLock;
use std::sync::Arc;

/// Cursor mode for remote users
#[derive(Clone, Debug, Copy, PartialEq, Eq, Default)]
pub enum RemoteCursorMode {
    /// Normal editing mode - show cursor at position
    #[default]
    Editing,
    /// Selection mode - show selection rectangle from start to current position
    Selection { start_col: i32, start_row: i32 },
    /// Operation mode - moving a floating selection block
    Operation,
    /// Cursor is hidden
    Hidden,
}

/// Remote cursor information for rendering
#[derive(Clone, Debug)]
pub struct RemoteCursor {
    /// User nickname
    pub nick: String,
    /// Cursor column position
    pub col: i32,
    /// Cursor row position
    pub row: i32,
    /// User ID (used for color generation)
    pub user_id: u32,
    /// Cursor mode
    pub mode: RemoteCursorMode,
}

/// Predefined colors for remote cursors
const CURSOR_COLORS: &[[f32; 3]] = &[
    [1.0, 0.4, 0.4], // Red
    [0.4, 1.0, 0.4], // Green
    [0.4, 0.4, 1.0], // Blue
    [1.0, 1.0, 0.4], // Yellow
    [1.0, 0.4, 1.0], // Magenta
    [0.4, 1.0, 1.0], // Cyan
    [1.0, 0.7, 0.4], // Orange
    [0.7, 0.4, 1.0], // Purple
    [0.4, 1.0, 0.7], // Mint
    [1.0, 0.4, 0.7], // Pink
];

/// Get a color for a user based on their ID
fn color_for_user(user_id: u32) -> Color {
    let idx = (user_id as usize) % CURSOR_COLORS.len();
    let [r, g, b] = CURSOR_COLORS[idx];
    Color::from_rgb(r, g, b)
}

/// Create remote cursors overlay that draws on top of the terminal
pub fn remote_cursors_overlay(
    render_info: Arc<RwLock<RenderInfo>>,
    cursors: Vec<RemoteCursor>,
    font_width: f32,
    font_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    buffer_width: usize,
    buffer_height: usize,
) -> Element<'static, AnsiEditorCoreMessage> {
    let state = RemoteCursorsOverlayState {
        render_info,
        cursors,
        font_width,
        font_height,
        scroll_x,
        scroll_y,
        buffer_width,
        buffer_height,
    };

    canvas(state).width(Length::Fill).height(Length::Fill).into()
}

struct RemoteCursorsOverlayState {
    render_info: Arc<RwLock<RenderInfo>>,
    cursors: Vec<RemoteCursor>,
    font_width: f32,
    font_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    buffer_width: usize,
    buffer_height: usize,
}

impl<Message> canvas::Program<Message> for RemoteCursorsOverlayState {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: icy_ui::Rectangle, _cursor: icy_ui::mouse::Cursor) -> Vec<canvas::Geometry> {
        if self.cursors.is_empty() {
            return vec![];
        }

        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Get the effective zoom from RenderInfo
        let zoom = {
            let info = self.render_info.read();
            if info.display_scale > 0.0 {
                info.display_scale
            } else {
                1.0
            }
        };

        // Calculate scaled font dimensions
        let scaled_font_width = self.font_width * zoom;
        let scaled_font_height = self.font_height * zoom;

        if scaled_font_width <= 0.0 || scaled_font_height <= 0.0 {
            return vec![frame.into_geometry()];
        }

        // Calculate total content size
        let content_width = self.buffer_width as f32 * scaled_font_width;
        let content_height = self.buffer_height as f32 * scaled_font_height;

        // Calculate centering offset
        let offset_x = ((bounds.width - content_width) / 2.0).max(0.0);
        let offset_y = ((bounds.height - content_height) / 2.0).max(0.0);

        // Calculate scroll offset in pixels
        let scroll_x_px = self.scroll_x * zoom;
        let scroll_y_px = self.scroll_y * zoom;

        // Draw each remote cursor
        for cursor in &self.cursors {
            // Skip hidden cursors
            if cursor.mode == RemoteCursorMode::Hidden {
                continue;
            }

            let color = color_for_user(cursor.user_id);

            // Calculate cursor position in screen coordinates
            let x = offset_x + (cursor.col as f32 * scaled_font_width) - scroll_x_px;
            let y = offset_y + (cursor.row as f32 * scaled_font_height) - scroll_y_px;

            // Skip if outside bounds
            if x < -scaled_font_width || x > bounds.width || y < -scaled_font_height || y > bounds.height {
                continue;
            }

            match cursor.mode {
                RemoteCursorMode::Selection { start_col, start_row } => {
                    // Draw selection rectangle from start to current position
                    let start_x = offset_x + (start_col as f32 * scaled_font_width) - scroll_x_px;
                    let start_y = offset_y + (start_row as f32 * scaled_font_height) - scroll_y_px;

                    let min_x = start_x.min(x);
                    let min_y = start_y.min(y);
                    let max_x = (start_x + scaled_font_width).max(x + scaled_font_width);
                    let max_y = (start_y + scaled_font_height).max(y + scaled_font_height);

                    let sel_rect = icy_ui::Rectangle {
                        x: min_x,
                        y: min_y,
                        width: max_x - min_x,
                        height: max_y - min_y,
                    };

                    // Draw selection fill (light transparent)
                    frame.fill_rectangle(
                        icy_ui::Point::new(sel_rect.x, sel_rect.y),
                        icy_ui::Size::new(sel_rect.width, sel_rect.height),
                        Color { a: 0.08, ..color },
                    );

                    // Draw outer border for selection
                    let outer_path = canvas::Path::rectangle(icy_ui::Point::new(sel_rect.x, sel_rect.y), icy_ui::Size::new(sel_rect.width, sel_rect.height));
                    frame.stroke(&outer_path, canvas::Stroke::default().with_width(2.0).with_color(Color { a: 0.9, ..color }));

                    // Draw inner border for double-line effect (to distinguish from regular cursor)
                    if sel_rect.width > 6.0 && sel_rect.height > 6.0 {
                        let inner_path = canvas::Path::rectangle(
                            icy_ui::Point::new(sel_rect.x + 3.0, sel_rect.y + 3.0),
                            icy_ui::Size::new(sel_rect.width - 6.0, sel_rect.height - 6.0),
                        );
                        frame.stroke(&inner_path, canvas::Stroke::default().with_width(1.0).with_color(Color { a: 0.6, ..color }));
                    }
                }
                RemoteCursorMode::Operation => {
                    // Draw operation cursor - double bordered rectangle
                    let cursor_rect = icy_ui::Rectangle {
                        x,
                        y,
                        width: scaled_font_width,
                        height: scaled_font_height,
                    };

                    // Inner fill
                    frame.fill_rectangle(
                        icy_ui::Point::new(cursor_rect.x, cursor_rect.y),
                        icy_ui::Size::new(cursor_rect.width, cursor_rect.height),
                        Color { a: 0.25, ..color },
                    );

                    // Inner border
                    let inner_path = canvas::Path::rectangle(
                        icy_ui::Point::new(cursor_rect.x + 2.0, cursor_rect.y + 2.0),
                        icy_ui::Size::new(cursor_rect.width - 4.0, cursor_rect.height - 4.0),
                    );
                    frame.stroke(&inner_path, canvas::Stroke::default().with_width(1.0).with_color(color));

                    // Outer border
                    let outer_path = canvas::Path::rectangle(
                        icy_ui::Point::new(cursor_rect.x, cursor_rect.y),
                        icy_ui::Size::new(cursor_rect.width, cursor_rect.height),
                    );
                    frame.stroke(&outer_path, canvas::Stroke::default().with_width(2.0).with_color(Color { a: 0.9, ..color }));
                }
                RemoteCursorMode::Editing | RemoteCursorMode::Hidden => {
                    // Normal cursor box (outline style like Moebius)
                    let cursor_rect = icy_ui::Rectangle {
                        x,
                        y,
                        width: scaled_font_width,
                        height: scaled_font_height,
                    };

                    // Draw filled rectangle with transparency
                    frame.fill_rectangle(
                        icy_ui::Point::new(cursor_rect.x, cursor_rect.y),
                        icy_ui::Size::new(cursor_rect.width, cursor_rect.height),
                        Color { a: 0.22, ..color },
                    );

                    // Draw border
                    let border_path = canvas::Path::rectangle(
                        icy_ui::Point::new(cursor_rect.x, cursor_rect.y),
                        icy_ui::Size::new(cursor_rect.width, cursor_rect.height),
                    );
                    frame.stroke(&border_path, canvas::Stroke::default().with_width(2.0).with_color(color));
                }
            }

            // Draw nickname label above cursor
            let label_y = y - 16.0;
            if label_y > 0.0 {
                // Background for label
                let text_size = 11.0;
                let label_width = cursor.nick.len() as f32 * 7.0 + 8.0;
                let label_height = 14.0;

                frame.fill_rectangle(
                    icy_ui::Point::new(x - 2.0, label_y - 2.0),
                    icy_ui::Size::new(label_width, label_height),
                    Color { a: 0.65, ..color },
                );

                // Nickname text
                frame.fill_text(canvas::Text {
                    content: cursor.nick.clone(),
                    position: icy_ui::Point::new(x + 2.0, label_y),
                    color: Color::WHITE,
                    size: text_size.into(),
                    font: icy_ui::Font::MONOSPACE,
                    align_x: icy_ui::alignment::Horizontal::Left.into(),
                    align_y: icy_ui::alignment::Vertical::Top.into(),
                    ..Default::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }
}
