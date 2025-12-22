//! Layer view component
//!
//! Shows the layer stack with visibility toggles and layer management controls.
//! Each layer shows a preview rendered via Canvas with checkerboard background
//! for transparent areas.
//!
//! NOTE: Some fields and methods are prepared for future preview caching.

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::usize;

use iced::{
    Border, Color, Element, Event, Length, Point, Rectangle, Size, Task, Theme,
    advanced::{
        image::{self as adv_image, Renderer as _},
        layout::{self, Layout},
        renderer::{self, Renderer as _},
        text::Renderer as _,
        widget::{self, Widget},
    },
    mouse,
    widget::{button, column, container, image, row, svg, text},
};

use iced_aw::ContextMenu;
use icy_engine::{BitFont, Layer, Position, RenderOptions, Screen, TextBuffer, TextPane};
use icy_engine_edit::EditState;
use icy_engine_gui::CheckerboardColors;
use icy_engine_gui::DoubleClickDetector;
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::{Viewport, wrap_with_scrollbars};
use parking_lot::Mutex;

use crate::fl;

// SVG icon data
const ADD_LAYER_SVG: &[u8] = include_bytes!("../../../../../../data/icons/add_layer.svg");
const MOVE_UP_SVG: &[u8] = include_bytes!("../../../../../../data/icons/move_up.svg");
const MOVE_DOWN_SVG: &[u8] = include_bytes!("../../../../../../data/icons/move_down.svg");
const DELETE_SVG: &[u8] = include_bytes!("../../../../../../data/icons/delete.svg");
const VISIBILITY_SVG: &[u8] = include_bytes!("../../../../../../data/icons/visibility.svg");
const VISIBILITY_OFF_SVG: &[u8] = include_bytes!("../../../../../../data/icons/visibility_off.svg");

// Preview dimensions
const MAX_PREVIEW_CHARS_WIDTH: i32 = 80;
const MAX_PREVIEW_CHARS_HEIGHT: i32 = 25;
const PREVIEW_WIDTH: f32 = 128.0;
const PREVIEW_HEIGHT: f32 = PREVIEW_WIDTH / 1.6;
const LAYER_ROW_PADDING: u16 = 2;
const LAYER_ROW_HEIGHT: f32 = PREVIEW_HEIGHT + 2.0 * (LAYER_ROW_PADDING as f32) + 2.0;

const PREVIEW_TEX_W: u32 = PREVIEW_WIDTH as u32;
const PREVIEW_TEX_H: u32 = PREVIEW_HEIGHT as u32;

const PREVIEW_ATLAS_COLS: u32 = 8;
const PREVIEW_ATLAS_ROWS: u32 = 32;
const PREVIEW_ATLAS_SLOTS: u32 = PREVIEW_ATLAS_COLS * PREVIEW_ATLAS_ROWS;
const PREVIEW_ATLAS_W: u32 = PREVIEW_TEX_W * PREVIEW_ATLAS_COLS;
const PREVIEW_ATLAS_H: u32 = PREVIEW_TEX_H * PREVIEW_ATLAS_ROWS;

const MAX_LABEL_CHARS: usize = 64;

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
    // === Paste mode messages ===
    /// Keep paste as separate layer (exit paste mode without merging)
    PasteKeepAsLayer,
    /// Cancel paste mode (discard floating layer)
    PasteCancel,
}

#[derive(Clone)]
struct PreviewTexture {
    rgba_128x80: Arc<Vec<u8>>,
}

impl PreviewTexture {
    fn new(rgba_128x80: Vec<u8>) -> Self {
        Self {
            rgba_128x80: Arc::new(rgba_128x80),
        }
    }
}

#[derive(Clone, Default)]
struct PreviewAtlasState {
    version: u64,
    first_list_idx: u32,
    row_count: u32,
    width: u32,
    height: u32,
    pixels: Arc<Vec<u8>>,
}

impl std::fmt::Debug for PreviewAtlasState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewAtlasState")
            .field("version", &self.version)
            .field("first_list_idx", &self.first_list_idx)
            .field("row_count", &self.row_count)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixels_len", &self.pixels.len())
            .finish()
    }
}

#[derive(Clone)]
struct LayerRowInfo {
    layer_index: usize,
    title: String,
    is_visible: bool,
}

#[derive(Clone)]
struct LabelTexture {
    handle: image::Handle,
    width: u32,
    height: u32,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct LabelKey {
    font_key: u64,
    text: String,
    fg_rgba8: u32,
}

/// Layer view state
pub struct LayerView {
    /// Preview cache (layer index -> texture handle)
    preview_cache: RefCell<HashMap<usize, PreviewTexture>>,
    /// Last known buffer version
    last_buffer_version: RefCell<u64>,
    /// Last known layer count
    last_layer_count: RefCell<usize>,
    /// Double-click detector for opening layer properties
    double_click: RefCell<DoubleClickDetector<usize>>,

    /// Cached rendered label textures (font+text -> handle)
    label_cache: RefCell<HashMap<LabelKey, LabelTexture>>,

    /// Viewport for owner-rendered list (overlay scrollbar)
    viewport: RefCell<Viewport>,

    hovered_list_idx: Arc<AtomicU32>,

    visibility_icon_cache: RefCell<HashMap<bool, image::Handle>>,

    preview_atlas: Arc<Mutex<PreviewAtlasState>>,

