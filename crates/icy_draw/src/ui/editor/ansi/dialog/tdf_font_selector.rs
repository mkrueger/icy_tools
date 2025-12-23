//! TDF/Figlet Font Selector Dialog
//!
//! A beautiful font selector with:
//! - Search/filter field
//! - Font type filters (Outline, Block, Color, Figlet)
//! - Virtualized font list (only renders visible items)
//! - Keyboard navigation support
//! - Viewport-based scrolling with overlay scrollbar

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

use iced::{
    advanced::{
        image::{self as adv_image, Renderer as _},
        layout::{self, Layout},
        renderer::{self, Renderer as _},
        text::Renderer as _,
        widget::{self, Widget},
    },
    mouse,
    widget::{button, column, container, row, text, text_input, Space},
    Alignment, Background, Border, Color, Element, Event, Length, Point, Rectangle, Size, Theme,
};
use icy_engine_gui::{
    ui::{
        dialog_area, modal_container, primary_button, secondary_button, separator, ButtonType, Dialog, DialogAction, DIALOG_SPACING, DIALOG_WIDTH_XARGLE,
        TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
    },
    wrap_with_scrollbars, Viewport,
};

use crate::ui::editor::ansi::AnsiEditorMessage;
use crate::ui::Message;
use crate::SharedFontLibrary;
use crate::LANGUAGE_LOADER;
use i18n_embed_fl::fl;

/// Helper function to wrap TdfFontSelectorMessage in the full Message path
fn tdf_msg(m: TdfFontSelectorMessage) -> Message {
    Message::AnsiEditor(AnsiEditorMessage::TdfFontSelector(m))
}

// ============================================================================
// Constants
// ============================================================================

/// Dialog dimensions
const DIALOG_WIDTH: f32 = DIALOG_WIDTH_XARGLE;
const LIST_HEIGHT: f32 = 380.0;

/// Font list item height
const FONT_ITEM_HEIGHT: f32 = 100.0;

/// List padding inside each row
const ROW_PADDING_X: f32 = 12.0;
const ROW_PADDING_Y: f32 = 8.0;
const PREVIEW_MAX_W: f32 = 300.0;
const PREVIEW_MAX_H: f32 = 84.0;
const PREVIEW_GAP: f32 = 6.0;

// ============================================================================
// Font Type
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontType {
    Outline,
    Block,
    Color,
    Figlet,
}

impl FontType {
    fn label(&self) -> String {
        match self {
            FontType::Outline => fl!(LANGUAGE_LOADER, "tdf-font-selector-type_outline"),
            FontType::Block => fl!(LANGUAGE_LOADER, "tdf-font-selector-type_block"),
            FontType::Color => fl!(LANGUAGE_LOADER, "tdf-font-selector-type_color"),
            FontType::Figlet => fl!(LANGUAGE_LOADER, "tdf-font-selector-type_figlet"),
        }
    }

    fn from_retrofont(font: &retrofont::Font) -> Self {
        use retrofont::tdf::TdfFontType;
        match font {
            retrofont::Font::Figlet(_) => FontType::Figlet,
            retrofont::Font::Tdf(tdf) => match tdf.font_type() {
                TdfFontType::Outline => FontType::Outline,
                TdfFontType::Block => FontType::Block,
                TdfFontType::Color => FontType::Color,
            },
        }
    }

    fn color(&self) -> Color {
        match self {
            FontType::Outline => Color::from_rgb8(100, 150, 200),
            FontType::Block => Color::from_rgb8(150, 100, 200),
            FontType::Color => Color::from_rgb8(200, 150, 100),
            FontType::Figlet => Color::from_rgb8(100, 200, 150),
        }
    }
}

// ============================================================================
// Messages
// ============================================================================

#[derive(Clone, Debug)]
pub enum TdfFontSelectorMessage {
    /// Search filter changed
    FilterChanged(String),
    /// Toggle font type filter
    ToggleOutline,
    ToggleBlock,
    ToggleColor,
    ToggleFiglet,
    /// Select a font by index
    SelectFont(usize),
    /// Confirm selection - includes selected font index
    Confirm(i32),
    /// Cancel dialog
    Cancel,
    /// Export the currently selected font
    Export,
    /// Keyboard navigation
    KeyUp,
    KeyDown,
    KeyHome,
    KeyEnd,
    KeyPageUp,
    KeyPageDown,
}

// ============================================================================
// Cached Font Info (to avoid locks during rendering)
// ============================================================================

#[derive(Clone)]
struct CachedFontInfo {
    name: String,
    font_type: FontType,
}

// ============================================================================
// Dialog State
// ============================================================================

