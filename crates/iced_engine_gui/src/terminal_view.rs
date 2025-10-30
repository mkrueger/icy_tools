#![allow(static_mut_refs)]
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, renderer};
use iced::mouse::Cursor;
use iced::widget::canvas::{Frame, Geometry, Path};
use iced::{Color, Element, Length, Point, Rectangle, Size};
use iced_core::text::LineHeight;
use icy_engine::{Position, TextPane};

use crate::{MonitorSettings, Terminal};

#[derive(Debug, Clone)]
pub enum Message {
    SetCaret(Position),
    BufferChanged,
    Resize(i32, i32),
}

pub struct TerminalView<'a> {
    term: &'a Terminal,
    settings: MonitorSettings,
}

impl<'a> TerminalView<'a> {
    pub fn new(term: &'a Terminal, settings: MonitorSettings) -> Self {
        Self { term, settings }
    }

    pub fn show(term: &'a Terminal, settings: MonitorSettings) -> Element<'a, Message> {
        Element::new(Self { term, settings })
    }

    pub fn show_with_effects(term: &'a Terminal, settings: MonitorSettings) -> Element<'a, Message> {
        if !matches!(term.edit_state.lock().unwrap().get_buffer().buffer_type, icy_engine::BufferType::Unicode) {
            iced::widget::container(crate::terminal_shader::create_crt_shader(term, settings)).into()
        } else {
            // Only use direct rendering for Color mode with no filter
            Element::new(Self { term, settings })
        }
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

        self.draw_unicode_buffer(frame, bounds, &state);
    }

    fn draw_unicode_buffer(&self, frame: &mut Frame, bounds: Rectangle, state: &icy_engine::editor::EditState) {
        // Your existing Unicode rendering code
        let buffer = state.get_buffer();
        let buffer_width = buffer.get_width();
        let buffer_height = buffer.get_height();

        // Base font metrics (for reference monospace font)
        let base_char_w = self.term.char_width;
        let base_char_h = self.term.char_height;
        let base_font_size = self.term.font_size;

        // Calculate the scale factor to fit the terminal in the available space
        let scale_x = bounds.width / (buffer_width as f32 * base_char_w);
        let scale_y = bounds.height / (buffer_height as f32 * base_char_h);
        let scale = scale_x.min(scale_y); // Use uniform scaling to maintain aspect ratio

        // Calculate scaled font size and derive character dimensions from it
        let font_size = (base_font_size * scale).max(6.0); // Minimum 6pt font

        // For monospace fonts, char dimensions scale proportionally with font size
        let font_scale = font_size / base_font_size;
        let char_w = base_char_w * font_scale;
        let char_h = base_char_h * font_scale;

        let selection_opt = state.get_selection();

        // Background batching - now considering selection
        for y in 0..buffer_height {
            let mut x = 0;
            while x < buffer_width {
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
                while x + run_len < buffer_width {
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
        for y in 0..buffer_height {
            for x in 0..buffer_width {
                let pos = Position::new(x, y);
                let ch_attr = buffer.get_char(pos);
                let ch = ch_attr.ch;

                // Skip null characters
                if ch == '\0' || ch == ' ' {
                    continue;
                }

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
                    size: iced::Pixels(font_size),
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
        if caret_pos.y >= 0 && caret_pos.y < buffer_height && caret_pos.x >= 0 && caret_pos.x < buffer_width {
            let caret_rect = Path::rectangle(
                Point::new(bounds.x + (caret_pos.x as f32 * char_w), bounds.y + (caret_pos.y as f32 * char_h)),
                Size::new(2.0, char_h),
            );
            frame.fill(&caret_rect, Color::WHITE);
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

/*
#[derive(Debug, Clone, Default)]
struct TerminalViewState {
    is_focused: bool,
    is_dragged: bool,
    scroll_pixels: f32,
    keyboard_modifiers: Modifiers,
    size: Size<f32>,
    mouse_position_on_grid: Position,
}*/