    // Track viewport changes to avoid rebuilding previews/atlas on every redraw.
    last_scroll_y: RefCell<f32>,
    last_visible_height: RefCell<f32>,
}

impl Default for LayerView {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerView {
    pub fn new() -> Self {
        let mut viewport = Viewport::default();
        viewport.zoom = 1.0;
        viewport.content_width = 400.0;
        viewport.content_height = 0.0;
        viewport.visible_width = 400.0;
        viewport.visible_height = 300.0;

        Self {
            preview_cache: RefCell::new(HashMap::new()),
            last_buffer_version: RefCell::new(u64::MAX),
            last_layer_count: RefCell::new(0),
            double_click: RefCell::new(DoubleClickDetector::new()),
            viewport: RefCell::new(viewport),
            label_cache: RefCell::new(HashMap::new()),
            hovered_list_idx: Arc::new(AtomicU32::new(u32::MAX)),
            visibility_icon_cache: RefCell::new(HashMap::new()),
            preview_atlas: Arc::new(Mutex::new(PreviewAtlasState {
                width: PREVIEW_ATLAS_W,
                height: PREVIEW_ATLAS_H,
                pixels: Arc::new(vec![0u8; (PREVIEW_ATLAS_W * PREVIEW_ATLAS_H * 4) as usize]),
                ..Default::default()
            })),
            last_scroll_y: RefCell::new(f32::NAN),
            last_visible_height: RefCell::new(f32::NAN),
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

    /// Generate a preview texture for a layer
    fn generate_preview(layer: &Layer, buffer: &TextBuffer) -> Option<PreviewTexture> {
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

        fn resize_rgba_letterbox_bilinear(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
            let mut out = vec![0u8; (dw * dh * 4) as usize];
            if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
                return out;
            }

            let scale = (dw as f32 / sw as f32).min(dh as f32 / sh as f32);
            let nw = ((sw as f32) * scale).round().clamp(1.0, dw as f32) as u32;
            let nh = ((sh as f32) * scale).round().clamp(1.0, dh as f32) as u32;
            let x_off = ((dw - nw) / 2) as usize;
            let y_off = ((dh - nh) / 2) as usize;

            let sample = |x: f32, y: f32, c: usize| -> f32 {
                let x = x.clamp(0.0, (sw - 1) as f32);
                let y = y.clamp(0.0, (sh - 1) as f32);

                let x0 = x.floor() as u32;
                let y0 = y.floor() as u32;
                let x1 = (x0 + 1).min(sw - 1);
                let y1 = (y0 + 1).min(sh - 1);

                let tx = x - x0 as f32;
                let ty = y - y0 as f32;

                let idx = |px: u32, py: u32| -> usize { ((py * sw + px) * 4) as usize + c };
                let v00 = src[idx(x0, y0)] as f32;
                let v10 = src[idx(x1, y0)] as f32;
                let v01 = src[idx(x0, y1)] as f32;
                let v11 = src[idx(x1, y1)] as f32;

                let a = v00 * (1.0 - tx) + v10 * tx;
                let b = v01 * (1.0 - tx) + v11 * tx;
                a * (1.0 - ty) + b * ty
            };

            for dy in 0..nh {
                let sy = (dy as f32 + 0.5) * (sh as f32 / nh as f32) - 0.5;
                for dx in 0..nw {
                    let sx = (dx as f32 + 0.5) * (sw as f32 / nw as f32) - 0.5;
                    let dst_x = x_off + dx as usize;
                    let dst_y = y_off + dy as usize;
                    let out_idx = (dst_y * dw as usize + dst_x) * 4;
                    for c in 0..4 {
                        out[out_idx + c] = sample(sx, sy, c).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }

            out
        }

        let sw = size.width as u32;
        let sh = size.height as u32;
        let rgba_128x80 = resize_rgba_letterbox_bilinear(&rgba, sw, sh, PREVIEW_TEX_W, PREVIEW_TEX_H);
        Some(PreviewTexture::new(rgba_128x80))
    }

    fn icon_button<'a>(icon_data: &'static [u8], icon_color: Color, message: LayerMessage) -> Element<'a, LayerMessage> {
        let icon = svg(svg::Handle::from_memory(icon_data))
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .style(move |_theme: &Theme, _status| svg::Style {
                color: Some(icon_color),
                ..Default::default()
            });

        button(icon).on_press(message).padding(4).style(button::text).into()
    }

    fn icon_button_opt<'a>(icon_data: &'static [u8], icon_color: Color, message: Option<LayerMessage>) -> Element<'a, LayerMessage> {
        let icon = svg(svg::Handle::from_memory(icon_data))
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .style(move |_theme: &Theme, _status| svg::Style {
                color: Some(icon_color),
                ..Default::default()
            });
        let mut b = button(icon).padding(4).style(button::text);
        if let Some(msg) = message {
            b = b.on_press(msg);
        }
        b.into()
    }

    fn update_viewport_content_size(&self, row_count: usize) {
        let total_height = row_count as f32 * LAYER_ROW_HEIGHT;
        let mut viewport = self.viewport.borrow_mut();
        // Avoid marking the viewport as changed every frame.
        if (viewport.content_height - total_height).abs() > 0.5 {
            viewport.content_height = total_height;
            viewport.sync_scrollbar_position();
            viewport.changed.store(true, Ordering::Relaxed);
        }
    }

    fn clear_atlas(&self) {
        let mut atlas = self.preview_atlas.lock();
        atlas.first_list_idx = 0;
        atlas.row_count = 0;
        atlas.width = PREVIEW_ATLAS_W;
        atlas.height = PREVIEW_ATLAS_H;
        atlas.pixels = Arc::new(vec![0u8; (PREVIEW_ATLAS_W * PREVIEW_ATLAS_H * 4) as usize]);
        atlas.version = atlas.version.wrapping_add(1);
    }