pub struct TdfFontSelectorDialog {
    /// Reference to the shared font library (previews are cached there)
    font_library: SharedFontLibrary,
    /// Currently selected font index
    selected_font: i32,
    /// Search filter
    filter: String,
    /// Font type filters
    show_outline: bool,
    show_block: bool,
    show_color: bool,
    show_figlet: bool,
    /// Keyboard cursor in filtered list
    keyboard_cursor: usize,
    /// Cached filtered font indices
    filtered_fonts: Vec<usize>,
    /// Cached font info (font_index -> info) - avoids locks during view
    font_info_cache: HashMap<usize, CachedFontInfo>,
    /// Viewport for scrolling (with integrated scrollbar state)
    viewport: RefCell<Viewport>,
}

// ============================================================================
// Virtualized Font List Widget (advanced) - draws preview images + charset
// ============================================================================

struct FontListWidget<'a> {
    font_library: &'a SharedFontLibrary,
    filtered_fonts: &'a [usize],
    font_info_cache: &'a HashMap<usize, CachedFontInfo>,
    selected_font: i32,
    keyboard_cursor: usize,
    has_filter: bool,
    viewport: &'a RefCell<Viewport>,
}

impl<'a> FontListWidget<'a> {
    fn visible_range(&self, bounds: Rectangle) -> (usize, usize) {
        let scroll_offset = self.viewport.borrow().scroll_y;
        let first_visible = (scroll_offset / FONT_ITEM_HEIGHT).floor().max(0.0) as usize;
        let visible_count = (bounds.height / FONT_ITEM_HEIGHT).ceil() as usize + 2;
        let last_visible = (first_visible + visible_count).min(self.filtered_fonts.len());
        (first_visible, last_visible)
    }

    fn intersect(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
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

    fn draw_background(&self, r: &mut iced::Renderer, theme: &Theme, bounds: Rectangle) {
        r.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            theme.extended_palette().background.weak.color,
        );
    }

    fn draw_empty_state(&self, r: &mut iced::Renderer, theme: &Theme, bounds: Rectangle) {
        // Center horizontally + vertically (as a group) within the list area.
        let show_filter_hint = self.has_filter;
        let line_h = 28.0;
        let gap = 6.0;
        let total_h = if show_filter_hint { line_h * 2.0 + gap } else { line_h };
        let start_y = bounds.y + (bounds.height - total_h).max(0.0) * 0.5;

        let line_bounds = |y: f32| Rectangle {
            x: bounds.x,
            y,
            width: bounds.width,
            height: line_h,
        };

        let count_rect = line_bounds(start_y);
        let count_text = iced::advanced::text::Text {
            content: "0 fonts".to_string(),
            bounds: count_rect.size(),
            size: iced::Pixels(TEXT_SIZE_NORMAL + 4.0),
            line_height: iced::advanced::text::LineHeight::Relative(1.0),
            font: iced::Font::default(),
            align_x: iced::advanced::text::Alignment::Center,
            align_y: iced::alignment::Vertical::Center,
            shaping: iced::advanced::text::Shaping::Advanced,
            wrapping: iced::advanced::text::Wrapping::None,
            hint_factor: Some(0.0),
        };
        r.fill_text(
            count_text,
            Point::new(count_rect.x, count_rect.y),
            theme.extended_palette().background.strong.text,
            count_rect,
        );

        if show_filter_hint {
            let hint_rect = line_bounds(start_y + line_h + gap);
            let empty_text = iced::advanced::text::Text {
                content: "No fonts match filter".to_string(),
                bounds: hint_rect.size(),
                size: iced::Pixels(TEXT_SIZE_SMALL),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: iced::Font::default(),
                align_x: iced::advanced::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::advanced::text::Wrapping::None,
                hint_factor: Some(0.0),
            };
            r.fill_text(
                empty_text,
                Point::new(hint_rect.x, hint_rect.y),
                theme.extended_palette().background.weak.text,
                hint_rect,
            );
        }
    }

