use std::sync::{Arc, Mutex};

use iced::{
    Color, Element, Event, Length, Point, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Shell, layout, mouse, renderer,
        widget::{self, Widget},
    },
};
use icy_engine::editor::EditState;
use icy_engine::{Position, TextPane};

#[derive(Clone)]
pub struct Scene {
    pub edit_state: Arc<Mutex<EditState>>,
    font_size: f32,
    char_width: f32,
    char_height: f32,
}

impl Scene {
    pub fn new(edit_state: Arc<Mutex<EditState>>) -> Self {
        Self {
            edit_state,
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::SetCaret(pos) => {
                if let Ok(mut state) = self.edit_state.lock() {
                    state.get_caret_mut().set_position(pos);
                }
            } // Add more messages as needed
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SetCaret(Position),
    // Add more messages as needed
}

// Implement Widget for &Scene (immutable reference)
impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for &Scene
where
    Renderer: renderer::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
    Theme: iced::widget::text::Catalog,
{
    fn size(&self) -> Size<Length> {
        let state = self.edit_state.lock().unwrap();
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, _tree: &mut widget::Tree, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let state = self.edit_state.lock().unwrap();
        let buffer = state.get_buffer();
        let size = limits.resolve(
            Length::Fill,
            Length::Fill,
            Size::new(buffer.get_width() as f32 * self.char_width, buffer.get_height() as f32 * self.char_height),
        );

        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = self.edit_state.lock().unwrap();
        let buffer = state.get_buffer();

        // Calculate visible area
        let start_row = ((viewport.y - bounds.y) / self.char_height).max(0.0) as i32;
        let end_row = ((viewport.y + viewport.height - bounds.y) / self.char_height).ceil() as i32;
        let start_col = ((viewport.x - bounds.x) / self.char_width).max(0.0) as i32;
        let end_col = ((viewport.x + viewport.width - bounds.x) / self.char_width).ceil() as i32;

        // Render visible characters
        for y in start_row..=end_row.min(buffer.get_height() - 1) {
            for x in start_col..=end_col.min(buffer.get_width() - 1) {
                let pos = Position::new(x, y);

                // Get character directly from buffer (no Result/Option wrapping)
                let ch_attr = buffer.get_char(pos);
                let ch = ch_attr.ch;

                let x_pos = bounds.x + (x as f32 * self.char_width);
                let y_pos = bounds.y + (y as f32 * self.char_height);

                // Get colors from buffer's palette
                let fg_color = buffer_color_to_iced(buffer.palette.get_color(ch_attr.attribute.get_foreground()));
                let bg_color = buffer_color_to_iced(buffer.palette.get_color(ch_attr.attribute.get_background()));

                // Draw background if not default
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: x_pos,
                            y: y_pos,
                            width: self.char_width,
                            height: self.char_height,
                        },
                        border: Default::default(),
                        shadow: Default::default(),
                        snap: false,
                    },
                    bg_color,
                );

                let text_string = ch.to_string();
                renderer.fill_text(
                    iced::advanced::text::Text {
                        content: text_string,
                        bounds: Size::new(self.char_width, self.char_height),
                        size: iced::Pixels(self.font_size),
                        font: iced::Font::MONOSPACE,
                        align_x: iced::alignment::Horizontal::Left.into(),
                        align_y: iced::alignment::Vertical::Top,
                        line_height: iced::advanced::text::LineHeight::default(),
                        shaping: iced::advanced::text::Shaping::Basic,
                        wrapping: iced::advanced::text::Wrapping::None,
                    },
                    Point::new(x_pos, y_pos),
                    fg_color,
                    viewport.clone(),
                );
            }
        }

        // Draw caret
        let caret_pos = state.get_caret().get_position();
        let caret_x = bounds.x + (caret_pos.x as f32 * self.char_width);
        let caret_y = bounds.y + (caret_pos.y as f32 * self.char_height);

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: caret_x,
                    y: caret_y,
                    width: 2.0,
                    height: self.char_height,
                },
                border: Default::default(),
                shadow: Default::default(),
                snap: false,
            },
            Color::from_rgb(1.0, 1.0, 1.0),
        );
    }

    fn update(
        &mut self,
        _tree: &mut widget::Tree,
        _event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        // Since this is &Scene (immutable reference passed to the widget),
        // but this method receives &mut self (mutable reference to the widget itself),
        // we can't actually mutate the Scene here.
        // Events should be handled at the application level.
    }
}