    fn ensure_previews_and_atlas(&self, screen: &Arc<Mutex<Box<dyn Screen>>>, rows: &[LayerRowInfo]) {
        let (scroll_y, visible_height) = {
            let vp = self.viewport.borrow();
            (vp.scroll_y, vp.visible_height)
        };

        // Track changes that affect the visible range.
        let mut viewport_changed = false;
        {
            let mut last = self.last_scroll_y.borrow_mut();
            if !last.is_finite() || (*last - scroll_y).abs() > 0.5 {
                *last = scroll_y;
                viewport_changed = true;
            }
        }
        {
            let mut last = self.last_visible_height.borrow_mut();
            if !last.is_finite() || (*last - visible_height).abs() > 0.5 {
                *last = visible_height;
                viewport_changed = true;
            }
        }

        if rows.is_empty() {
            return;
        }

        let first = (scroll_y / LAYER_ROW_HEIGHT).floor().max(0.0) as usize;
        let visible_count = (visible_height / LAYER_ROW_HEIGHT).ceil() as usize + 2;
        let last = (first + visible_count).min(rows.len());
        let visible_count = last.saturating_sub(first).min(PREVIEW_ATLAS_SLOTS as usize);
        if visible_count == 0 {
            return;
        }

        // Determine missing previews for the visible range.
        let mut missing: Vec<usize> = Vec::new();
        {
            let cache = self.preview_cache.borrow();
            for list_idx in first..(first + visible_count) {
                let layer_idx = rows[list_idx].layer_index;
                if !cache.contains_key(&layer_idx) {
                    missing.push(layer_idx);
                }
            }
        }

        if !missing.is_empty() {
            // Render under a single lock
            let mut rendered: Vec<(usize, PreviewTexture)> = Vec::new();
            {
                let mut screen_guard = screen.lock();
                let state = screen_guard.as_any_mut().downcast_mut::<EditState>().expect("Screen should be EditState");
                let buffer = state.get_buffer();

                for layer_idx in missing {
                    let Some(layer) = buffer.layers.get(layer_idx) else {
                        continue;
                    };
                    if let Some(preview) = LayerView::generate_preview(layer, buffer) {
                        rendered.push((layer_idx, preview));
                    }
                }
            }

            if !rendered.is_empty() {
                let mut cache = self.preview_cache.borrow_mut();
                for (idx, preview) in rendered {
                    cache.insert(idx, preview);
                }
                // New previews affect atlas contents.
                viewport_changed = true;
            }
        }

        // Only rebuild atlas when viewport range changed OR atlas range doesn't match OR any preview was generated.
        let (needs_atlas_params, needs_atlas_content) = {
            let atlas = self.preview_atlas.lock();
            let params_mismatch = atlas.first_list_idx != first as u32 || atlas.row_count != visible_count as u32;
            // If any preview for the visible range is still missing, we need an atlas rebuild later.
            let mut content_missing = false;
            let cache = self.preview_cache.borrow();
            for list_idx in first..(first + visible_count) {
                let layer_idx = rows[list_idx].layer_index;
                if !cache.contains_key(&layer_idx) {
                    content_missing = true;
                    break;
                }
            }
            (params_mismatch, content_missing)
        };

        if !(viewport_changed || needs_atlas_params) || needs_atlas_content {
            return;
        }

        // Build atlas pixels for current visible range.
        let mut pixels = vec![0u8; (PREVIEW_ATLAS_W * PREVIEW_ATLAS_H * 4) as usize];
        let cache = self.preview_cache.borrow();
        for slot in 0..visible_count {
            let list_idx = first + slot;
            let layer_idx = rows[list_idx].layer_index;
            let Some(preview) = cache.get(&layer_idx) else {
                continue;
            };

            let slot = slot as u32;
            let col = slot % PREVIEW_ATLAS_COLS;
            let row = slot / PREVIEW_ATLAS_COLS;
            if row >= PREVIEW_ATLAS_ROWS {
                break;
            }

            let dst_x = col * PREVIEW_TEX_W;
            let dst_y = row * PREVIEW_TEX_H;

            let src = preview.rgba_128x80.as_slice();
            for y in 0..PREVIEW_TEX_H {
                let src_row = (y * PREVIEW_TEX_W * 4) as usize;
                let dst_row = (((dst_y + y) * PREVIEW_ATLAS_W + dst_x) * 4) as usize;
                pixels[dst_row..dst_row + (PREVIEW_TEX_W * 4) as usize].copy_from_slice(&src[src_row..src_row + (PREVIEW_TEX_W * 4) as usize]);
            }
        }

        {
            let mut atlas = self.preview_atlas.lock();
            atlas.first_list_idx = first as u32;
            atlas.row_count = visible_count as u32;
            atlas.width = PREVIEW_ATLAS_W;
            atlas.height = PREVIEW_ATLAS_H;
            atlas.pixels = Arc::new(pixels);
            atlas.version = atlas.version.wrapping_add(1);
        }
    }

