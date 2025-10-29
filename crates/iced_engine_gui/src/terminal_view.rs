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
        // Always use shader if we need color conversion OR CRT effects
        if settings.use_filter || settings.monitor_type != crate::MonitorType::Color {
            // Wrap the shader in a container that fills available space
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

        let buffer = state.get_buffer();

        // Check buffer type - use bitmap rendering for non-Unicode buffers
        if !matches!(buffer.buffer_type, icy_engine::BufferType::Unicode) {
            if self.settings.use_pixel_perfect_scaling {
                self.draw_bitmap_buffer_pixel_correct(frame, bounds, &state);
            } else {
                self.draw_bitmap_buffer_smooth(frame, bounds, &state);
            }
        } else {
            self.draw_unicode_buffer(frame, bounds, &state);
        }
    }

    fn draw_bitmap_buffer_smooth(&self, frame: &mut Frame, bounds: Rectangle, state: &icy_engine::editor::EditState) {
        let buffer = state.get_buffer();
        let buffer_width = buffer.get_width();
        let buffer_height = buffer.get_height();

        // Create a rectangle covering the entire buffer
        let rect = icy_engine::Rectangle {
            start: Position::new(0, 0),
            size: icy_engine::Size::new(buffer_width, buffer_height),
        };

        // Render buffer to RGBA using bitmap fonts
        let blink_on = true; // TODO: Add blink animation support
        let (img_size, rgba_data) = buffer.render_to_rgba(rect, blink_on);

        // Create an image handle from the RGBA data
        // This is much faster than drawing individual pixels
        let handle = iced::widget::image::Handle::from_rgba(
            img_size.width as u32,
            img_size.height as u32,
            rgba_data, // Unfortunately needs a clone, but still faster than pixel-by-pixel
        );

        // Calculate scale to fit the image in the available bounds
        let scale_x = bounds.width / img_size.width as f32;
        let scale_y = bounds.height / img_size.height as f32;
        let scale = scale_x.min(scale_y);

        let scaled_width = img_size.width as f32 * scale;
        let scaled_height = img_size.height as f32 * scale;

        // Center the scaled image
        let offset_x = (bounds.width - scaled_width) / 2.0;
        let offset_y = (bounds.height - scaled_height) / 2.0;

        // Draw the image as a single operation
        frame.draw_image(
            Rectangle {
                x: bounds.x + offset_x,
                y: bounds.y + offset_y,
                width: scaled_width,
                height: scaled_height,
            },
            &handle,
        );

        // Draw caret overlay
        let caret_pos = state.get_caret().get_position();
        if let Some(font) = buffer.get_font(0) {
            let font_width = font.size.width as f32;
            let font_height = font.size.height as f32;

            if caret_pos.y >= 0 && caret_pos.y < buffer_height && caret_pos.x >= 0 && caret_pos.x < buffer_width {
                let caret_rect = Path::rectangle(
                    Point::new(
                        bounds.x + offset_x + (caret_pos.x as f32 * font_width * scale),
                        bounds.y + offset_y + (caret_pos.y as f32 * font_height * scale),
                    ),
                    Size::new(2.0, font_height * scale),
                );
                frame.fill(&caret_rect, Color::from_rgba(1.0, 1.0, 1.0, 0.8));
            }
        }
    }

    fn draw_bitmap_buffer_pixel_correct(&self, frame: &mut Frame, bounds: Rectangle, state: &icy_engine::editor::EditState) {
        let buffer = state.get_buffer();
        let buffer_width = buffer.get_width();
        let buffer_height = buffer.get_height();

        let rect = icy_engine::Rectangle {
            start: Position::new(0, 0),
            size: icy_engine::Size::new(buffer_width, buffer_height),
        };

        let blink_on = true;
        let (img_size, rgba_data) = buffer.render_to_rgba(rect, blink_on);

        // Compute uniform (float) scale fitting the available bounds.
        let scale_x = bounds.width / img_size.width as f32;
        let scale_y = bounds.height / img_size.height as f32;
        let uniform_scale = scale_x.min(scale_y);

        // Integer scale for crisp pixelated enlargement (>=1).
        let int_scale = uniform_scale.floor().max(1.0);

        // If we are not actually scaling up (scale < 2 and floor == 1) or the scale is already integer,
        // just use existing path (single texture draw) to avoid extra allocation.
        let use_integer_upscale = int_scale > 1.0 && (uniform_scale - int_scale).abs() > f32::EPSILON;

        let (final_handle, scaled_width_f, scaled_height_f, effective_scale) = if use_integer_upscale {
            // Perform nearest-neighbor replication.
            let src_w = img_size.width as usize;
            let src_h = img_size.height as usize;
            let s = int_scale as usize;
            let dst_w = src_w * s;
            let dst_h = src_h * s;

            // Guard against excessive memory use (optional threshold).
            if dst_w * dst_h > 16_000_000 {
                // Fallback to original (too large).
                let handle = iced::widget::image::Handle::from_rgba(img_size.width as u32, img_size.height as u32, rgba_data.clone());
                (
                    handle,
                    (img_size.width as f32) * uniform_scale,
                    (img_size.height as f32) * uniform_scale,
                    uniform_scale,
                )
            } else {
                let mut enlarged = vec![0u8; dst_w * dst_h * 4];
                for sy in 0..src_h {
                    for sx in 0..src_w {
                        let src_idx = (sy * src_w + sx) * 4;
                        let pixel = &rgba_data[src_idx..src_idx + 4];
                        // Replicate into s x s block.
                        let base_y = sy * s;
                        let base_x = sx * s;
                        for oy in 0..s {
                            let row_start = (base_y + oy) * dst_w;
                            for ox in 0..s {
                                let dst_idx = (row_start + base_x + ox) * 4;
                                enlarged[dst_idx..dst_idx + 4].copy_from_slice(pixel);
                            }
                        }
                    }
                }

                let handle = iced::widget::image::Handle::from_rgba(dst_w as u32, dst_h as u32, enlarged);

                (
                    handle,
                    dst_w as f32, // draw 1:1
                    dst_h as f32,
                    int_scale, // caret & font scale
                )
            }
        } else {
            // Original path (still may look slightly smoothed when scale not integer).
            let scaled_w = img_size.width as f32 * uniform_scale;
            let scaled_h = img_size.height as f32 * uniform_scale;
            let handle = iced::widget::image::Handle::from_rgba(img_size.width as u32, img_size.height as u32, rgba_data.clone());
            (handle, scaled_w, scaled_h, uniform_scale)
        };

        // Center inside bounds
        let offset_x = (bounds.width - scaled_width_f) / 2.0;
        let offset_y = (bounds.height - scaled_height_f) / 2.0;

        frame.draw_image(
            Rectangle {
                x: bounds.x + offset_x,
                y: bounds.y + offset_y,
                width: scaled_width_f,
                height: scaled_height_f,
            },
            &final_handle,
        );

        // Caret overlay
        let caret_pos = state.get_caret().get_position();
        if let Some(font) = buffer.get_font(0) {
            let font_width = font.size.width as f32;
            let font_height = font.size.height as f32;

            if caret_pos.y >= 0 && caret_pos.y < buffer_height && caret_pos.x >= 0 && caret_pos.x < buffer_width {
                // Use effective_scale (integer or uniform) for caret positioning
                let caret_rect = Path::rectangle(
                    Point::new(
                        bounds.x + offset_x + (caret_pos.x as f32 * font_width * effective_scale),
                        bounds.y + offset_y + (caret_pos.y as f32 * font_height * effective_scale),
                    ),
                    Size::new(2.0, font_height * effective_scale),
                );
                frame.fill(&caret_rect, Color::from_rgba(1.0, 1.0, 1.0, 0.8));
            }
        }
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
