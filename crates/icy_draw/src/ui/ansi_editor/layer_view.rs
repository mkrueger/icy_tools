//! Layer view component
//!
//! Shows the layer stack with visibility toggles and layer management controls.
//! Each layer shows a preview rendered via Canvas with checkerboard background
//! for transparent areas.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Task, Theme, mouse,
    widget::{
        button, canvas,
        canvas::{Frame, Geometry, Path},
        column, container, row, scrollable, svg, text,
    },
};

use iced_aw::ContextMenu;
use icy_engine::{Layer, Position, RenderOptions, Screen, TextBuffer, TextPane};
use icy_engine_edit::EditState;
use icy_view_gui::DoubleClickDetector;
use parking_lot::Mutex;

use crate::fl;

// SVG icon data
const ADD_LAYER_SVG: &[u8] = include_bytes!("../../../data/icons/add_layer.svg");
const MOVE_UP_SVG: &[u8] = include_bytes!("../../../data/icons/move_up.svg");
const MOVE_DOWN_SVG: &[u8] = include_bytes!("../../../data/icons/move_down.svg");
const DELETE_SVG: &[u8] = include_bytes!("../../../data/icons/delete.svg");
const VISIBILITY_SVG: &[u8] = include_bytes!("../../../data/icons/visibility.svg");
const VISIBILITY_OFF_SVG: &[u8] = include_bytes!("../../../data/icons/visibility_off.svg");

// Preview dimensions
const MAX_PREVIEW_CHARS_WIDTH: i32 = 80;
const MAX_PREVIEW_CHARS_HEIGHT: i32 = 25;
const PREVIEW_WIDTH: f32 = 128.0;
const PREVIEW_HEIGHT: f32 = PREVIEW_WIDTH / 1.6;
const CHECKER_SIZE: f32 = 4.0;
const LAYER_ROW_PADDING: u16 = 2;

/// Messages for the layer view
#[derive(Clone, Debug)]
pub enum LayerMessage {
    Select(usize),
    ToggleVisibility(usize),
    Add,
    Remove(usize),
    MoveUp(usize),
    MoveDown(usize),
    Rename(usize, String),
    /// Open layer properties dialog (triggered by double-click or context menu)
    EditLayer(usize),
    /// Duplicate a layer (context menu)
    Duplicate(usize),
    /// Merge layer down (context menu)
    MergeDown(usize),
    /// Clear layer contents (context menu)
    Clear(usize),
}

/// Cached preview data for a layer
#[derive(Clone)]
pub struct PreviewData {
    /// RGBA pixel data
    pub pixels: Vec<u8>,
    /// Source width in pixels
    pub src_width: u32,
    /// Source height in pixels
    pub src_height: u32,
}

impl PreviewData {
    fn new(pixels: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            pixels,
            src_width: width,
            src_height: height,
        }
    }
}

/// Layer view state
pub struct LayerView {
    /// Preview cache (layer index -> preview data)
    preview_cache: RefCell<HashMap<usize, PreviewData>>,
    /// Last known undo stack length
    last_undo_len: RefCell<usize>,
    /// Last known layer count
    last_layer_count: RefCell<usize>,
    /// Double-click detector for opening layer properties
    double_click: RefCell<DoubleClickDetector<usize>>,
}

impl Default for LayerView {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerView {
    pub fn new() -> Self {
        Self {
            preview_cache: RefCell::new(HashMap::new()),
            last_undo_len: RefCell::new(usize::MAX),
            last_layer_count: RefCell::new(0),
            double_click: RefCell::new(DoubleClickDetector::new()),
        }
    }

    pub fn update(&mut self, _message: LayerMessage) -> Task<LayerMessage> {
        Task::none()
    }

    /// Check if the given layer index was double-clicked.
    /// Returns true if it was a double-click, false for single click.
    pub fn check_double_click(&self, index: usize) -> bool {
        self.double_click.borrow_mut().is_double_click(index)
    }