    fn render_label(font: &BitFont, text: &str, fg: Color) -> Option<LabelTexture> {
        let size = font.size();
        let gw = size.width.max(1) as u32;
        let gh = size.height.max(1) as u32;

        let text: String = text.chars().take(MAX_LABEL_CHARS).collect();
        if text.is_empty() {
            return None;
        }

        let w = (text.chars().count() as u32).saturating_mul(gw).max(1);
        let h = gh.max(1);

        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let fg_rgba = (
            (fg.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (fg.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (fg.b * 255.0).round().clamp(0.0, 255.0) as u8,
            (fg.a * 255.0).round().clamp(0.0, 255.0) as u8,
        );

        for (i, ch) in text.chars().enumerate() {
            // Try both direct and CP437 slot char.
            let slot = super::glyph_renderer::cp437_index(ch);
            let slot_ch = char::from_u32(slot).unwrap_or('?');

            let glyph = font.glyph(slot_ch);

            for y in 0..h as usize {
                for x in 0..gw as usize {
                    let on = glyph.get_pixel(x, y);
                    if !on {
                        continue;
                    }

                    let dst_x = i as u32 * gw + x as u32;
                    let dst_y = y as u32;
                    if dst_x >= w || dst_y >= h {
                        continue;
                    }

                    let idx = ((dst_y * w + dst_x) * 4) as usize;
                    rgba[idx] = fg_rgba.0;
                    rgba[idx + 1] = fg_rgba.1;
                    rgba[idx + 2] = fg_rgba.2;
                    rgba[idx + 3] = fg_rgba.3;
                }
            }
        }

        let handle = image::Handle::from_rgba(w, h, rgba);
        Some(LabelTexture { handle, width: w, height: h })
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
    /// If `paste_mode` is true, the layer view shows paste-specific behavior:
    /// - Layer selection is disabled (paste layer is always selected)
    /// - Add button anchors the paste layer
    /// - Up/Down move the paste layer position
    /// - Delete cancels the paste operation
    pub fn view<'a>(&'a self, theme: &Theme, screen: &'a Arc<Mutex<Box<dyn Screen>>>, font_page: Option<usize>, paste_mode: bool) -> Element<'a, LayerMessage> {
        // Read layer data (no per-frame preview cloning)
        let (rows, current_layer, layer_count, buffer_version, _font_key) = {
            let mut screen_guard = screen.lock();
            let state: &mut EditState = screen_guard.as_any_mut().downcast_mut::<EditState>().expect("Screen should be EditState");
            let buffer = state.get_buffer();
            let layer_count = buffer.layers.len();

            let current = if layer_count > 0 { state.get_current_layer().unwrap_or(0) } else { 0 };
            let buffer_version = buffer.version();

            let rows: Vec<LayerRowInfo> = buffer
                .layers
                .iter()
                .enumerate()
                .rev()
                .map(|(idx, layer)| LayerRowInfo {
                    layer_index: idx,
                    title: if layer.title().is_empty() {
                        format!("Layer {}", idx + 1)
                    } else {
                        layer.title().to_string()
                    },
                    is_visible: layer.is_visible(),
                })
                .collect();

            let font_key = font_page
                .and_then(|fp| buffer.font(fp as u8).map(super::glyph_renderer::font_key))
                .or_else(|| buffer.font(0).map(super::glyph_renderer::font_key));

            (rows, current, layer_count, buffer_version, font_key)
        };

        // UI text in the layer list should use high-resolution TrueType rendering.
        // The widget already has a TTF fallback path when `font_key` is None.
        let font_key: Option<u64> = None;

        // Invalidate preview cache when buffer changes or layers change
        let needs_invalidate = buffer_version != *self.last_buffer_version.borrow() || layer_count != *self.last_layer_count.borrow();
        if needs_invalidate {
            *self.last_buffer_version.borrow_mut() = buffer_version;
            *self.last_layer_count.borrow_mut() = layer_count;
            self.preview_cache.borrow_mut().clear();
            self.clear_atlas();
        }

        // Keep viewport content size in sync
        self.update_viewport_content_size(rows.len());

        // Update previews/atlas only when something actually changed (buffer, scroll, viewport size).
        self.ensure_previews_and_atlas(screen, &rows);

        let selected_list_idx: u32 = rows
            .iter()
            .position(|r| r.layer_index == current_layer)
            .map(|idx| idx as u32)
            .unwrap_or(u32::MAX);

        // Owner-rendered list widget (virtualized) + overlay scrollbar
        let list_widget: Element<'a, LayerMessage> = LayerListWidget {
            screen: Arc::clone(screen),
            rows,
            current_layer,
            preview_cache: &self.preview_cache,
            label_cache: &self.label_cache,
            font_page,
            font_key,
            viewport: &self.viewport,
            double_click: &self.double_click,
            hovered_list_idx: self.hovered_list_idx.clone(),
            visibility_icon_cache: &self.visibility_icon_cache,
            preview_atlas: self.preview_atlas.clone(),
            paste_mode,
        }
        .into();

        let needs_scrollbar = self.viewport.borrow().is_scrollable_y();
        let list_with_scrollbar = wrap_with_scrollbars(list_widget, &self.viewport, needs_scrollbar, false);

        let scroll_y = self.viewport.borrow().scroll_y;
        let palette = theme.extended_palette();
        let icon_color = palette.background.base.text;
        let shader_bg: Element<'a, LayerMessage> = iced::widget::shader(LayerListBackgroundProgram {
            row_count: layer_count as u32,
            row_height: LAYER_ROW_HEIGHT,
            scroll_y,
            selected_row: selected_list_idx,
            hovered_row: self.hovered_list_idx.clone(),
            bg_color: main_area_background(theme),
            preview_bg_color: palette.background.weak.color,
            preview_border_color: palette.background.strong.color.scale_alpha(0.7),
            preview_border_width: 1.0,
            preview_radius: 2.0,
            preview_atlas: self.preview_atlas.clone(),
            checkerboard_colors: CheckerboardColors::default(),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        let list_stack: Element<'a, LayerMessage> = iced::widget::stack![shader_bg, list_with_scrollbar]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        let list_container = container(list_stack).width(Length::Fill).height(Length::Fill).style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: None,
                border: Border::default().width(1).color(palette.background.strong.color),
                ..Default::default()
            }
        });

        // Button bar - changes behavior in paste mode
        let button_bar = if paste_mode {
            // Paste mode: Add=Keep as layer, Up/Down=Move layer order, Delete=Cancel
            let keep_layer_btn = Self::icon_button(ADD_LAYER_SVG, icon_color, LayerMessage::PasteKeepAsLayer);
            let move_up_btn = Self::icon_button(MOVE_UP_SVG, icon_color, LayerMessage::MoveUp(current_layer));
            let move_down_btn = Self::icon_button(MOVE_DOWN_SVG, icon_color, LayerMessage::MoveDown(current_layer));
            let cancel_btn = Self::icon_button(DELETE_SVG, icon_color, LayerMessage::PasteCancel);

            container(row![keep_layer_btn, move_up_btn, move_down_btn, cancel_btn].spacing(0))
                .padding([2, 0])
                .width(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        // Slightly different background to indicate paste mode
                        background: Some(iced::Background::Color(palette.primary.weak.color.scale_alpha(0.3))),
                        ..Default::default()
                    }
                })
        } else {
            // Normal mode
            let add_btn = Self::icon_button(ADD_LAYER_SVG, icon_color, LayerMessage::Add);
            let has_layers = layer_count > 0;
            let move_up_btn = Self::icon_button_opt(MOVE_UP_SVG, icon_color, has_layers.then(|| LayerMessage::MoveUp(current_layer)));
            let move_down_btn = Self::icon_button_opt(MOVE_DOWN_SVG, icon_color, has_layers.then(|| LayerMessage::MoveDown(current_layer)));
            let delete_btn = Self::icon_button_opt(DELETE_SVG, icon_color, has_layers.then(|| LayerMessage::Remove(current_layer)));

            container(row![add_btn, move_up_btn, move_down_btn, delete_btn].spacing(0))
                .padding([2, 0])
                .width(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(iced::Background::Color(palette.background.weak.color)),
                        ..Default::default()
                    }
                })
        };

        let content = column![list_container, button_bar];

        // Wrap with context menu (for current selection)
        if layer_count > 0 {
            ContextMenu::new(content, move || Self::build_context_menu(current_layer, layer_count)).into()
        } else {
            content.into()
        }
    }
}