    fn draw_selection_background(&self, r: &mut iced::Renderer, theme: &Theme, row_bounds: Rectangle, is_selected: bool, is_cursor: bool) {
        if !(is_selected || is_cursor) {
            return;
        }

        let bg = if is_selected {
            theme.extended_palette().primary.weak.color
        } else {
            theme.extended_palette().secondary.weak.color
        };

        r.fill_quad(
            renderer::Quad {
                bounds: row_bounds,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            bg,
        );
    }

    fn draw_separator(&self, r: &mut iced::Renderer, row_bounds: Rectangle) {
        r.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: row_bounds.x,
                    y: row_bounds.y + row_bounds.height - 1.0,
                    width: row_bounds.width,
                    height: 1.0,
                },
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            Color::from_rgba8(100, 100, 100, 0.25),
        );
    }

    fn preview_area_for_row(&self, row_bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: row_bounds.x + row_bounds.width - PREVIEW_MAX_W - ROW_PADDING_X,
            y: row_bounds.y + (row_bounds.height - PREVIEW_MAX_H) / 2.0,
            width: PREVIEW_MAX_W,
            height: PREVIEW_MAX_H,
        }
    }

    fn left_area_width(&self, preview_area: Rectangle, left_x: f32) -> f32 {
        (preview_area.x - left_x - PREVIEW_GAP).max(0.0)
    }

    fn draw_name(&self, r: &mut iced::Renderer, theme: &Theme, left_x: f32, top_y: f32, left_area_w: f32, font_name: &str) {
        let name_clip = Rectangle {
            x: left_x,
            y: top_y,
            width: left_area_w,
            height: 24.0,
        };

        let name_text = iced::advanced::text::Text {
            content: font_name.to_string(),
            bounds: Size::new(left_area_w, 24.0),
            size: iced::Pixels(TEXT_SIZE_NORMAL),
            line_height: iced::advanced::text::LineHeight::Relative(1.0),
            font: iced::Font::default(),
            align_x: iced::advanced::text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: iced::advanced::text::Shaping::Advanced,
            wrapping: iced::advanced::text::Wrapping::None,
            hint_factor: Some(0.0),
        };

        r.fill_text(name_text, Point::new(left_x, top_y), theme.extended_palette().background.strong.text, name_clip);
    }

    fn draw_charset_preview(&self, r: &mut iced::Renderer, theme: &Theme, bounds: Rectangle, charset_bounds: Rectangle, font: &retrofont::Font) {
        let supported_color = theme.extended_palette().background.base.text;
        let unsupported_color = theme.extended_palette().background.weak.text;

        let char_w = (TEXT_SIZE_SMALL * 0.62).max(1.0);
        let line_h = (TEXT_SIZE_SMALL * 1.15).max(1.0);

        r.with_layer(charset_bounds, |rr| {
            let mut col: usize = 0;
            let mut row: usize = 0;
            for ch in '!'..='~' {
                let has = font.has_char(ch);
                let color = if has { supported_color } else { unsupported_color };

                let x = charset_bounds.x + col as f32 * char_w;
                let y = charset_bounds.y + row as f32 * line_h;

                let t = iced::advanced::text::Text {
                    content: ch.to_string(),
                    bounds: Size::new(char_w, line_h),
                    size: iced::Pixels(TEXT_SIZE_SMALL),
                    line_height: iced::advanced::text::LineHeight::Relative(1.15),
                    font: iced::Font::MONOSPACE,
                    align_x: iced::advanced::text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: iced::advanced::text::Shaping::Advanced,
                    wrapping: iced::advanced::text::Wrapping::None,
                    hint_factor: Some(0.0),
                };
                rr.fill_text(t, Point::new(x, y), color, bounds);

                col += 1;
                if col >= 32 {
                    col = 0;
                    row += 1;
                    if row >= 3 {
                        break;
                    }
                }
            }
        });
    }

    fn draw_preview(&self, r: &mut iced::Renderer, theme: &Theme, list_bounds: Rectangle, preview_area: Rectangle, preview: Option<&crate::FontPreview>) {
        let Some(preview_clip) = Self::intersect(preview_area, list_bounds) else {
            return;
        };

        r.with_layer(preview_clip, |r2| {
            r2.fill_quad(
                renderer::Quad {
                    bounds: preview_area,
                    border: iced::Border::default().rounded(2.0).width(1.0).color(Color::from_rgba8(50, 50, 50, 0.6)),
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                Color::from_rgb(0.05, 0.05, 0.05),
            );

            if let Some(preview) = preview {
                let scale = (preview_area.width / preview.width as f32)
                    .min(preview_area.height / preview.height as f32)
                    .min(1.0);
                let w = preview.width as f32 * scale;
                let h = preview.height as f32 * scale;
                let img_bounds = Rectangle {
                    x: preview_area.x + (preview_area.width - w) / 2.0,
                    y: preview_area.y + (preview_area.height - h) / 2.0,
                    width: w,
                    height: h,
                };

                let image = adv_image::Image::<iced::widget::image::Handle> {
                    handle: preview.handle.clone(),
                    filter_method: adv_image::FilterMethod::Linear,
                    rotation: iced::Radians(0.0),
                    opacity: 1.0,
                    snap: true,
                    border_radius: iced::border::Radius::default(),
                };

                r2.draw_image(image, img_bounds, preview_clip);
            } else {
                let ph = iced::advanced::text::Text {
                    content: "Loading…".to_string(),
                    bounds: preview_area.size(),
                    size: iced::Pixels(TEXT_SIZE_SMALL),
                    line_height: iced::advanced::text::LineHeight::Relative(1.0),
                    font: iced::Font::default(),
                    align_x: iced::advanced::text::Alignment::Center,
                    align_y: iced::alignment::Vertical::Center,
                    shaping: iced::advanced::text::Shaping::Advanced,
                    wrapping: iced::advanced::text::Wrapping::None,
                    hint_factor: Some(0.0),
                };

                r2.fill_text(
                    ph,
                    Point::new(preview_area.x, preview_area.y),
                    theme.extended_palette().background.weak.text,
                    preview_clip,
                );
            }
        });
    }

    fn draw_row(
        &self,
        r: &mut iced::Renderer,
        theme: &Theme,
        list_bounds: Rectangle,
        row_bounds: Rectangle,
        list_idx: usize,
        font_idx: usize,
        font_name: &str,
        font_type: FontType,
        lib: &crate::TextArtFontLibrary,
    ) {
        let is_selected = self.selected_font == font_idx as i32;
        let is_cursor = self.keyboard_cursor == list_idx;

        self.draw_selection_background(r, theme, row_bounds, is_selected, is_cursor);
        self.draw_separator(r, row_bounds);

        let preview_area = self.preview_area_for_row(row_bounds);
        let left_x = row_bounds.x + ROW_PADDING_X;
        let top_y = row_bounds.y + ROW_PADDING_Y;
        let left_area_w = self.left_area_width(preview_area, left_x);

        self.draw_name(r, theme, left_x, top_y, left_area_w, font_name);
        self.draw_badge(r, theme, row_bounds, &font_type.label().to_uppercase(), font_type.color());

        if let Some(font) = lib.get_font(font_idx) {
            let charset_bounds = Rectangle {
                x: left_x,
                y: top_y + 26.0,
                width: left_area_w,
                height: row_bounds.height - (ROW_PADDING_Y * 2.0 + 26.0),
            };
            self.draw_charset_preview(r, theme, list_bounds, charset_bounds, font);
        }

        self.draw_preview(r, theme, list_bounds, preview_area, lib.get_preview(font_idx));
    }

    fn draw_badge(&self, renderer: &mut iced::Renderer, _theme: &Theme, bounds: Rectangle, label: &str, color: Color) {
        let badge_w = 58.0;
        let badge_h = 18.0;
        let x = bounds.x + bounds.width - badge_w - ROW_PADDING_X;
        let y = bounds.y + ROW_PADDING_Y;

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x,
                    y,
                    width: badge_w,
                    height: badge_h,
                },
                border: iced::Border::default().rounded(4.0).width(1.0).color(color),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            Color::from_rgba(color.r, color.g, color.b, 0.18),
        );

        let text = iced::advanced::text::Text {
            content: label.to_string(),
            bounds: Size::new(badge_w, badge_h),
            size: iced::Pixels(TEXT_SIZE_SMALL),
            line_height: iced::advanced::text::LineHeight::Relative(1.0),
            font: iced::Font::default(),
            align_x: iced::advanced::text::Alignment::Center,
            align_y: iced::alignment::Vertical::Center,
            shaping: iced::advanced::text::Shaping::Advanced,
            wrapping: iced::advanced::text::Wrapping::None,
            hint_factor: Some(0.0),
        };

        let clip_bounds = Rectangle {
            x,
            y,
            width: badge_w,
            height: badge_h,
        };
        renderer.fill_text(text, Point::new(x, y), color, clip_bounds);
    }
}

