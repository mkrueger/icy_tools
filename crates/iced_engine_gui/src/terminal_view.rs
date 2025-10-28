use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, renderer};
use iced::keyboard::Modifiers;
use iced::mouse::Cursor;
use iced::widget::canvas::{Frame, Geometry, Path};
use iced::{Color, Element, Length, Point, Rectangle, Size};
use iced_core::Renderer as CoreRenderer; // Add this for fill_quad
use iced_core::text::LineHeight;
use iced_core::text::Renderer as TextRenderer; // Add this for fill_text
use icy_engine::{Position, TextPane};

use crate::Terminal;

#[derive(Debug, Clone)]
pub enum Message {
    SetCaret(Position),
    BufferChanged,
    Resize(i32, i32),
}

pub struct TerminalView<'a> {
    term: &'a Terminal,
}

impl<'a> TerminalView<'a> {
    pub fn new(term: &'a Terminal) -> Self {
        Self { term }
    }

    pub fn show(term: &'a Terminal) -> Element<'a, Message> {
        Element::new(Self { term })
    }

    pub fn invalidate_cache(&self) {
        self.term.cache.clear();
    }

    fn is_cursor_in_layout(&self, cursor: Cursor, layout: Layout<'_>) -> bool {
        if let Some(cursor_position) = cursor.position() {
            let bounds = layout.bounds();
            cursor_position.x >= bounds.x
                && cursor_position.y >= bounds.y
                && cursor_position.x < (bounds.x + bounds.width)
                && cursor_position.y < (bounds.y + bounds.height)
        } else {
            false
        }
    }

    fn draw_cached(&self, renderer: &iced::Renderer, bounds: Rectangle, viewport: &Rectangle) -> Geometry {
        self.term
            .cache
            .draw(renderer, bounds.size(), |frame| self.draw_to_frame(frame, bounds, viewport))
    }

    fn draw_to_frame(&self, frame: &mut Frame, bounds: Rectangle, _viewport: &Rectangle) {
        let state = match self.term.edit_state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };

        let buffer = state.get_buffer();
        let char_w = self.term.char_width;
        let char_h = self.term.char_height;
        let selection_opt = state.get_selection();

        // Background batching - now considering selection
        for y in 0..buffer.get_height() {
            let mut x = 0;
            while x < buffer.get_width() {
                let pos = Position::new(x, y);
                let ch_attr = buffer.get_char(pos);

                // Check if position is selected
                let is_selected = selection_opt.as_ref().map_or(false, |sel| sel.is_inside(pos));
                // Get colors (swap if selected)
                let bg_idx = if is_selected {
                    ch_attr.attribute.get_foreground()
                } else {
                    ch_attr.attribute.get_background()
                };

                let mut run_len = 1;
                // Find consecutive cells with same background and selection state
                while x + run_len < buffer.get_width() {
                    let next_pos = Position::new(x + run_len, y);
                    let next_attr = buffer.get_char(next_pos);
                    let next_selected = selection_opt.as_ref().map_or(false, |sel| sel.is_inside(next_pos));

                    let next_bg_idx = if next_selected {
                        next_attr.attribute.get_foreground()
                    } else {
                        next_attr.attribute.get_background()
                    };

                    if next_bg_idx != bg_idx || next_selected != is_selected {
                        break;
                    }
                    run_len += 1;
                }

                let rect = Path::rectangle(
                    Point::new(bounds.x + (x as f32 * char_w), bounds.y + (y as f32 * char_h)),
                    Size::new(run_len as f32 * char_w, char_h),
                );
                let color = Terminal::buffer_color_to_iced(buffer.palette.get_color(bg_idx));
                frame.fill(&rect, color);

                x += run_len;
            }
        }

        // Text cells - now considering selection
        for y in 0..buffer.get_height() {
            for x in 0..buffer.get_width() {
                let pos = Position::new(x, y);
                let ch_attr = buffer.get_char(pos);
                let ch = ch_attr.ch;

                // Check if position is selected
                let is_selected = selection_opt.as_ref().map_or(false, |sel| sel.is_inside(pos));

                // Get foreground color (swap if selected)
                let fg_idx = if is_selected {
                    ch_attr.attribute.get_background()
                } else {
                    ch_attr.attribute.get_foreground()
                };
                let color = Terminal::buffer_color_to_iced(buffer.palette.get_color(fg_idx));

                frame.fill_text(iced::widget::canvas::Text {
                    content: ch.to_string(),
                    position: Point::new(bounds.x + (x as f32 * char_w + char_w / 2.0), bounds.y + (y as f32 * char_h + char_h / 2.0)),
                    color,
                    size: iced::Pixels(self.term.font_size),
                    font: iced::Font::MONOSPACE,
                    max_width: char_w,
                    line_height: LineHeight::default(),
                    align_x: iced_core::text::Alignment::Center,
                    align_y: iced::alignment::Vertical::Center,
                    shaping: iced_core::text::Shaping::Basic,
                });
            }
        }

        // Caret
        let caret_pos = state.get_caret().get_position();
        if caret_pos.y >= 0 && caret_pos.y < buffer.get_height() && caret_pos.x >= 0 && caret_pos.x < buffer.get_width() {
            let caret_rect = Path::rectangle(
                Point::new(bounds.x + (caret_pos.x as f32 * char_w), bounds.y + (caret_pos.y as f32 * char_h)),
                Size::new(2.0, char_h),
            );
            frame.fill(&caret_rect, Color::WHITE);
        }
    }