struct LayerListWidget<'a> {
    screen: Arc<Mutex<Box<dyn Screen>>>,
    rows: Vec<LayerRowInfo>,
    current_layer: usize,
    preview_cache: &'a RefCell<HashMap<usize, PreviewTexture>>,
    label_cache: &'a RefCell<HashMap<LabelKey, LabelTexture>>,
    font_page: Option<usize>,
    font_key: Option<u64>,
    viewport: &'a RefCell<Viewport>,
    double_click: &'a RefCell<DoubleClickDetector<usize>>,

    hovered_list_idx: Arc<AtomicU32>,

    visibility_icon_cache: &'a RefCell<HashMap<bool, image::Handle>>,

    preview_atlas: Arc<Mutex<PreviewAtlasState>>,

    /// When true, layer selection is disabled (paste layer is always selected)
    paste_mode: bool,
}

#[derive(Clone, Copy, Debug)]
struct LayerListWidgetState {
    left_button_down: bool,
    pressed_list_idx: Option<usize>,
    pressed_pos: Option<Point>,
    pressed_was_selected: bool,
    /// Tracks the last completed click (release) for double-click detection.
    /// (layer_index, timestamp)
    last_click: Option<(usize, std::time::Instant)>,
}

impl Default for LayerListWidgetState {
    fn default() -> Self {
        Self {
            left_button_down: false,
            pressed_list_idx: None,
            pressed_pos: None,
            pressed_was_selected: false,
            last_click: None,
        }
    }
}

impl<'a> LayerListWidget<'a> {
    fn visibility_icon_handle(&self, is_visible: bool) -> Option<image::Handle> {
        if let Some(handle) = self.visibility_icon_cache.borrow().get(&is_visible) {
            return Some(handle.clone());
        }

        let svg_bytes = if is_visible { VISIBILITY_SVG } else { VISIBILITY_OFF_SVG };
        let size = 16u32;
        let rgba = crate::ui::tool_panel::render_svg_to_rgba(svg_bytes, size, size)?;
        let handle = image::Handle::from_rgba(size, size, rgba);

        self.visibility_icon_cache.borrow_mut().insert(is_visible, handle.clone());
        Some(handle)
    }

    fn visible_range(&self, bounds: Rectangle) -> (usize, usize) {
        let scroll_offset = self.viewport.borrow().scroll_y;
        let first_visible = (scroll_offset / LAYER_ROW_HEIGHT).floor().max(0.0) as usize;
        let visible_count = (bounds.height / LAYER_ROW_HEIGHT).ceil() as usize + 2;
        let last_visible = (first_visible + visible_count).min(self.rows.len());
        (first_visible, last_visible)
    }

    fn row_bounds(&self, list_bounds: Rectangle, list_idx: usize) -> Rectangle {
        let scroll_offset = self.viewport.borrow().scroll_y;
        let y = list_bounds.y + list_idx as f32 * LAYER_ROW_HEIGHT - scroll_offset;
        Rectangle {
            x: list_bounds.x,
            y,
            width: list_bounds.width,
            height: LAYER_ROW_HEIGHT,
        }
    }