impl Widget<Message, Theme, iced::Renderer> for FontListWidget<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(LIST_HEIGHT))
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

        // Clip all list contents to the list bounds (scissor)
        renderer.with_layer(bounds, |r| {
            self.draw_background(r, theme, bounds);

            if self.filtered_fonts.is_empty() {
                self.draw_empty_state(r, theme, bounds);
                return;
            }

            let scroll_offset = self.viewport.borrow().scroll_y;
            let (first_visible, last_visible) = self.visible_range(bounds);

            // Only lock for visible items
            let lib = self.font_library.read();

            for list_idx in first_visible..last_visible {
                let font_idx = self.filtered_fonts[list_idx];
                let y = bounds.y + list_idx as f32 * FONT_ITEM_HEIGHT - scroll_offset;
                let row_bounds = Rectangle {
                    x: bounds.x,
                    y,
                    width: bounds.width,
                    height: FONT_ITEM_HEIGHT,
                };

                if row_bounds.y + row_bounds.height < bounds.y || row_bounds.y > bounds.y + bounds.height {
                    continue;
                }

                let info = self.font_info_cache.get(&font_idx);
                let font_name = info.map(|i| i.name.as_str()).unwrap_or("Unknown");
                let font_type = info.map(|i| i.font_type).unwrap_or(FontType::Figlet);

                self.draw_row(r, theme, bounds, row_bounds, list_idx, font_idx, font_name, font_type, &lib);
            }
        });
    }

    fn update(
        &mut self,
        _tree: &mut widget::Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let scroll_offset = self.viewport.borrow().scroll_y;
                    let clicked_y = pos.y + scroll_offset;
                    let list_idx = (clicked_y / FONT_ITEM_HEIGHT) as usize;
                    if list_idx < self.filtered_fonts.len() {
                        let font_idx = self.filtered_fonts[list_idx];
                        // If clicking on already selected font, confirm and close
                        if self.selected_font == font_idx as i32 {
                            shell.publish(tdf_msg(TdfFontSelectorMessage::Confirm(self.selected_font)));
                        } else {
                            shell.publish(tdf_msg(TdfFontSelectorMessage::SelectFont(font_idx)));
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    let mut vp = self.viewport.borrow_mut();
                    match delta {
                        mouse::ScrollDelta::Lines { y, .. } => {
                            let scroll_delta = -y * FONT_ITEM_HEIGHT * 0.6;
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
            // Prefetch previews when anything causes redraws (scrollbar drag/animation)
            Event::Window(iced::window::Event::RedrawRequested(_)) => {
                let _changed = self.viewport.borrow().changed.swap(false, Ordering::Relaxed);

                // Sync preview generation (fast) for visible items.
                let (first, last) = self.visible_range(bounds);
                if first < last {
                    let mut missing: Vec<usize> = Vec::new();
                    {
                        let lib = self.font_library.read();
                        for list_idx in first..last {
                            let font_idx = self.filtered_fonts[list_idx];
                            if !lib.has_preview(font_idx) {
                                missing.push(font_idx);
                            }
                        }
                    }

                    if !missing.is_empty() {
                        let mut lib = self.font_library.write();
                        for font_idx in missing {
                            lib.generate_preview(font_idx);
                        }
                        shell.request_redraw();
                    }
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

impl<'a> From<FontListWidget<'a>> for Element<'a, Message> {
    fn from(widget: FontListWidget<'a>) -> Self {
        Element::new(widget)
    }
}

impl TdfFontSelectorDialog {
    pub fn new(font_library: SharedFontLibrary, selected_font: i32) -> Self {
        let mut viewport = Viewport::default();
        viewport.visible_height = LIST_HEIGHT;
        viewport.content_height = 0.0; // Will be updated based on font count

        let mut dialog = Self {
            font_library,
            selected_font,
            filter: String::new(),
            show_outline: true,
            show_block: true,
            show_color: true,
            show_figlet: true,
            keyboard_cursor: 0, // Will be set by update_filtered_fonts based on selected_font
            filtered_fonts: Vec::new(),
            font_info_cache: HashMap::new(),
            viewport: RefCell::new(viewport),
        };

        dialog.cache_all_font_info();
        dialog.update_filtered_fonts();
        dialog.scroll_to_cursor();
        dialog
    }

    /// Update viewport content size based on filtered fonts
    fn update_viewport_content_size(&self) {
        let total_height = self.filtered_fonts.len() as f32 * FONT_ITEM_HEIGHT;
        let mut viewport = self.viewport.borrow_mut();
        viewport.content_height = total_height;
        viewport.sync_scrollbar_position();
        viewport.changed.store(true, Ordering::Relaxed);
    }

    /// Ensure keyboard cursor is visible (immediate scroll, for arrow keys)
    fn scroll_to_cursor(&self) {
        self.scroll_to_cursor_impl(false);
    }

    /// Ensure keyboard cursor is visible (smooth scroll, for page/home/end)
    fn scroll_to_cursor_smooth(&self) {
        self.scroll_to_cursor_impl(true);
    }

    /// Implementation of scroll to cursor
    fn scroll_to_cursor_impl(&self, smooth: bool) {
        let cursor_y = self.keyboard_cursor as f32 * FONT_ITEM_HEIGHT;
        let mut viewport = self.viewport.borrow_mut();
        let visible_h = viewport.visible_height.max(0.0);

        let mut new_scroll_y = viewport.scroll_y;

        // If cursor is above visible area, scroll up
        if cursor_y < viewport.scroll_y {
            new_scroll_y = cursor_y;
        }

        // If cursor is below visible area, scroll down
        if cursor_y + FONT_ITEM_HEIGHT > viewport.scroll_y + visible_h {
            new_scroll_y = cursor_y + FONT_ITEM_HEIGHT - visible_h;
        }

        // Clamp to valid range
        let max_scroll = (viewport.content_height - visible_h).max(0.0);
        new_scroll_y = new_scroll_y.clamp(0.0, max_scroll);

        if smooth {
            viewport.scroll_y_to_smooth(new_scroll_y);
        } else {
            viewport.scroll_y_to(new_scroll_y);
        }

        // Sync scrollbar and mark as changed so UI updates
        viewport.sync_scrollbar_position();
        viewport.changed.store(true, Ordering::Relaxed);
    }

    /// Cache info for all fonts (called once at dialog creation)
    fn cache_all_font_info(&mut self) {
        let lib = self.font_library.read();
        for idx in 0..lib.font_count() {
            if let Some(font) = lib.get_font(idx) {
                let font_type = FontType::from_retrofont(font);
                self.font_info_cache.insert(
                    idx,
                    CachedFontInfo {
                        name: font.name().to_string(),
                        font_type,
                    },
                );
            }
        }
    }

    /// Select a font by index
    pub fn select_font(&mut self, idx: usize) {
        if let Some(pos) = self.filtered_fonts.iter().position(|&i| i == idx) {
            self.set_cursor_and_selected(pos);
        } else {
            self.selected_font = idx as i32;
        }
    }

    fn set_cursor_and_selected(&mut self, cursor: usize) {
        self.keyboard_cursor = cursor;
        if let Some(&idx) = self.filtered_fonts.get(self.keyboard_cursor) {
            self.selected_font = idx as i32;
        }
        self.scroll_to_cursor();
    }

    fn set_cursor_and_selected_smooth(&mut self, cursor: usize) {
        self.keyboard_cursor = cursor;
        if let Some(&idx) = self.filtered_fonts.get(self.keyboard_cursor) {
            self.selected_font = idx as i32;
        }
        self.scroll_to_cursor_smooth();
    }

    /// Export the currently selected font to a file
    fn export_selected_font(&self) {
        if self.selected_font < 0 {
            return;
        }
        let font_idx = self.selected_font as usize;

        // Get font info
        let lib = self.font_library.read();
        let Some(font) = lib.get_font(font_idx) else {
            return;
        };

        let font_name = font.name().to_string();
        let extension = font.default_extension();

        // Serialize font to bytes
        let Ok(bytes) = font.to_bytes() else {
            log::error!("Failed to serialize font: {}", font_name);
            return;
        };

        // Must drop the lock before opening the dialog (which blocks)
        drop(lib);

        // Open save dialog
        let file_dialog = rfd::FileDialog::new()
            .set_title(fl!(LANGUAGE_LOADER, "tdf-font-selector-export_title"))
            .set_file_name(format!("{}.{}", font_name, extension))
            .add_filter("Font file", &[extension]);

        if let Some(path) = file_dialog.save_file() {
            if let Err(e) = std::fs::write(&path, &bytes) {
                log::error!("Failed to write font file: {}", e);
            } else {
                log::info!("Exported font to: {}", path.display());
            }
        }
    }

    /// Set the search filter
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.update_filtered_fonts();
    }

    /// Toggle outline font filter
    pub fn toggle_outline(&mut self) {
        self.show_outline = !self.show_outline;
        self.update_filtered_fonts();
    }

    /// Toggle block font filter
    pub fn toggle_block(&mut self) {
        self.show_block = !self.show_block;
        self.update_filtered_fonts();
    }

    /// Toggle color font filter
    pub fn toggle_color(&mut self) {
        self.show_color = !self.show_color;
        self.update_filtered_fonts();
    }

    /// Toggle figlet font filter
    pub fn toggle_figlet(&mut self) {
        self.show_figlet = !self.show_figlet;
        self.update_filtered_fonts();
    }

    /// Update the list of filtered fonts based on current filter settings
    /// Uses cached font info - no lock required!
    fn update_filtered_fonts(&mut self) {
        self.filtered_fonts.clear();

        let filter_lower = self.filter.to_lowercase();

        for (&idx, info) in &self.font_info_cache {
            // Check name filter
            if !self.filter.is_empty() {
                let name_lower = info.name.to_lowercase();
                if !name_lower.contains(&filter_lower) {
                    continue;
                }
            }

            // Check type filter
            let visible = match info.font_type {
                FontType::Outline => self.show_outline,
                FontType::Block => self.show_block,
                FontType::Color => self.show_color,
                FontType::Figlet => self.show_figlet,
            };

            if visible {
                self.filtered_fonts.push(idx);
            }
        }

        // Sort for consistent ordering
        self.filtered_fonts.sort();

        // Find position of selected font in filtered list and set cursor there
        if !self.filtered_fonts.is_empty() {
            if let Some(pos) = self.filtered_fonts.iter().position(|&idx| idx as i32 == self.selected_font) {
                self.keyboard_cursor = pos;
            } else {
                // Selected font not in filtered list, clamp cursor
                self.keyboard_cursor = self.keyboard_cursor.min(self.filtered_fonts.len() - 1);
            }
        } else {
            self.keyboard_cursor = 0;
        }

        // Update viewport content size
        self.update_viewport_content_size();

        // Keep selection visible (or reset scroll for empty)
        if !self.filtered_fonts.is_empty() {
            self.scroll_to_cursor();
        } else {
            let mut viewport = self.viewport.borrow_mut();
            viewport.scroll_y = 0.0;
            viewport.sync_scrollbar_position();
            viewport.changed.store(true, Ordering::Relaxed);
        }
    }

    /// Handle keyboard navigation
    fn handle_key(&mut self, key: &iced::keyboard::Key, _modifiers: &iced::keyboard::Modifiers) -> bool {
        use iced::keyboard::key::Named;
        use iced::keyboard::Key;

        if self.filtered_fonts.is_empty() {
            return false;
        }

        match key {
            Key::Named(Named::ArrowUp) => {
                if self.keyboard_cursor > 0 {
                    self.set_cursor_and_selected(self.keyboard_cursor - 1);
                }
                true
            }
            Key::Named(Named::ArrowDown) => {
                if self.keyboard_cursor < self.filtered_fonts.len() - 1 {
                    self.set_cursor_and_selected(self.keyboard_cursor + 1);
                }
                true
            }
            Key::Named(Named::Home) => {
                self.set_cursor_and_selected(0);
                true
            }
            Key::Named(Named::End) => {
                self.set_cursor_and_selected(self.filtered_fonts.len() - 1);
                true
            }
            Key::Named(Named::PageUp) => {
                let page_items = (LIST_HEIGHT / FONT_ITEM_HEIGHT) as usize;
                self.set_cursor_and_selected(self.keyboard_cursor.saturating_sub(page_items));
                true
            }
            Key::Named(Named::PageDown) => {
                let page_items = (LIST_HEIGHT / FONT_ITEM_HEIGHT) as usize;
                self.set_cursor_and_selected((self.keyboard_cursor + page_items).min(self.filtered_fonts.len() - 1));
                true
            }
            _ => false,
        }
    }

    /// Create the canvas state for rendering
    fn list_widget(&self) -> Element<'_, Message> {
        FontListWidget {
            font_library: &self.font_library,
            filtered_fonts: &self.filtered_fonts,
            font_info_cache: &self.font_info_cache,
            selected_font: self.selected_font,
            keyboard_cursor: self.keyboard_cursor,
            has_filter: !self.filter.is_empty(),
            viewport: &self.viewport,
        }
        .into()
    }
}

impl Dialog<Message> for TdfFontSelectorDialog {
    fn view(&self) -> Element<'_, Message> {
        // Search bar
        let search_input = text_input(&fl!(LANGUAGE_LOADER, "tdf-font-selector-filter_placeholder"), &self.filter)
            .on_input(|s| tdf_msg(TdfFontSelectorMessage::FilterChanged(s)))
            .width(Length::Fixed(200.0))
            .padding(DIALOG_SPACING as u16)
            .size(TEXT_SIZE_NORMAL);

        // Filter buttons
        let outline_btn = filter_toggle_button(
            fl!(LANGUAGE_LOADER, "tdf-font-selector-type_outline"),
            self.show_outline,
            tdf_msg(TdfFontSelectorMessage::ToggleOutline),
        );
        let block_btn = filter_toggle_button(
            fl!(LANGUAGE_LOADER, "tdf-font-selector-type_block"),
            self.show_block,
            tdf_msg(TdfFontSelectorMessage::ToggleBlock),
        );
        let color_btn = filter_toggle_button(
            fl!(LANGUAGE_LOADER, "tdf-font-selector-type_color"),
            self.show_color,
            tdf_msg(TdfFontSelectorMessage::ToggleColor),
        );
        let figlet_btn = filter_toggle_button(
            fl!(LANGUAGE_LOADER, "tdf-font-selector-type_figlet"),
            self.show_figlet,
            tdf_msg(TdfFontSelectorMessage::ToggleFiglet),
        );

        let filter_row = row![search_input, Space::new().width(DIALOG_SPACING), outline_btn, block_btn, color_btn, figlet_btn,]
            .spacing(DIALOG_SPACING / 2.0)
            .align_y(Alignment::Center);

        // Font count
        let font_count_text = text(fl!(LANGUAGE_LOADER, "tdf-font-selector-font_count", count = self.filtered_fonts.len())).size(TEXT_SIZE_SMALL);

        let header = row![filter_row, Space::new().width(Length::Fill), font_count_text,].align_y(Alignment::Center);

        // Font list canvas (virtualized)
        let font_list: Element<'_, Message> = self.list_widget();

        // Wrap canvas with scrollbar overlay
        let needs_scrollbar = self.filtered_fonts.len() as f32 * FONT_ITEM_HEIGHT > LIST_HEIGHT;
        let canvas_with_scrollbar = wrap_with_scrollbars(font_list, &self.viewport, needs_scrollbar, false);

        let list_container = container(canvas_with_scrollbar)
            .width(Length::Fill)
            .height(Length::Fixed(LIST_HEIGHT))
            .style(|theme: &Theme| container::Style {
                background: Some(Background::Color(theme.extended_palette().background.weak.color)),
                border: Border::default().rounded(4).width(1).color(theme.extended_palette().background.strong.color),
                ..Default::default()
            });

        // Content area (filter + list)
        let content = column![header, Space::new().height(DIALOG_SPACING), list_container,];

        let content_area = dialog_area(content.into());

        // Button row
        let button_row = row![
            secondary_button(fl!(LANGUAGE_LOADER, "tdf-font-selector-export"), Some(tdf_msg(TdfFontSelectorMessage::Export))),
            Space::new().width(Length::Fill),
            secondary_button(format!("{}", ButtonType::Cancel), Some(tdf_msg(TdfFontSelectorMessage::Cancel))),
            primary_button(
                format!("{}", ButtonType::Ok),
                Some(tdf_msg(TdfFontSelectorMessage::Confirm(self.selected_font)))
            ),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let button_area = dialog_area(button_row.into());

        // Assemble dialog: content, separator, buttons
        let dialog_column = column![container(content_area).height(Length::Shrink), separator(), button_area,];

        modal_container(dialog_column.into(), DIALOG_WIDTH).into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::AnsiEditor(AnsiEditorMessage::TdfFontSelector(msg)) = message else {
            return None;
        };

        match msg {
            TdfFontSelectorMessage::FilterChanged(s) => {
                self.filter = s.clone();
                self.update_filtered_fonts();
                // Reset scroll when filter changes
                {
                    let mut vp = self.viewport.borrow_mut();
                    vp.scroll_y = 0.0;
                    vp.sync_scrollbar_position();
                    vp.changed.store(true, Ordering::Relaxed);
                }
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::ToggleOutline => {
                self.show_outline = !self.show_outline;
                self.update_filtered_fonts();
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::ToggleBlock => {
                self.show_block = !self.show_block;
                self.update_filtered_fonts();
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::ToggleColor => {
                self.show_color = !self.show_color;
                self.update_filtered_fonts();
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::ToggleFiglet => {
                self.show_figlet = !self.show_figlet;
                self.update_filtered_fonts();
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::SelectFont(idx) => {
                if let Some(pos) = self.filtered_fonts.iter().position(|&i| i == *idx) {
                    self.set_cursor_and_selected(pos);
                } else {
                    self.selected_font = *idx as i32;
                }
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::Confirm(_) => Some(DialogAction::CloseWith(tdf_msg(TdfFontSelectorMessage::Confirm(self.selected_font)))),
            TdfFontSelectorMessage::Cancel => Some(DialogAction::Close),
            TdfFontSelectorMessage::Export => {
                self.export_selected_font();
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::KeyUp => {
                if !self.filtered_fonts.is_empty() && self.keyboard_cursor > 0 {
                    self.set_cursor_and_selected(self.keyboard_cursor - 1);
                }
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::KeyDown => {
                if !self.filtered_fonts.is_empty() && self.keyboard_cursor < self.filtered_fonts.len() - 1 {
                    self.set_cursor_and_selected(self.keyboard_cursor + 1);
                }
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::KeyHome => {
                if !self.filtered_fonts.is_empty() {
                    self.set_cursor_and_selected_smooth(0);
                }
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::KeyEnd => {
                if !self.filtered_fonts.is_empty() {
                    self.set_cursor_and_selected_smooth(self.filtered_fonts.len() - 1);
                }
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::KeyPageUp => {
                if !self.filtered_fonts.is_empty() {
                    let page_items = (LIST_HEIGHT / FONT_ITEM_HEIGHT) as usize;
                    self.set_cursor_and_selected_smooth(self.keyboard_cursor.saturating_sub(page_items));
                }
                Some(DialogAction::None)
            }
            TdfFontSelectorMessage::KeyPageDown => {
                if !self.filtered_fonts.is_empty() {
                    let page_items = (LIST_HEIGHT / FONT_ITEM_HEIGHT) as usize;
                    self.set_cursor_and_selected_smooth((self.keyboard_cursor + page_items).min(self.filtered_fonts.len() - 1));
                }
                Some(DialogAction::None)
            }
        }
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        DialogAction::CloseWith(tdf_msg(TdfFontSelectorMessage::Confirm(self.selected_font)))
    }

    fn handle_event(&mut self, event: &iced::Event) -> Option<DialogAction<Message>> {
        use iced::keyboard::key::Named;
        use iced::keyboard::Key;

        if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
            let msg = match key {
                Key::Named(Named::ArrowUp) => Some(TdfFontSelectorMessage::KeyUp),
                Key::Named(Named::ArrowDown) => Some(TdfFontSelectorMessage::KeyDown),
                Key::Named(Named::Home) => Some(TdfFontSelectorMessage::KeyHome),
                Key::Named(Named::End) => Some(TdfFontSelectorMessage::KeyEnd),
                Key::Named(Named::PageUp) => Some(TdfFontSelectorMessage::KeyPageUp),
                Key::Named(Named::PageDown) => Some(TdfFontSelectorMessage::KeyPageDown),
                _ => None,
            };
            if let Some(m) = msg {
                return Some(DialogAction::SendMessage(tdf_msg(m)));
            }
        }
        None
    }
}

// ============================================================================
// Filter Toggle Button
// ============================================================================

fn filter_toggle_button(label: String, is_active: bool, on_press: Message) -> Element<'static, Message> {
    let style = if is_active { button::primary } else { button::secondary };

    button(text(label).size(TEXT_SIZE_SMALL)).padding([4, 8]).style(style).on_press(on_press).into()
}
