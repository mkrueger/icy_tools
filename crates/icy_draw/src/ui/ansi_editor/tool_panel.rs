//! Tool panel component
//!
//! Shows the 9 tool icons in a grid (3 columns × 3 rows).
//! Each icon can represent a toggle pair - clicking an already-selected tool
//! switches to its partner.

use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse, widget::canvas::{Canvas, Frame, Geometry, Path, Program, Stroke, Action},
};
use icy_engine_edit::tools::{Tool, TOOL_SLOTS, get_slot_display_tool, click_tool_slot};

/// Size of each tool icon
const ICON_SIZE: f32 = 24.0;
/// Padding between icons
const ICON_PADDING: f32 = 2.0;
/// Number of columns in the grid
const COLS: usize = 3;
/// Number of rows in the grid
const ROWS: usize = 3;

/// Messages from the tool panel
#[derive(Clone, Debug)]
pub enum ToolPanelMessage {
    /// Clicked on a tool slot
    ClickSlot(usize),
}

/// Tool panel state
pub struct ToolPanel {
    /// Currently selected tool
    current_tool: Tool,
}

impl Default for ToolPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolPanel {
    pub fn new() -> Self {
        Self {
            current_tool: Tool::Click,
        }
    }

    /// Get the current tool
    pub fn current_tool(&self) -> Tool {
        self.current_tool
    }

    /// Set the current tool
    pub fn set_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
    }

    /// Update the tool panel state
    pub fn update(&mut self, message: ToolPanelMessage) -> iced::Task<ToolPanelMessage> {
        match message {
            ToolPanelMessage::ClickSlot(slot) => {
                self.current_tool = click_tool_slot(slot, self.current_tool);
            }
        }
        iced::Task::none()
    }

    /// Render the tool panel
    pub fn view(&self) -> Element<'_, ToolPanelMessage> {
        let total_width = COLS as f32 * (ICON_SIZE + ICON_PADDING) + ICON_PADDING;
        let total_height = ROWS as f32 * (ICON_SIZE + ICON_PADDING) + ICON_PADDING;

        Canvas::new(ToolPanelProgram {
            current_tool: self.current_tool,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into()
    }
}

/// Canvas program for drawing the tool panel
struct ToolPanelProgram {
    current_tool: Tool,
}

impl Program<ToolPanelMessage> for ToolPanelProgram {
    type State = Option<usize>;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        for slot_idx in 0..TOOL_SLOTS.len() {
            let row = slot_idx / COLS;
            let col = slot_idx % COLS;
            
            let x = ICON_PADDING + col as f32 * (ICON_SIZE + ICON_PADDING);
            let y = ICON_PADDING + row as f32 * (ICON_SIZE + ICON_PADDING);
            
            let display_tool = get_slot_display_tool(slot_idx, self.current_tool);
            let is_selected = TOOL_SLOTS[slot_idx].contains(self.current_tool);
            let is_hovered = *state == Some(slot_idx);

            // Draw background
            let bg_color = if is_selected {
                Color::from_rgb8(80, 120, 180)
            } else if is_hovered {
                Color::from_rgb8(70, 70, 80)
            } else {
                Color::from_rgb8(50, 50, 60)
            };

            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(ICON_SIZE, ICON_SIZE),
                bg_color,
            );

            // Draw border
            let border_color = if is_selected {
                Color::from_rgb8(120, 160, 220)
            } else {
                Color::from_rgb8(80, 80, 90)
            };

            let border_path = Path::rectangle(
                Point::new(x, y),
                Size::new(ICON_SIZE, ICON_SIZE),
            );

            frame.stroke(
                &border_path,
                Stroke::default()
                    .with_color(border_color)
                    .with_width(1.0),
            );

            // Draw icon representation (simple text for now)
            // In a real implementation, you'd load SVG icons here
            let icon_char = match display_tool {
                Tool::Click => "→",
                Tool::Select => "▢",
                Tool::Pencil => "✎",
                Tool::Line => "╱",
                Tool::Brush => "◉",
                Tool::Erase => "⌫",
                Tool::RectangleOutline => "□",
                Tool::RectangleFilled => "■",
                Tool::EllipseOutline => "○",
                Tool::EllipseFilled => "●",
                Tool::Fill => "◧",
                Tool::Pipette => "◎",
                Tool::Shifter => "↔",
                Tool::Font => "A",
                Tool::Tag => "⚑",
            };

            // Draw centered text
            let text_color = if is_selected {
                Color::WHITE
            } else {
                Color::from_rgb8(200, 200, 200)
            };

            frame.fill_text(iced::widget::canvas::Text {
                content: icon_char.to_string(),
                position: Point::new(x + ICON_SIZE / 2.0, y + ICON_SIZE / 2.0 + 4.0),
                color: text_color,
                size: iced::Pixels(14.0),
                ..Default::default()
            });
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<ToolPanelMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let col = ((pos.x - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)) as usize;
                    let row = ((pos.y - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)) as usize;
                    
                    if col < COLS && row < ROWS {
                        let slot = row * COLS + col;
                        if slot < TOOL_SLOTS.len() {
                            *state = Some(slot);
                            return Some(Action::request_redraw());
                        }
                    }
                }
                *state = None;
                Some(Action::request_redraw())
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let col = ((pos.x - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)) as usize;
                    let row = ((pos.y - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)) as usize;
                    
                    if col < COLS && row < ROWS {
                        let slot = row * COLS + col;
                        if slot < TOOL_SLOTS.len() {
                            return Some(Action::publish(ToolPanelMessage::ClickSlot(slot)));
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}