    fn preview_bounds(&self, row_bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: row_bounds.x + LAYER_ROW_PADDING as f32,
            y: row_bounds.y + LAYER_ROW_PADDING as f32,
            width: PREVIEW_WIDTH,
            height: PREVIEW_HEIGHT,
        }
    }

    fn visibility_toggle_bounds(&self, row_bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: row_bounds.x + PREVIEW_WIDTH + (LAYER_ROW_PADDING as f32) * 2.0 + 8.0,
            y: row_bounds.y + LAYER_ROW_PADDING as f32 + 2.0,
            width: 16.0,
            height: 16.0,
        }
    }

    fn draw_row(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        list_bounds: Rectangle,
        row_bounds: Rectangle,
        row: &LayerRowInfo,
        is_hovered: bool,
        is_selected: bool,
    ) {
        let palette = theme.extended_palette();

        let title_fg = if is_selected {
            // Selected row background comes from primary; use its text color.
            palette.primary.base.text
        } else if is_hovered {
            palette.background.base.text
        } else {
            palette.background.base.text
        };

        let _preview_bounds = self.preview_bounds(row_bounds);
        // Previews (including border/background) are rendered by the background WGSL shader via the atlas.

        // Visibility toggle (SVG icon)
        let toggle_bounds = self.visibility_toggle_bounds(row_bounds);

        renderer.fill_quad(
            renderer::Quad {
                bounds: toggle_bounds,
                border: Border::default().width(1.0).color(palette.background.strong.color).rounded(2.0),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            palette.background.weak.color,
        );

        // Draw cached icon (white SVG) centered.
        if let Some(icon_handle) = self.visibility_icon_handle(row.is_visible) {
            let pad = 2.0;
            let img_bounds = Rectangle {
                x: toggle_bounds.x + pad,
                y: toggle_bounds.y + pad,
                width: (toggle_bounds.width - pad * 2.0).max(1.0),
                height: (toggle_bounds.height - pad * 2.0).max(1.0),
            };

            let image = adv_image::Image::<iced::widget::image::Handle> {
                handle: icon_handle,
                filter_method: adv_image::FilterMethod::Linear,
                rotation: iced::Radians(0.0),
                opacity: 1.0,
                snap: true,
                border_radius: iced::border::Radius::default(),
            };

            if let Some(clip) = super_intersect(toggle_bounds, list_bounds) {
                renderer.draw_image(image, img_bounds, clip);
            }
        }

        // Title (BitFont label texture)
        let title_x = toggle_bounds.x + toggle_bounds.width + 6.0;
        let title_bounds = Rectangle {
            x: title_x,
            y: row_bounds.y,
            width: (row_bounds.x + row_bounds.width - title_x).max(0.0),
            height: row_bounds.height,
        };

        let title_font_px: f32 = 14.0;
        let title_text_y = title_bounds.y + (title_bounds.height - title_font_px).max(0.0) * 0.5;
        let title_text_bounds = Size {
            width: title_bounds.width,
            height: title_font_px,
        };

        let Some(font_key) = self.font_key else {
            let title_text = iced::advanced::text::Text {
                content: row.title.clone(),
                bounds: title_text_bounds,
                size: iced::Pixels(title_font_px),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: iced::Font::default(),
                align_x: iced::advanced::text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::advanced::text::Wrapping::None,
                hint_factor: Some(0.0),
            };
            renderer.fill_text(title_text, Point::new(title_bounds.x, title_text_y), title_fg, list_bounds);
            return;
        };

        let [r, g, b, a] = title_fg.into_rgba8();
        let fg_rgba8: u32 = (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24);
        let key = LabelKey {
            font_key,
            text: row.title.chars().take(MAX_LABEL_CHARS).collect(),
            fg_rgba8,
        };

        let label = {
            let cache = self.label_cache.borrow();
            cache.get(&key).cloned()
        }
        .or_else(|| {
            let fg = title_fg;

            let Some(label) = ({
                let mut screen_guard = self.screen.lock();
                let state = screen_guard.as_any_mut().downcast_mut::<EditState>().expect("Screen should be EditState");
                let buffer = state.get_buffer();
                let font = self.font_page.and_then(|fp| buffer.font(fp as u8)).or_else(|| buffer.font(0));
                font.and_then(|font| LayerView::render_label(font, &key.text, fg))
            }) else {
                return None;
            };
            let mut cache = self.label_cache.borrow_mut();
            cache.entry(key).or_insert_with(|| label.clone());
            Some(label)
        });

        if let Some(label) = label {
            let scale = (title_bounds.height / label.height as f32).min(1.0).max(0.001);
            let w = (label.width as f32 * scale).min(title_bounds.width);
            let h = label.height as f32 * scale;
            let img_bounds = Rectangle {
                x: title_bounds.x,
                y: title_bounds.y + (title_bounds.height - h) * 0.5,
                width: w,
                height: h,
            };

            let image = adv_image::Image::<iced::widget::image::Handle> {
                handle: label.handle,
                filter_method: adv_image::FilterMethod::Nearest,
                rotation: iced::Radians(0.0),
                opacity: 1.0,
                snap: true,
                border_radius: iced::border::Radius::default(),
            };

            if let Some(clip) = super_intersect(title_bounds, list_bounds) {
                renderer.draw_image(image, img_bounds, clip);
            }
        } else {
            // Fallback (e.g. first frame or unsupported glyphs)
            let title_text = iced::advanced::text::Text {
                content: row.title.clone(),
                bounds: title_text_bounds,
                size: iced::Pixels(title_font_px),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: iced::Font::default(),
                align_x: iced::advanced::text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::advanced::text::Wrapping::None,
                hint_factor: Some(0.0),
            };
            renderer.fill_text(title_text, Point::new(title_bounds.x, title_text_y), title_fg, list_bounds);
        }
    }
}