    /// Generate preview RGBA data for a layer
    fn generate_preview(layer: &Layer, buffer: &TextBuffer) -> Option<PreviewData> {
        let width = buffer.width().min(MAX_PREVIEW_CHARS_WIDTH);
        let height = buffer.height().min(MAX_PREVIEW_CHARS_HEIGHT);

        if width <= 0 || height <= 0 {
            return None;
        }

        let mut temp_buffer = TextBuffer::create((width, height));
        temp_buffer.palette = buffer.palette.clone();
        temp_buffer.set_font_table(buffer.font_table());

        let mut layer_copy = layer.clone();
        layer_copy.set_offset(Position::default());
        layer_copy.set_is_visible(true);
        temp_buffer.set_size(layer_copy.size());
        temp_buffer.layers.clear();
        temp_buffer.layers.push(layer_copy);

        let options = RenderOptions {
            rect: icy_engine::Rectangle::from(0, 0, MAX_PREVIEW_CHARS_WIDTH, MAX_PREVIEW_CHARS_HEIGHT).into(),
            ..Default::default()
        };
        let region = icy_engine::Rectangle::from(
            0,
            0,
            MAX_PREVIEW_CHARS_WIDTH * temp_buffer.font_dimensions().width,
            MAX_PREVIEW_CHARS_HEIGHT * temp_buffer.font_dimensions().height,
        );
        let (size, rgba) = temp_buffer.render_region_to_rgba(region, &options, false);

        if size.width <= 0 || size.height <= 0 || rgba.is_empty() {
            return None;
        }

        Some(PreviewData::new(rgba, size.width as u32, size.height as u32))
    }

    fn icon_button<'a>(icon_data: &'static [u8], message: LayerMessage) -> Element<'a, LayerMessage> {
        let icon = svg(svg::Handle::from_memory(icon_data)).width(Length::Fixed(20.0)).height(Length::Fixed(20.0));