    #[allow(dead_code)]
    fn draw_direct(&self, renderer: &mut iced::Renderer, bounds: Rectangle, viewport: &Rectangle) {
        let state = match self.term.edit_state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        let buffer = state.get_buffer();
        let start_row = 0;
        let end_row = buffer.get_height() - 1;
        let start_col = 0;
        let end_col = buffer.get_width() - 1;

        // Background batching
        for y in start_row..=end_row {
            let mut x = start_col;
            while x <= end_col {
                let pos = Position::new(x, y);
                let ch_attr = buffer.get_char(pos);
                let bg_idx = ch_attr.attribute.get_background();
                if bg_idx != 0 {
                    let mut run_end = x + 1;
                    while run_end <= end_col {
                        let next_attr = buffer.get_char(Position::new(run_end, y));
                        if next_attr.attribute.get_background() != bg_idx {
                            break;
                        }
                        run_end += 1;
                    }
                    let rect = Rectangle {
                        x: bounds.x + (x as f32 * self.term.char_width),
                        y: bounds.y + (y as f32 * self.term.char_height),
                        width: ((run_end - x) as f32 * self.term.char_width),
                        height: self.term.char_height,
                    };
                    let color = Terminal::buffer_color_to_iced(buffer.palette.get_color(bg_idx));
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: rect,
                            border: Default::default(),
                            shadow: Default::default(),
                            ..Default::default()
                        },
                        color,
                    );
                    x = run_end;
                } else {
                    x += 1;
                }
            }
        }

        // Text
        for y in start_row..=end_row {
            for x in start_col..=end_col {
                let pos = Position::new(x, y);
                let ch_attr = buffer.get_char(pos);
                let ch = ch_attr.ch;
                if ch == ' ' || ch == '\0' {
                    continue;
                }
                let fg_idx = ch_attr.attribute.get_foreground();
                let color = Terminal::buffer_color_to_iced(buffer.palette.get_color(fg_idx));
                renderer.fill_text(
                    iced::advanced::text::Text {
                        content: ch.to_string(),
                        bounds: Size::new(self.term.char_width, self.term.char_height),
                        size: iced::Pixels(self.term.font_size),
                        font: iced::Font::MONOSPACE,
                        line_height: iced::advanced::text::LineHeight::default(),
                        shaping: iced::advanced::text::Shaping::Basic,
                        wrapping: iced::advanced::text::Wrapping::None,
                        align_x: iced_core::text::Alignment::Center,
                        align_y: iced::alignment::Vertical::Center,
                    },
                    Point::new(bounds.x + (x as f32 * self.term.char_width), bounds.y + (y as f32 * self.term.char_height)),
                    color,
                    *viewport,
                );
            }
        }

        // Caret
        let caret_pos = state.get_caret().get_position();
        if caret_pos.x >= start_col && caret_pos.x <= end_col && caret_pos.y >= start_row && caret_pos.y <= end_row {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + (caret_pos.x as f32 * self.term.char_width),
                        y: bounds.y + (caret_pos.y as f32 * self.term.char_height),
                        width: 2.0,
                        height: self.term.char_height,
                    },
                    border: Default::default(),
                    shadow: Default::default(),
                    ..Default::default()
                },
                Color::WHITE,
            );
        }
    }
}

impl<'a, Theme> Widget<Message, Theme, iced::Renderer> for TerminalView<'a>
where
    Theme: 'a,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let state = self.term.edit_state.lock().unwrap();
        let buffer = state.get_buffer();
        let size = limits.resolve(
            Length::Fill,
            Length::Fill,
            Size::new(
                buffer.get_width() as f32 * self.term.char_width,
                buffer.get_height() as f32 * self.term.char_height,
            ),
        );
        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        use iced::advanced::graphics::geometry::Renderer as _;
        let geom = self.draw_cached(renderer, bounds, viewport);
        renderer.draw_geometry(geom);
    }

    fn update(
        &mut self,
        _state: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let iced::Event::Mouse(mouse_event) = event {
            if self.is_cursor_in_layout(cursor, layout) {
                if let mouse::Event::ButtonPressed(mouse::Button::Left) = mouse_event {
                    if let Some(position) = cursor.position() {
                        let bounds = layout.bounds();
                        let x = ((position.x - bounds.x) / self.term.char_width) as i32;
                        let y = ((position.y - bounds.y) / self.term.char_height) as i32;
                        shell.publish(Message::SetCaret(Position::new(x, y)));
                    }
                }
            }
        }
    }
}

impl<'a, Theme> From<TerminalView<'a>> for Element<'a, Message, Theme, iced::Renderer>
where
    Theme: 'a,
{
    fn from(view: TerminalView<'a>) -> Self {
        Element::new(view)
    }
}

#[derive(Debug, Clone, Default)]
struct TerminalViewState {
    is_focused: bool,
    is_dragged: bool,
    scroll_pixels: f32,
    keyboard_modifiers: Modifiers,
    size: Size<f32>,
    mouse_position_on_grid: Position,
}