impl Widget<LayerMessage, Theme, iced::Renderer> for LayerListWidget<'_> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<LayerListWidgetState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(LayerListWidgetState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _tree: &mut widget::Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let size = limits.max();
        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        // Keep viewport aware of size
        {
            let mut vp = self.viewport.borrow_mut();
            vp.set_visible_size(bounds.width, bounds.height);
        }

        renderer.with_layer(bounds, |r| {
            if self.rows.is_empty() {
                return;
            }

            let (first_visible, last_visible) = self.visible_range(bounds);

            let hovered_list_idx = self.hovered_list_idx.load(Ordering::Relaxed);
            let hovered_list_idx = if hovered_list_idx == u32::MAX {
                None
            } else {
                Some(hovered_list_idx as usize)
            };

            for list_idx in first_visible..last_visible {
                let row_bounds = self.row_bounds(bounds, list_idx);
                if row_bounds.y + row_bounds.height < bounds.y || row_bounds.y > bounds.y + bounds.height {
                    continue;
                }

                let row = &self.rows[list_idx];
                let is_selected = row.layer_index == self.current_layer;
                let is_hovered = hovered_list_idx == Some(list_idx);
                self.draw_row(r, theme, bounds, row_bounds, row, is_hovered, is_selected);
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, LayerMessage>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let state = tree.state.downcast_mut::<LayerListWidgetState>();

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let hovered = cursor
                    .position_in(bounds)
                    .map(|pos| {
                        let y = pos.y + self.viewport.borrow().scroll_y;
                        (y / LAYER_ROW_HEIGHT) as usize
                    })
                    .filter(|&idx| idx < self.rows.len())
                    .map(|idx| idx as u32)
                    .unwrap_or(u32::MAX);

                let prev = self.hovered_list_idx.swap(hovered, Ordering::Relaxed);
                if prev != hovered {
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // In paste mode, layer selection is disabled
                if self.paste_mode {
                    return;
                }

                if state.left_button_down {
                    return;
                }
                state.left_button_down = true;
                state.pressed_list_idx = None;
                state.pressed_pos = None;
                state.pressed_was_selected = false;

                if let Some(pos) = cursor.position_in(bounds) {
                    let scroll_offset = self.viewport.borrow().scroll_y;
                    let clicked_y = pos.y + scroll_offset;
                    let list_idx = (clicked_y / LAYER_ROW_HEIGHT) as usize;
                    if list_idx < self.rows.len() {
                        state.pressed_list_idx = Some(list_idx);
                        state.pressed_pos = Some(pos);

                        let row_bounds = self.row_bounds(bounds, list_idx);
                        let row = &self.rows[list_idx];
                        let toggle_bounds = self.visibility_toggle_bounds(row_bounds);

                        if cursor.is_over(toggle_bounds) {
                            shell.publish(LayerMessage::ToggleVisibility(row.layer_index));
                            state.pressed_list_idx = None;
                            state.pressed_pos = None;
                            return;
                        }

                        // Click selects; editing is handled on Release via double-click.
                        // IMPORTANT: Only allow EditLayer double-clicks when the row was already selected
                        // before the click started (prevents "too easy" accidental opens).
                        state.pressed_was_selected = row.layer_index == self.current_layer;
                        shell.publish(LayerMessage::Select(row.layer_index));
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.left_button_down = false;

                let Some(pressed_idx) = state.pressed_list_idx.take() else {
                    return;
                };

                let pressed_pos = state.pressed_pos.take();
                let pressed_was_selected = state.pressed_was_selected;

                let Some(pos) = cursor.position_in(bounds) else {
                    return;
                };

                // Cancel click if it was actually a drag.
                if let Some(pressed_pos) = pressed_pos {
                    let dx = pos.x - pressed_pos.x;
                    let dy = pos.y - pressed_pos.y;
                    if (dx * dx + dy * dy) > 16.0 {
                        return;
                    }
                }

                let scroll_offset = self.viewport.borrow().scroll_y;
                let released_y = pos.y + scroll_offset;
                let released_idx = (released_y / LAYER_ROW_HEIGHT) as usize;
                if released_idx != pressed_idx {
                    return;
                }

                if released_idx >= self.rows.len() {
                    return;
                }

                let row = &self.rows[released_idx];
                let now = std::time::Instant::now();

                // Only open the layer properties dialog on a true double-click.
                // The row must have been selected BEFORE this click cycle started.
                // This prevents: click to select → immediate second click opens dialog.
                // User must: click to select → release → click again → release → dialog opens.
                if pressed_was_selected {
                    // Check if this is a double-click (same layer, within 400ms of last click)
                    let is_double = state.last_click.map_or(false, |(last_idx, last_time)| {
                        last_idx == row.layer_index && now.duration_since(last_time).as_millis() < 400
                    });

                    if is_double {
                        // Reset after double-click to prevent triple-click triggering again
                        state.last_click = None;
                        shell.publish(LayerMessage::EditLayer(row.layer_index));
                    } else {
                        // Record this click for potential double-click
                        state.last_click = Some((row.layer_index, now));
                    }
                } else {
                    // Selection changed, reset double-click tracking
                    state.last_click = None;
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    let mut vp = self.viewport.borrow_mut();
                    match delta {
                        mouse::ScrollDelta::Lines { y, .. } => {
                            let scroll_delta = -y * LAYER_ROW_HEIGHT * 0.6;
                            vp.scroll_y_by_smooth(scroll_delta);
                        }
                        mouse::ScrollDelta::Pixels { y, .. } => {
                            vp.scroll_y_by(-y);
                        }
                    }
                    vp.scrollbar.mark_interaction(true);
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        if cursor.is_over(bounds) {
            return mouse::Interaction::Pointer;
        }
        mouse::Interaction::default()
    }
}

impl<'a> From<LayerListWidget<'a>> for Element<'a, LayerMessage> {
    fn from(widget: LayerListWidget<'a>) -> Self {
        Element::new(widget)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Layer list background shader (WGSL)
// ─────────────────────────────────────────────────────────────────────────────

#[repr(C, align(16))]
#[derive(Clone, Copy, Default, Debug)]
struct LayerListBackgroundUniforms {
    widget_origin: [f32; 2],
    widget_size: [f32; 2],

    scroll_y: f32,
    row_height: f32,

    row_count: u32,
    selected_row: u32,
    hovered_row: u32,
    _flags: u32,

    // WGSL aligns vec4<T> to 16 bytes. After the four u32s above we are at 40 bytes,
    // so we need 8 bytes padding to start the next vec4 on a 16-byte boundary.
    _pad0: [u32; 2],

    preview_rect: [f32; 4],
    atlas_size: [f32; 4],
    atlas_params: [u32; 4],

    preview_bg_color: [f32; 4],
    preview_border_color: [f32; 4],
    preview_style: [f32; 4],

    bg_color: [f32; 4],

    // Checkerboard colors for transparency (from icy_engine_gui::CheckerboardColors)
    checker_color1: [f32; 4],
    checker_color2: [f32; 4],
    // x=cell_size, y=enabled, z=unused, w=unused
    checker_params: [f32; 4],
}

unsafe impl bytemuck::Pod for LayerListBackgroundUniforms {}
unsafe impl bytemuck::Zeroable for LayerListBackgroundUniforms {}

#[derive(Clone)]
struct LayerListBackgroundProgram {
    row_count: u32,
    row_height: f32,
    scroll_y: f32,
    selected_row: u32,
    hovered_row: Arc<AtomicU32>,
    bg_color: Color,
    preview_bg_color: Color,
    preview_border_color: Color,
    preview_border_width: f32,
    preview_radius: f32,
    preview_atlas: Arc<Mutex<PreviewAtlasState>>,
    checkerboard_colors: CheckerboardColors,
}

impl iced::widget::shader::Program<LayerMessage> for LayerListBackgroundProgram {
    type State = ();
    type Primitive = LayerListBackgroundPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: iced::mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        let hovered_row = self.hovered_row.load(Ordering::Relaxed);
        LayerListBackgroundPrimitive {
            row_count: self.row_count,
            row_height: self.row_height,
            scroll_y: self.scroll_y,
            selected_row: self.selected_row,
            hovered_row,
            bg_color: self.bg_color,
            preview_bg_color: self.preview_bg_color,
            preview_border_color: self.preview_border_color,
            preview_border_width: self.preview_border_width,
            preview_radius: self.preview_radius,
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
            preview_atlas: self.preview_atlas.clone(),
            checkerboard_colors: self.checkerboard_colors.clone(),
        }
    }

    fn update(
        &self,
        _state: &mut Self::State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Option<iced::widget::Action<LayerMessage>> {
        None
    }

    fn mouse_interaction(&self, _state: &Self::State, _bounds: Rectangle, _cursor: iced::mouse::Cursor) -> iced::mouse::Interaction {
        iced::mouse::Interaction::default()
    }
}

#[derive(Clone, Debug)]
struct LayerListBackgroundPrimitive {
    row_count: u32,
    row_height: f32,
    scroll_y: f32,
    selected_row: u32,
    hovered_row: u32,
    bg_color: Color,
    preview_bg_color: Color,
    preview_border_color: Color,
    preview_border_width: f32,
    preview_radius: f32,
    uniform_offset_bytes: Arc<AtomicU32>,
    preview_atlas: Arc<Mutex<PreviewAtlasState>>,
    checkerboard_colors: CheckerboardColors,
}

impl iced::widget::shader::Primitive for LayerListBackgroundPrimitive {
    type Pipeline = LayerListBackgroundRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let (atlas_version, atlas_pixels) = {
            let atlas = self.preview_atlas.lock();
            (atlas.version, Arc::clone(&atlas.pixels))
        };

        let prev = pipeline.atlas_version.load(Ordering::Relaxed);
        if prev != atlas_version {
            queue.write_texture(
                iced::wgpu::TexelCopyTextureInfo {
                    texture: &pipeline.atlas_texture,
                    mip_level: 0,
                    origin: iced::wgpu::Origin3d::ZERO,
                    aspect: iced::wgpu::TextureAspect::All,
                },
                atlas_pixels.as_slice(),
                iced::wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(PREVIEW_ATLAS_W * 4),
                    rows_per_image: Some(PREVIEW_ATLAS_H),
                },
                iced::wgpu::Extent3d {
                    width: PREVIEW_ATLAS_W,
                    height: PREVIEW_ATLAS_H,
                    depth_or_array_layers: 1,
                },
            );

            pipeline.atlas_version.store(atlas_version, Ordering::Relaxed);
        }

        let scale = viewport.scale_factor();

        let origin_x = (bounds.x * scale).round();
        let origin_y = (bounds.y * scale).round();
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        let uniforms = LayerListBackgroundUniforms {
            widget_origin: [origin_x, origin_y],
            widget_size: [size_w, size_h],
            scroll_y: self.scroll_y * scale,
            row_height: self.row_height * scale,
            row_count: self.row_count,
            selected_row: self.selected_row,
            hovered_row: self.hovered_row,
            _flags: 0,
            _pad0: [0, 0],
            preview_rect: [
                (LAYER_ROW_PADDING as f32) * scale,
                (LAYER_ROW_PADDING as f32) * scale,
                PREVIEW_WIDTH * scale,
                PREVIEW_HEIGHT * scale,
            ],
            atlas_size: [PREVIEW_ATLAS_W as f32, PREVIEW_ATLAS_H as f32, PREVIEW_TEX_W as f32, PREVIEW_TEX_H as f32],
            atlas_params: {
                let atlas = self.preview_atlas.lock();
                [atlas.first_list_idx, atlas.row_count, PREVIEW_ATLAS_COLS, 0]
            },
            preview_bg_color: [
                self.preview_bg_color.r,
                self.preview_bg_color.g,
                self.preview_bg_color.b,
                self.preview_bg_color.a,
            ],
            preview_border_color: [
                self.preview_border_color.r,
                self.preview_border_color.g,
                self.preview_border_color.b,
                self.preview_border_color.a,
            ],
            preview_style: [self.preview_border_width * scale, self.preview_radius * scale, 0.0, 0.0],
            bg_color: [self.bg_color.r, self.bg_color.g, self.bg_color.b, self.bg_color.a],
            checker_color1: self.checkerboard_colors.color1_rgba(),
            checker_color2: self.checkerboard_colors.color2_rgba(),
            checker_params: [self.checkerboard_colors.cell_size / 2.0, 1.0, 0.0, 0.0],
        };

        let slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let offset = (slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(offset as u32, Ordering::Relaxed);

        queue.write_buffer(&pipeline.uniform_buffer, offset, bytemuck::bytes_of(&uniforms));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Layer List Background Render Pass"),
            color_attachments: &[Some(iced::wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: iced::wgpu::Operations {
                    load: iced::wgpu::LoadOp::Load,
                    store: iced::wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if clip_bounds.width > 0 && clip_bounds.height > 0 {
            pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            pass.set_viewport(
                clip_bounds.x as f32,
                clip_bounds.y as f32,
                clip_bounds.width as f32,
                clip_bounds.height as f32,
                0.0,
                1.0,
            );
            pass.set_pipeline(&pipeline.pipeline);
            let offset = self.uniform_offset_bytes.load(Ordering::Relaxed);
            pass.set_bind_group(0, &pipeline.bind_group, &[offset]);
            pass.draw(0..6, 0..1);
        }
    }
}

pub struct LayerListBackgroundRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    atlas_texture: iced::wgpu::Texture,
    atlas_view: iced::wgpu::TextureView,
    atlas_sampler: iced::wgpu::Sampler,
    atlas_version: AtomicU64,
    uniform_stride: u64,
    uniform_capacity: u32,
    next_uniform: AtomicU32,
}

fn align_up(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    ((value + alignment - 1) / alignment) * alignment
}

impl iced::widget::shader::Pipeline for LayerListBackgroundRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Layer List Background Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<LayerListBackgroundUniforms>() as u64;
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Layer List Background Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("Layer List Preview Atlas"),
            size: iced::wgpu::Extent3d {
                width: PREVIEW_ATLAS_W,
                height: PREVIEW_ATLAS_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: iced::wgpu::TextureDimension::D2,
            format: iced::wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Layer List Preview Atlas Sampler"),
            address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
            mag_filter: iced::wgpu::FilterMode::Linear,
            min_filter: iced::wgpu::FilterMode::Linear,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Layer List Background Bind Group Layout"),
            entries: &[
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: iced::wgpu::BindingType::Buffer {
                        ty: iced::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(uniform_size),
                    },
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Texture {
                        sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: iced::wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("Layer List Background Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                iced::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(uniform_size),
                    }),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: iced::wgpu::BindingResource::TextureView(&atlas_view),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: iced::wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Layer List Background Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Layer List Background Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(iced::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(iced::wgpu::ColorTargetState {
                    format,
                    blend: Some(iced::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: iced::wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: iced::wgpu::PrimitiveState {
                topology: iced::wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_version: AtomicU64::new(0),
            uniform_stride,
            uniform_capacity,
            next_uniform: AtomicU32::new(0),
        }
    }
}

fn super_intersect(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 <= x1 || y2 <= y1 {
        None
    } else {
        Some(Rectangle {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        })
    }
}