        button(icon).on_press(message).padding(4).style(button::text).into()
    }

    /// Render a layer row with canvas preview
    fn layer_row<'a>(
        index: usize,
        title: String,
        is_visible: bool,
        is_selected: bool,
        preview: Option<PreviewData>,
        layer_count: usize,
    ) -> Element<'a, LayerMessage> {
        // Preview canvas
        let preview_canvas: Element<'a, LayerMessage> = canvas(PreviewCanvas { data: preview })
            .width(Length::Fixed(PREVIEW_WIDTH))
            .height(Length::Fixed(PREVIEW_HEIGHT))
            .into();

        let preview_container = container(preview_canvas).style(|_theme: &Theme| container::Style {
            border: iced::Border {
                color: Color::from_rgb8(60, 60, 60),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        });

        // Visibility toggle with SVG icon
        let vis_icon_data = if is_visible { VISIBILITY_SVG } else { VISIBILITY_OFF_SVG };
        let vis_icon = svg(svg::Handle::from_memory(vis_icon_data))
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0));
        let vis_btn = button(vis_icon).on_press(LayerMessage::ToggleVisibility(index)).padding(2).style(button::text);

        // Layer title
        let title_text = text(title).size(14);

        // Title row with visibility icon
        let title_row = row![vis_btn, title_text].spacing(4).align_y(iced::Alignment::Center);

        // Right side content: title above visibility
        let right_content = column![title_row].spacing(2);

        // Row content: preview on left, title+visibility on right
        let row_content = row![preview_container, right_content]
            .spacing(8)
            .padding(LAYER_ROW_PADDING)
            .align_y(iced::Alignment::Center);

        // Clickable row
        let layer_button = button(row_content)
            .on_press(LayerMessage::Select(index))
            .width(Length::Fill)
            .style(move |theme: &Theme, status| {
                let palette = theme.extended_palette();
                let base = button::Style::default();

                match status {
                    button::Status::Active | button::Status::Pressed => {
                        if is_selected {
                            button::Style {
                                background: Some(iced::Background::Color(palette.primary.weak.color)),
                                text_color: palette.primary.weak.text,
                                border: iced::Border {
                                    color: palette.primary.strong.color,
                                    width: 1.0,
                                    radius: 2.0.into(),
                                },
                                ..base
                            }
                        } else {
                            button::Style {
                                background: Some(iced::Background::Color(palette.background.base.color)),
                                text_color: palette.background.base.text,
                                border: iced::Border {
                                    color: palette.background.strong.color,
                                    width: 1.0,
                                    radius: 2.0.into(),
                                },
                                ..base
                            }
                        }
                    }
                    button::Status::Hovered => button::Style {
                        background: Some(iced::Background::Color(palette.primary.weak.color.scale_alpha(0.5))),
                        text_color: palette.background.base.text,
                        border: iced::Border {
                            color: palette.primary.base.color,
                            width: 1.0,
                            radius: 2.0.into(),
                        },
                        ..base
                    },
                    button::Status::Disabled => base,
                }
            });

        // Wrap with context menu
        ContextMenu::new(layer_button, move || Self::build_context_menu(index, layer_count)).into()
    }

    /// Build the context menu for a layer
    fn build_context_menu(index: usize, layer_count: usize) -> Element<'static, LayerMessage> {
        let props_btn = Self::menu_item(fl!("layer_tool_menu_layer_properties"), Some(LayerMessage::EditLayer(index)));
        let new_btn = Self::menu_item(fl!("layer_tool_menu_new_layer"), Some(LayerMessage::Add));
        let duplicate_btn = Self::menu_item(fl!("layer_tool_menu_duplicate_layer"), Some(LayerMessage::Duplicate(index)));
        let merge_btn = Self::menu_item(
            fl!("layer_tool_menu_merge_layer"),
            if index > 0 { Some(LayerMessage::MergeDown(index)) } else { None },
        );
        let delete_btn = Self::menu_item(
            fl!("layer_tool_menu_delete_layer"),
            if layer_count > 1 { Some(LayerMessage::Remove(index)) } else { None },
        );
        let clear_btn = Self::menu_item(fl!("layer_tool_menu_clear_layer"), Some(LayerMessage::Clear(index)));

        container(
            column![props_btn, new_btn, duplicate_btn, merge_btn, delete_btn, clear_btn]
                .spacing(2)
                .width(Length::Fixed(200.0)),
        )
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background.weak.color)),
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .padding(4)
        .into()
    }

    /// Create a single menu item button
    fn menu_item(label: String, message: Option<LayerMessage>) -> Element<'static, LayerMessage> {
        let is_enabled = message.is_some();

        button(text(label).size(13))
            .on_press_maybe(message)
            .width(Length::Fill)
            .padding([6, 10])
            .style(move |theme: &Theme, status: button::Status| {
                let palette = theme.extended_palette();

                match status {
                    button::Status::Hovered if is_enabled => button::Style {
                        background: Some(iced::Background::Color(palette.primary.base.color)),
                        text_color: palette.primary.base.text,
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    button::Status::Pressed if is_enabled => button::Style {
                        background: Some(iced::Background::Color(palette.primary.strong.color)),
                        text_color: palette.primary.strong.text,
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    _ if !is_enabled => button::Style {
                        background: None,
                        text_color: palette.background.weak.text.scale_alpha(0.4),
                        ..Default::default()
                    },
                    _ => button::Style {
                        background: None,
                        text_color: palette.background.weak.text,
                        ..Default::default()
                    },
                }
            })
            .into()
    }

    /// Render the layer view
    pub fn view<'a>(&'a self, screen: &'a Arc<Mutex<Box<dyn Screen>>>) -> Element<'a, LayerMessage> {
        // Read layer data and update cache
        let (layer_data, current_layer, layer_count) = {
            let mut screen_guard = screen.lock();
            let state = screen_guard.as_any_mut().downcast_mut::<EditState>().expect("Screen should be EditState");

            let buffer = state.get_buffer();
            let current = state.get_current_layer().unwrap_or(0);
            let undo_len = state.undo_stack_len();
            let layer_count = buffer.layers.len();

            // Check if we need to regenerate previews
            let needs_regen = undo_len != *self.last_undo_len.borrow() || layer_count != *self.last_layer_count.borrow();

            if needs_regen {
                *self.last_undo_len.borrow_mut() = undo_len;
                *self.last_layer_count.borrow_mut() = layer_count;
                self.preview_cache.borrow_mut().clear();

                // Generate all previews
                for (idx, layer) in buffer.layers.iter().enumerate() {
                    if let Some(preview) = Self::generate_preview(layer, buffer) {
                        self.preview_cache.borrow_mut().insert(idx, preview);
                    }
                }
            }

            // Collect layer info with cloned preview data (reversed for display)
            let cache = self.preview_cache.borrow();
            let data: Vec<(usize, String, bool, Option<PreviewData>)> = buffer
                .layers
                .iter()
                .enumerate()
                .rev()
                .map(|(idx, layer)| {
                    let title = if layer.title().is_empty() {
                        format!("Layer {}", idx + 1)
                    } else {
                        layer.title().to_string()
                    };
                    let preview = cache.get(&idx).cloned();
                    (idx, title, layer.is_visible(), preview)
                })
                .collect();

            (data, current, layer_count)
        };

        // Create layer rows
        let mut layer_list = column![].spacing(2).padding(4);
        for (idx, title, is_visible, preview) in layer_data {
            layer_list = layer_list.push(Self::layer_row(idx, title, is_visible, idx == current_layer, preview, layer_count));
        }

        // Scrollable layer list
        let scrollable_layers = scrollable(layer_list).width(Length::Fill).height(Length::Fill);

        // Button bar
        let add_btn = Self::icon_button(ADD_LAYER_SVG, LayerMessage::Add);
        let move_up_btn = Self::icon_button(MOVE_UP_SVG, LayerMessage::MoveUp(current_layer));
        let move_down_btn = Self::icon_button(MOVE_DOWN_SVG, LayerMessage::MoveDown(current_layer));
        let delete_btn = Self::icon_button(DELETE_SVG, LayerMessage::Remove(current_layer));

        let button_bar = container(row![add_btn, move_up_btn, move_down_btn, delete_btn].spacing(0))
            .padding([2, 0])
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.weak.color)),
                    ..Default::default()
                }
            });

        column![
            container(scrollable_layers).width(Length::Fill).height(Length::Fill).style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.base.color)),
                    ..Default::default()
                }
            }),
            button_bar,
        ]
        .into()
    }
}