// Implement Widget for &mut Scene as well (for mutable contexts)
impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for &mut Scene
where
    Renderer: renderer::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
    Theme: iced::widget::text::Catalog,
{
    fn size(&self) -> Size<Length> {
        let state = self.edit_state.lock().unwrap();
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, _tree: &mut widget::Tree, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let state = self.edit_state.lock().unwrap();
        let buffer = state.get_buffer();
        let size = limits.resolve(
            Length::Fill,
            Length::Fill,
            Size::new(buffer.get_width() as f32 * self.char_width, buffer.get_height() as f32 * self.char_height),
        );

        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = self.edit_state.lock().unwrap();
        let buffer = state.get_buffer();

        // Calculate visible area
        let start_row = ((viewport.y - bounds.y) / self.char_height).max(0.0) as i32;
        let end_row = ((viewport.y + viewport.height - bounds.y) / self.char_height).ceil() as i32;
        let start_col = ((viewport.x - bounds.x) / self.char_width).max(0.0) as i32;
        let end_col = ((viewport.x + viewport.width - bounds.x) / self.char_width).ceil() as i32;

        // Render visible characters
        for y in start_row..=end_row.min(buffer.get_height() - 1) {
            for x in start_col..=end_col.min(buffer.get_width() - 1) {
                let pos = Position::new(x, y);

                // Get character directly from buffer (no Result/Option wrapping)
                let ch_attr = buffer.get_char(pos);
                let ch = ch_attr.ch;

                if ch != ' ' && ch != '\0' {
                    let x_pos = bounds.x + (x as f32 * self.char_width);
                    let y_pos = bounds.y + (y as f32 * self.char_height);

                    // Get colors from buffer's palette
                    let fg_color = buffer_color_to_iced(buffer.palette.get_color(ch_attr.attribute.get_foreground()));
                    let bg_color = buffer_color_to_iced(buffer.palette.get_color(ch_attr.attribute.get_background()));

                    // Draw background if not default
                    if ch_attr.attribute.get_background() != 0 {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle {
                                    x: x_pos,
                                    y: y_pos,
                                    width: self.char_width,
                                    height: self.char_height,
                                },
                                border: Default::default(),
                                shadow: Default::default(),
                                snap: false,
                            },
                            bg_color,
                        );
                    }

                    // Convert ch (u16) to char for rendering
                    let text_string = ch.to_string();
                    renderer.fill_text(
                        iced::advanced::text::Text {
                            content: text_string,
                            bounds: Size::new(self.char_width, self.char_height),
                            size: iced::Pixels(self.font_size),
                            font: iced::Font::MONOSPACE,
                            align_x: iced::alignment::Horizontal::Left.into(),
                            align_y: iced::alignment::Vertical::Top,
                            line_height: iced::advanced::text::LineHeight::default(),
                            shaping: iced::advanced::text::Shaping::Basic,
                            wrapping: iced::advanced::text::Wrapping::None,
                        },
                        Point::new(x_pos, y_pos),
                        fg_color,
                        viewport.clone(),
                    );
                }
            }
        }

        // Draw caret
        let caret_pos = state.get_caret().get_position();
        let caret_x = bounds.x + (caret_pos.x as f32 * self.char_width);
        let caret_y = bounds.y + (caret_pos.y as f32 * self.char_height);

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: caret_x,
                    y: caret_y,
                    width: 2.0,
                    height: self.char_height,
                },
                border: Default::default(),
                shadow: Default::default(),
                snap: false,
            },
            Color::from_rgb(1.0, 1.0, 1.0),
        );
    }

    fn update(
        &mut self,
        _tree: &mut widget::Tree,
        _event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        // Event handling can be done here with &mut self
        // For now, we'll leave it empty as events should be handled at the app level
    }
}

// Helper function to convert attribute color to iced Color
fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
    let (r, g, b) = color.get_rgb_f32();
    Color::from_rgb(r, g, b)
}

// Implement From for &Scene
impl<'a, Message, Theme, Renderer> From<&'a Scene> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + iced::widget::text::Catalog,
    Renderer: 'a + renderer::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn from(scene: &'a Scene) -> Self {
        Element::new(scene)
    }
}

// Implement From for &mut Scene
impl<'a, Message, Theme, Renderer> From<&'a mut Scene> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + iced::widget::text::Catalog,
    Renderer: 'a + renderer::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn from(scene: &'a mut Scene) -> Self {
        Element::new(scene)
    }
}
