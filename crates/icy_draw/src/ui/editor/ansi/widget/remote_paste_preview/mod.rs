//! Remote Paste Preview Overlay Widget
//!
//! Renders floating selection previews (Moebius PASTE_AS_SELECTION) for remote users.
//! The preview is drawn as an image (RGBA) positioned in document cell space and
//! transformed into screen space using the same math as the terminal renderer.

use crate::ui::editor::ansi::AnsiEditorCoreMessage;
use iced::advanced::image::Renderer as _;
use iced::advanced::text::Renderer as _;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::Renderer as _;
use iced::advanced::{layout, renderer, widget};
use iced::widget::image;
use iced::{Border, Color, Element, Length, Point, Rectangle, Theme};
use icy_engine_gui::RenderInfo;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct RemotePastePreview {
    pub _user_id: u32,
    pub _nick: String,
    /// Display label (e.g. "nick" or "nick <group>")
    pub label: String,
    /// Highlight color for outline/label
    pub color: Color,
    /// Top-left position in document cells.
    pub col: i32,
    pub row: i32,
    /// Pre-rendered RGBA image.
    pub handle: image::Handle,
    /// Size of the preview image in *unscaled* pixels (font cell pixels).
    pub _width_px: u32,
    pub _height_px: u32,
    /// Size in cells.
    pub columns: u32,
    pub rows: u32,
}

pub fn remote_paste_preview_overlay(
    render_info: Arc<RwLock<RenderInfo>>,
    previews: Vec<RemotePastePreview>,
    font_width: f32,
    font_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    buffer_width: usize,
    buffer_height: usize,
) -> Element<'static, AnsiEditorCoreMessage> {
    let overlay = RemotePastePreviewOverlay {
        render_info,
        previews,
        font_width,
        font_height,
        scroll_x,
        scroll_y,
        buffer_width,
        buffer_height,
    };

    iced::Element::new(overlay)
}

struct RemotePastePreviewOverlay {
    render_info: Arc<RwLock<RenderInfo>>,
    previews: Vec<RemotePastePreview>,
    font_width: f32,
    font_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    buffer_width: usize,
    buffer_height: usize,
}

impl<Message> widget::Widget<Message, Theme, iced::Renderer> for RemotePastePreviewOverlay {
    fn size(&self) -> iced::Size<Length> {
        iced::Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: layout::Layout<'_>,
        _cursor: iced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        if self.previews.is_empty() {
            return;
        }

        let bounds = layout.bounds();

        // Effective zoom (matches the terminal renderer).
        let zoom = {
            let info = self.render_info.read();
            if info.display_scale > 0.0 {
                info.display_scale
            } else {
                1.0
            }
        };

        let scaled_font_width = self.font_width * zoom;
        let scaled_font_height = self.font_height * zoom;
        if scaled_font_width <= 0.0 || scaled_font_height <= 0.0 {
            return;
        }

        // Centering offsets (same as remote cursors overlay).
        let content_width = self.buffer_width as f32 * scaled_font_width;
        let content_height = self.buffer_height as f32 * scaled_font_height;
        let offset_x = ((bounds.width - content_width) / 2.0).max(0.0);
        let offset_y = ((bounds.height - content_height) / 2.0).max(0.0);

        let scroll_x_px = self.scroll_x * zoom;
        let scroll_y_px = self.scroll_y * zoom;

        for preview in &self.previews {
            let x = offset_x + (preview.col as f32 * scaled_font_width) - scroll_x_px;
            let y = offset_y + (preview.row as f32 * scaled_font_height) - scroll_y_px;

            // Compute scaled size from cell dimensions.
            let w = preview.columns as f32 * scaled_font_width;
            let h = preview.rows as f32 * scaled_font_height;

            if w <= 0.0 || h <= 0.0 {
                continue;
            }

            // Skip if fully outside.
            if x > bounds.width || y > bounds.height || (x + w) < 0.0 || (y + h) < 0.0 {
                continue;
            }

            let img_bounds = Rectangle { x, y, width: w, height: h };
            let clip = super_intersect(bounds, img_bounds).unwrap_or(bounds);

            // Draw selection frame (fill + outline), similar to remote cursor selection.
            // Keep it subtle so the underlying preview content remains readable.
            let frame_fill = Color { a: 0.08, ..preview.color };
            let frame_border = Color { a: 0.9, ..preview.color };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: img_bounds,
                    border: Border::default().width(2.0).color(frame_border),
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                frame_fill,
            );

            if img_bounds.width > 6.0 && img_bounds.height > 6.0 {
                let inner = Rectangle {
                    x: img_bounds.x + 3.0,
                    y: img_bounds.y + 3.0,
                    width: img_bounds.width - 6.0,
                    height: img_bounds.height - 6.0,
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: inner,
                        border: Border::default().width(1.0).color(Color { a: 0.6, ..preview.color }),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                    Color { a: 0.0, ..preview.color },
                );
            }

            let image = iced::advanced::image::Image::<image::Handle> {
                handle: preview.handle.clone(),
                filter_method: iced::advanced::image::FilterMethod::Nearest,
                rotation: iced::Radians(0.0),
                // Slightly transparent so the preview isn't overpowering.
                opacity: 0.65,
                snap: true,
                border_radius: iced::border::Radius::default(),
            };

            renderer.draw_image(image, img_bounds, clip);

            // Label above the selection
            let label_y = y - 16.0;
            if label_y > 0.0 {
                let text_size = 11.0;
                let label_width = preview.label.len() as f32 * 7.0 + 8.0;
                let label_height = 14.0;
                let label_bounds = Rectangle {
                    x: x - 2.0,
                    y: label_y - 2.0,
                    width: label_width,
                    height: label_height,
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: label_bounds,
                        border: Border::default().width(0.0),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                    Color { a: 0.65, ..preview.color },
                );

                let text = iced::advanced::text::Text {
                    content: preview.label.clone(),
                    bounds: label_bounds.size(),
                    size: iced::Pixels(text_size),
                    line_height: iced::advanced::text::LineHeight::Relative(1.0),
                    font: iced::Font::MONOSPACE,
                    align_x: iced::advanced::text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: iced::advanced::text::Shaping::Advanced,
                    wrapping: iced::advanced::text::Wrapping::None,
                    hint_factor: Some(0.0),
                };

                renderer.fill_text(text, Point::new(x + 2.0, label_y), Color::WHITE, bounds);
            }
        }
    }

    fn children(&self) -> Vec<Tree> {
        Vec::new()
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }
}

impl<'a, Message> From<RemotePastePreviewOverlay> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(widget: RemotePastePreviewOverlay) -> Self {
        Element::new(widget)
    }
}

fn super_intersect(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);

    let w = x1 - x0;
    let h = y1 - y0;

    if w <= 0.0 || h <= 0.0 {
        None
    } else {
        Some(Rectangle {
            x: x0,
            y: y0,
            width: w,
            height: h,
        })
    }
}