/// Canvas widget for rendering a layer preview with checkerboard background
struct PreviewCanvas {
    data: Option<PreviewData>,
}

impl canvas::Program<LayerMessage> for PreviewCanvas {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Draw checkerboard background
        draw_checkerboard(&mut frame, bounds.size());

        // Draw preview if available
        if let Some(ref data) = self.data {
            // Scale factors to fit preview in bounds while maintaining aspect ratio
            let scale_x = bounds.width / data.src_width as f32;
            let scale_y = bounds.height / data.src_height as f32;
            let scale = scale_x.min(scale_y);

            // Centered offset
            let scaled_w = data.src_width as f32 * scale;
            let scaled_h = data.src_height as f32 * scale;
            let offset_x = ((bounds.width - scaled_w) / 2.0).floor();
            let offset_y = ((bounds.height - scaled_h) / 2.0).floor();

            // Draw pixels in blocks for efficiency
            // Determine sampling step based on scale
            let block_size = 2.0_f32.max(scale);
            let sample_step_x = ((data.src_width as f32 / (scaled_w / block_size)).floor() as u32).max(1);
            let sample_step_y = ((data.src_height as f32 / (scaled_h / block_size)).ceil() as u32).max(1);

            for sy in (0..data.src_height).step_by(sample_step_y as usize) {
                for sx in (0..data.src_width).step_by(sample_step_x as usize) {
                    let idx = ((sy * data.src_width + sx) * 4) as usize;
                    if idx + 3 >= data.pixels.len() {
                        continue;
                    }

                    let r = data.pixels[idx];
                    let g = data.pixels[idx + 1];
                    let b = data.pixels[idx + 2];
                    let a = data.pixels[idx + 3];

                    // Skip fully transparent pixels (show checkerboard)
                    if a == 0 {
                        continue;
                    }

                    let x = (offset_x + sx as f32 * scale).floor();
                    let y = (offset_y + sy as f32 * scale).floor();
                    let w = (sample_step_x as f32 * scale).ceil();
                    let h = (sample_step_y as f32 * scale).ceil();

                    let color = Color::from_rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0);
                    let rect = Path::rectangle(Point::new(x, y), Size::new(w, h));
                    frame.fill(&rect, color);
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

/// Draw a checkerboard pattern for transparent areas
fn draw_checkerboard(frame: &mut Frame, size: Size) {
    let dark = Color::from_rgb8(40, 40, 40);
    let light = Color::from_rgb8(60, 60, 60);

    let cols = (size.width / CHECKER_SIZE).ceil() as i32;
    let rows = (size.height / CHECKER_SIZE).ceil() as i32;

    for row in 0..rows {
        for col in 0..cols {
            let color = if (row + col) % 2 == 0 { dark } else { light };
            let x = (col as f32 * CHECKER_SIZE).floor();
            let y = (row as f32 * CHECKER_SIZE).floor();
            let w: f32 = CHECKER_SIZE.min(size.width - x).ceil();
            let h = CHECKER_SIZE.min(size.height - y).ceil();

            let rect = Path::rectangle(Point::new(x, y), Size::new(w, h));
            frame.fill(&rect, color);
        }
    }
}
