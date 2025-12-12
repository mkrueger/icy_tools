//! Font Selector Dialog
//!
//! A unified font selector that adapts to the current FormatMode.
//! Uses Canvas-based rendering for smooth scrolling with font preview images.

use std::cell::RefCell;
use std::collections::HashMap;

use iced::{
    Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    mouse,
    widget::{
        Space, button,
        canvas::{self, Cache, Frame, Geometry, Image, Path},
        column, container, image, row, stack, text, text_input,
    },
};
use icy_engine::{AttributedChar, BitFont, FontMode, RenderOptions, SAUCE_FONT_NAMES, TextAttribute, TextBuffer};
use icy_engine_edit::FormatMode;
use icy_engine_gui::ButtonType;
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_LARGE, Dialog, DialogAction, TEXT_SIZE_SMALL, dialog_area, dialog_title, modal_container, primary_button,
    secondary_button, separator,
};
use icy_engine_gui::{ScrollbarOverlay, Viewport};

use crate::fl;
use crate::ui::Message;

// ============================================================================
// Constants
// ============================================================================

/// Preview grid: 64 chars wide, 4 chars tall = 256 chars
const PREVIEW_CHARS_WIDTH: i32 = 64;
const PREVIEW_CHARS_HEIGHT: i32 = 4;
/// List viewport height in pixels
const LIST_HEIGHT: f32 = 400.0;
/// Row height including padding
const FONT_ROW_HEIGHT: f32 = 82.0;
/// Preview height within a row
const PREVIEW_HEIGHT: f32 = 52.0;

// ============================================================================
// Cached Image Handle
// ============================================================================

/// Cached image handle for a font preview
#[derive(Clone)]
struct CachedPreview {
    handle: image::Handle,
    width: u32,
    height: u32,
}

// ============================================================================
// Font Source - where a font comes from
// ============================================================================

/// Describes the source of a font
#[derive(Debug, Clone, Default)]
pub struct FontSource {
    /// ANSI font slot (0-42)
    pub ansi_slot: Option<usize>,
    /// SAUCE font name
    pub sauce_name: Option<String>,
    /// Slot in current document
    pub document_slot: Option<usize>,
    /// From font library
    pub is_library: bool,
}

// ============================================================================
// Font Entry - a font with its source info
// ============================================================================

#[derive(Debug, Clone)]
pub struct FontEntry {
    pub font: BitFont,
    pub source: FontSource,
}

// ============================================================================
// Dialog Messages
// ============================================================================

/// Messages for the Font Selector dialog
#[derive(Debug, Clone)]
pub enum FontSelectorMessage {
    /// Filter text changed
    SetFilter(String),
    /// Clear filter
    ClearFilter,

    /// Toggle SAUCE fonts visibility
    ToggleSauce,
    /// Toggle ANSI fonts visibility
    ToggleAnsi,
    /// Toggle library fonts visibility
    ToggleLibrary,
    /// Toggle document fonts visibility
    ToggleDocument,

    /// Select a font by index
    SelectFont(usize),

    /// For XBin Extended: select which slot to edit
    SelectSlot(usize),

    /// Apply selection
    Apply,
    /// Cancel dialog
    Cancel,
}

// ============================================================================
// Dialog Result
// ============================================================================

/// Result of the Font Selector dialog
#[derive(Debug, Clone)]
pub enum FontSelectorResult {
    /// Single font selected (for LegacyDos/XBin/Single mode)
    SingleFont(BitFont),
    /// Font selected for a specific slot (for XBin Extended / Unrestricted)
    FontForSlot { slot: usize, font: BitFont },
}

// ============================================================================
// Dialog State
// ============================================================================

/// State for the Font Selector dialog
pub struct FontSelectorDialog {
    /// Current format mode - determines dialog behavior
    format_mode: FormatMode,

    /// All available fonts
    fonts: Vec<FontEntry>,

    /// Currently selected font index
    selected_index: usize,

    /// Filter string
    filter: String,

    /// Show SAUCE fonts
    show_sauce: bool,
    /// Show ANSI fonts
    show_ansi: bool,
    /// Show library fonts
    show_library: bool,
    /// Show document fonts
    show_document: bool,

    /// For XBin Extended: which slot is being edited (0 or 1)
    active_slot: usize,

    /// For XBin Extended: current fonts in slots
    slot_fonts: [Option<BitFont>; 2],

    /// Only show SAUCE-compatible fonts
    only_sauce_fonts: bool,

    /// Cached filtered indices
    filtered_indices: Vec<usize>,

    /// Cached image handles for font previews (RGBA data)
    image_cache: RefCell<HashMap<usize, CachedPreview>>,

    /// Viewport for scroll management
    viewport: RefCell<Viewport>,

    /// Canvas cache for rendering
    canvas_cache: Cache,

    /// Last scroll position (for cache invalidation)
    last_scroll_y: RefCell<f32>,

    /// Last known cursor position (for drag handling)
    last_cursor_pos: Option<Point>,
}

impl FontSelectorDialog {
    /// Create a new Font Selector dialog
    pub fn new(state: &icy_engine_edit::EditState) -> Self {
        let buffer = state.get_buffer();
        let format_mode = state.get_format_mode();
        let only_sauce_fonts = matches!(buffer.font_mode, FontMode::Sauce);

        // Collect all fonts
        let mut fonts = Vec::new();
        let mut font_key_map: HashMap<String, usize> = HashMap::new();

        let font_key = |font: &BitFont| -> String {
            format!("{}:{}x{}", font.name(), font.size().width, font.size().height)
        };

        // Add SAUCE fonts
        for sauce_name in SAUCE_FONT_NAMES {
            if let Ok(font) = BitFont::from_sauce_name(sauce_name) {
                let key = font_key(&font);
                let idx = fonts.len();
                font_key_map.insert(key, idx);
                fonts.push(FontEntry {
                    font,
                    source: FontSource {
                        sauce_name: Some(sauce_name.to_string()),
                        ..Default::default()
                    },
                });
            }
        }

        // Add ANSI fonts (if not in SAUCE-only mode)
        if !only_sauce_fonts {
            for slot in 0..icy_engine::ANSI_FONTS {
                if let Ok(ansi_font) = BitFont::from_ansi_font_page(slot) {
                    let key = font_key(&ansi_font);
                    if let Some(&existing_idx) = font_key_map.get(&key) {
                        fonts[existing_idx].source.ansi_slot = Some(slot);
                    } else {
                        let idx = fonts.len();
                        font_key_map.insert(key, idx);
                        fonts.push(FontEntry {
                            font: ansi_font,
                            source: FontSource {
                                ansi_slot: Some(slot),
                                ..Default::default()
                            },
                        });
                    }
                }
            }
        }

        // Add fonts from current document
        let current_font_page = state.get_caret().font_page();
        let mut selected_index = 0;

        for (slot, doc_font) in buffer.font_iter() {
            let key = font_key(doc_font);
            if let Some(&existing_idx) = font_key_map.get(&key) {
                fonts[existing_idx].source.document_slot = Some(*slot);
                if *slot == current_font_page {
                    selected_index = existing_idx;
                }
            } else {
                let idx = fonts.len();
                if *slot == current_font_page {
                    selected_index = idx;
                }
                font_key_map.insert(key, idx);
                fonts.push(FontEntry {
                    font: doc_font.clone(),
                    source: FontSource {
                        document_slot: Some(*slot),
                        ..Default::default()
                    },
                });
            }
        }

        let slot_fonts = if format_mode == FormatMode::XBinExtended {
            [buffer.font(0).cloned(), buffer.font(1).cloned()]
        } else {
            [None, None]
        };

        let mut dialog = Self {
            format_mode,
            fonts,
            selected_index,
            filter: String::new(),
            show_sauce: true,
            show_ansi: !only_sauce_fonts,
            show_library: !only_sauce_fonts,
            show_document: true,
            active_slot: 0,
            slot_fonts,
            only_sauce_fonts,
            filtered_indices: Vec::new(),
            image_cache: RefCell::new(HashMap::new()),
            viewport: RefCell::new(Viewport::default()),
            canvas_cache: Cache::new(),
            last_scroll_y: RefCell::new(0.0),
            last_cursor_pos: None,
        };

        dialog.update_filtered_indices();
        dialog
    }

    /// Total content height
    fn content_height(&self) -> f32 {
        self.filtered_indices.len() as f32 * FONT_ROW_HEIGHT
    }

    /// Update viewport content size
    fn update_viewport_size(&self) {
        let mut vp = self.viewport.borrow_mut();
        vp.set_visible_size(DIALOG_WIDTH_LARGE as f32 - 40.0, LIST_HEIGHT);
        vp.set_content_size(DIALOG_WIDTH_LARGE as f32 - 40.0, self.content_height());
    }


    /// Generate preview image for a font
    fn generate_preview(&self, font_idx: usize) -> Option<CachedPreview> {
        let entry = self.fonts.get(font_idx)?;
        let font = &entry.font;

        // Create a temporary buffer with this font
        let mut buffer = TextBuffer::new((PREVIEW_CHARS_WIDTH, PREVIEW_CHARS_HEIGHT));
        buffer.set_font(0, font.clone());

        // Fill with all 256 characters
        for ch_code in 0..256u32 {
            let x = (ch_code % 64) as i32;
            let y = (ch_code / 64) as i32;
            let ch = unsafe { char::from_u32_unchecked(ch_code) };
            buffer.layers[0].set_char(
                (x, y),
                AttributedChar::new(ch, TextAttribute::default()),
            );
        }

        // Render to RGBA
        let options = RenderOptions::default();
        let region = icy_engine::Rectangle::from(
            0, 0,
            PREVIEW_CHARS_WIDTH * font.size().width,
            PREVIEW_CHARS_HEIGHT * font.size().height,
        );
        let (size, rgba) = buffer.render_region_to_rgba(region, &options, false);

        if size.width <= 0 || size.height <= 0 || rgba.is_empty() {
            return None;
        }

        let handle = image::Handle::from_rgba(size.width as u32, size.height as u32, rgba);
        Some(CachedPreview {
            handle,
            width: size.width as u32,
            height: size.height as u32,
        })
    }

    /// Get or generate preview for a font
    fn get_preview(&self, font_idx: usize) -> Option<CachedPreview> {
        if let Some(preview) = self.image_cache.borrow().get(&font_idx) {
            return Some(preview.clone());
        }

        if let Some(preview) = self.generate_preview(font_idx) {
            self.image_cache.borrow_mut().insert(font_idx, preview.clone());
            return Some(preview);
        }

        None
    }

    /// Update filtered indices
    fn update_filtered_indices(&mut self) {
        self.filtered_indices.clear();
        let filter_lower = self.filter.to_lowercase();

        for (idx, entry) in self.fonts.iter().enumerate() {
            let visible = (self.show_sauce && entry.source.sauce_name.is_some())
                || (self.show_ansi && entry.source.ansi_slot.is_some())
                || (self.show_library && entry.source.is_library)
                || (self.show_document && entry.source.document_slot.is_some());

            if !visible {
                continue;
            }

            if !filter_lower.is_empty() && !entry.font.name().to_lowercase().contains(&filter_lower) {
                continue;
            }

            self.filtered_indices.push(idx);
        }

        // Reset scroll and update viewport
        {
            let mut vp = self.viewport.borrow_mut();
            vp.scroll_y_to(0.0);
        }
        self.update_viewport_size();

        if !self.filtered_indices.is_empty() && !self.filtered_indices.contains(&self.selected_index) {
            self.selected_index = self.filtered_indices[0];
        }
    }

    /// Scroll to ensure selected item is visible
    fn scroll_to_selected(&self) {
        if let Some(pos) = self.filtered_indices.iter().position(|&i| i == self.selected_index) {
            let item_top = pos as f32 * FONT_ROW_HEIGHT;
            let item_bottom = item_top + FONT_ROW_HEIGHT;
            
            let mut vp = self.viewport.borrow_mut();
            let view_top = vp.scroll_y;
            let view_bottom = view_top + LIST_HEIGHT;
            
            if item_top < view_top {
                // Item is above viewport, scroll up
                vp.scroll_y_to_smooth(item_top);
            } else if item_bottom > view_bottom {
                // Item is below viewport, scroll down
                vp.scroll_y_to_smooth(item_bottom - LIST_HEIGHT);
            }
            vp.scrollbar.mark_interaction(true);
        }
    }

    /// Get currently selected font entry
    fn selected_font(&self) -> Option<&FontEntry> {
        self.fonts.get(self.selected_index)
    }

    /// Get dialog title
    fn title(&self) -> String {
        let count = self.filtered_indices.len();
        match self.format_mode {
            FormatMode::LegacyDos | FormatMode::XBin => format!("{} ({} {})", fl!("font-selector-title-single"), count, fl!("font-selector-available")),
            FormatMode::XBinExtended => format!("{} ({} {})", fl!("font-selector-title-dual"), count, fl!("font-selector-available")),
            FormatMode::Unrestricted => format!("{} ({} {})", fl!("font-selector-title-full"), count, fl!("font-selector-available")),
        }
    }



    /// Main view
    fn view_single_mode(&self) -> Element<'_, Message> {
        let title = dialog_title(self.title());

        let filter_placeholder = fl!("font-selector-filter-placeholder");
        let sauce_label = fl!("font-selector-sauce");
        let ansi_label = fl!("font-selector-ansi");
        let library_label = fl!("font-selector-library");
        let document_label = fl!("font-selector-document");

        // Filter row
        let filter_input = text_input(&filter_placeholder, &self.filter)
            .on_input(|s| Message::FontSelector(FontSelectorMessage::SetFilter(s)))
            .width(Length::Fixed(200.0));

        let clear_button = secondary_button("✕".to_string(), Some(Message::FontSelector(FontSelectorMessage::ClearFilter)));

        let mut filter_row = row![filter_input, clear_button].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        if !self.only_sauce_fonts {
            filter_row = filter_row.push(Space::new().width(Length::Fixed(20.0)));
            filter_row = filter_row.push(toggle_button(sauce_label, self.show_sauce, FontSelectorMessage::ToggleSauce));
            filter_row = filter_row.push(toggle_button(ansi_label, self.show_ansi, FontSelectorMessage::ToggleAnsi));
            filter_row = filter_row.push(toggle_button(library_label, self.show_library, FontSelectorMessage::ToggleLibrary));
            filter_row = filter_row.push(toggle_button(document_label, self.show_document, FontSelectorMessage::ToggleDocument));
        }

        // Virtualized font list
        let font_list = self.view_font_list();

        let content = column![filter_row, Space::new().height(DIALOG_SPACING), font_list].spacing(DIALOG_SPACING);

        let content_box = effect_box(content.into());

        // Buttons
        let button_row = row![
            Space::new().width(Length::Fill),
            secondary_button(format!("{}", ButtonType::Cancel), Some(Message::FontSelector(FontSelectorMessage::Cancel))),
            primary_button(format!("{}", ButtonType::Ok), self.selected_font().map(|_| Message::FontSelector(FontSelectorMessage::Apply))),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());
        let button_area = dialog_area(button_row.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_LARGE,
        )
        .into()
    }

    /// View font list with Canvas-based smooth scrolling
    fn view_font_list(&self) -> Element<'_, Message> {
        // Ensure viewport size is up to date
        self.update_viewport_size();

        // Check if scroll position changed - if so, we need to redraw
        let current_scroll = self.viewport.borrow().scroll_y;
        let last_scroll = *self.last_scroll_y.borrow();
        if (current_scroll - last_scroll).abs() > 0.1 {
            *self.last_scroll_y.borrow_mut() = current_scroll;
            self.canvas_cache.clear();
        }

        // Create canvas widget
        let canvas_widget = canvas::Canvas::new(FontListCanvas { dialog: self })
            .width(Length::Fill)
            .height(Length::Fixed(LIST_HEIGHT));

        // Use ScrollbarOverlay from icy_engine_gui
        if self.content_height() > LIST_HEIGHT {
            let scrollbar_view: Element<'_, ()> = ScrollbarOverlay::new(&self.viewport).view();
            let scrollbar_mapped: Element<'_, Message> = scrollbar_view.map(|_| unreachable!());
            let scrollbar_container = container(scrollbar_mapped)
                .width(Length::Fill)
                .height(Length::Fixed(LIST_HEIGHT))
                .align_x(Alignment::End);
            
            stack![canvas_widget, scrollbar_container].into()
        } else {
            canvas_widget.into()
        }
    }

    /// Get the filtered font indices
    fn get_filtered_indices(&self) -> &[usize] {
        &self.filtered_indices
    }

    /// Create result
    fn create_result(&self) -> Option<FontSelectorResult> {
        let entry = self.selected_font()?;

        match self.format_mode {
            FormatMode::LegacyDos | FormatMode::XBin => Some(FontSelectorResult::SingleFont(entry.font.clone())),
            FormatMode::XBinExtended => Some(FontSelectorResult::FontForSlot {
                slot: self.active_slot,
                font: entry.font.clone(),
            }),
            FormatMode::Unrestricted => {
                let slot = entry.source.document_slot.unwrap_or(0);
                Some(FontSelectorResult::FontForSlot { slot, font: entry.font.clone() })
            }
        }
    }
}

// ============================================================================
// Canvas Program Implementation for smooth scrolling
// ============================================================================

struct FontListCanvas<'a> {
    dialog: &'a FontSelectorDialog,
}

impl<'a> canvas::Program<Message> for FontListCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let geometry = self.dialog.canvas_cache.draw(renderer, bounds.size(), |frame| {
            let palette = theme.extended_palette();
            let scroll_y = self.dialog.viewport.borrow().scroll_y;
            let visible_fonts = self.dialog.get_filtered_indices();
            
            // Calculate which rows are visible
            let first_visible = (scroll_y / FONT_ROW_HEIGHT).floor() as usize;
            let last_visible = ((scroll_y + LIST_HEIGHT) / FONT_ROW_HEIGHT).ceil() as usize;
            
            // Draw each visible row
            for idx in first_visible..=last_visible.min(visible_fonts.len().saturating_sub(1)) {
                if idx >= visible_fonts.len() {
                    break;
                }
                
                let font_idx = visible_fonts[idx];
                let entry = &self.dialog.fonts[font_idx];
                let is_selected = self.dialog.selected_index == font_idx;
                
                // Calculate row position (relative to scroll)
                let row_y = idx as f32 * FONT_ROW_HEIGHT - scroll_y;
                
                // Skip if completely outside visible area
                if row_y + FONT_ROW_HEIGHT < 0.0 || row_y > LIST_HEIGHT {
                    continue;
                }
                
                // Draw row background
                let bg_color = if is_selected {
                    palette.primary.weak.color
                } else {
                    Color::TRANSPARENT
                };
                
                let path = Path::rectangle(Point::new(0.0, row_y), Size::new(bounds.width, FONT_ROW_HEIGHT));
                frame.fill(&path, bg_color);
                
                // Draw font name
                let text_color = if is_selected {
                    palette.primary.weak.text
                } else {
                    palette.background.base.text
                };
                
                frame.fill_text(canvas::Text {
                    content: entry.font.name().to_string(),
                    position: Point::new(8.0, row_y + 8.0),
                    color: text_color,
                    size: iced::Pixels(14.0),
                    ..Default::default()
                });
                
                // Draw source badges
                let mut badge_x = bounds.width - 8.0;
                
                if entry.source.is_library {
                    badge_x -= 60.0;
                    draw_badge(frame, "LIBRARY", badge_x, row_y + 6.0, palette.background.weak.color, palette.background.weak.text);
                }
                if entry.source.ansi_slot.is_some() {
                    badge_x -= 45.0;
                    draw_badge(frame, "ANSI", badge_x, row_y + 6.0, palette.background.weak.color, palette.background.weak.text);
                }
                if entry.source.sauce_name.is_some() {
                    badge_x -= 50.0;
                    draw_badge(frame, "SAUCE", badge_x, row_y + 6.0, palette.secondary.base.color, palette.secondary.base.text);
                }
                if entry.source.document_slot.is_some() {
                    badge_x -= 40.0;
                    draw_badge(frame, "FILE", badge_x, row_y + 6.0, palette.background.weak.color, palette.background.weak.text);
                }
                
                // Draw preview image - get or generate from cache
                if let Some(cached) = self.dialog.get_preview(font_idx) {
                    // Scale to fit
                    let scale = (PREVIEW_HEIGHT / cached.height as f32).min(1.0);
                    let display_width = cached.width as f32 * scale;
                    let display_height = cached.height as f32 * scale;
                    
                    // Draw the preview image using Canvas Image
                    let preview_image = Image::new(cached.handle.clone())
                        .snap(true);
                    
                    frame.draw_image(
                        Rectangle::new(
                            Point::new(8.0, row_y + 26.0),
                            Size::new(display_width, display_height),
                        ),
                        preview_image,
                    );
                } else {
                    // Draw placeholder for preview
                    let placeholder_rect = Path::rectangle(
                        Point::new(8.0, row_y + 26.0),
                        Size::new(200.0, PREVIEW_HEIGHT),
                    );
                    frame.fill(&placeholder_rect, palette.background.weak.color);
                }
                
                // Draw separator line at bottom of row
                let separator_path = Path::line(
                    Point::new(0.0, row_y + FONT_ROW_HEIGHT - 1.0),
                    Point::new(bounds.width, row_y + FONT_ROW_HEIGHT - 1.0),
                );
                frame.stroke(&separator_path, canvas::Stroke::default().with_color(palette.background.weak.color).with_width(1.0));
            }
        });

        vec![geometry]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_delta = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * 40.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };
                self.dialog.viewport.borrow_mut().scroll_y_by(scroll_delta);
                self.dialog.canvas_cache.clear();
                Some(canvas::Action::request_redraw().and_capture())
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let scroll_y = self.dialog.viewport.borrow().scroll_y;
                    let clicked_row = ((pos.y + scroll_y) / FONT_ROW_HEIGHT).floor() as usize;
                    
                    let visible_fonts = self.dialog.get_filtered_indices();
                    if clicked_row < visible_fonts.len() {
                        let font_idx = visible_fonts[clicked_row];
                        return Some(canvas::Action::publish(
                            Message::FontSelector(FontSelectorMessage::SelectFont(font_idx))
                        ).and_capture());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Draw a badge on the canvas
fn draw_badge(frame: &mut Frame, label: &str, x: f32, y: f32, bg_color: Color, text_color: Color) {
    let badge_width = label.len() as f32 * 7.0 + 12.0;
    let badge_height = 16.0;
    
    // Draw background with rounded corners (approximated with rectangle for now)
    let badge_path = Path::rectangle(Point::new(x, y), Size::new(badge_width, badge_height));
    frame.fill(&badge_path, bg_color);
    
    // Draw text
    frame.fill_text(canvas::Text {
        content: label.to_string(),
        position: Point::new(x + 6.0, y + 2.0),
        color: text_color,
        size: iced::Pixels(10.0),
        ..Default::default()
    });
}

// ============================================================================
// Helper functions
// ============================================================================

fn toggle_button(label: String, active: bool, msg: FontSelectorMessage) -> Element<'static, Message> {
    let style = if active { active_toggle_style } else { inactive_toggle_style };

    button(text(label).size(TEXT_SIZE_SMALL))
        .on_press(Message::FontSelector(msg))
        .style(style)
        .padding([4, 8])
        .into()
}

fn active_toggle_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(iced::Background::Color(palette.primary.base.color)),
        text_color: palette.primary.base.text,
        border: iced::Border { radius: 4.0.into(), ..Default::default() },
        ..Default::default()
    }
}

fn inactive_toggle_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Hovered => palette.background.weak.color,
        _ => palette.background.strong.color,
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color: palette.background.base.text,
        border: iced::Border { radius: 4.0.into(), width: 1.0, color: palette.background.strong.color },
        ..Default::default()
    }
}

// ============================================================================
// Dialog Implementation
// ============================================================================

impl Dialog<Message> for FontSelectorDialog {
    fn view(&self) -> Element<'_, Message> {
        self.view_single_mode()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::FontSelector(msg) = message else { return None; };

        match msg {
            FontSelectorMessage::SetFilter(f) => {
                self.filter = f.clone();
                self.update_filtered_indices();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ClearFilter => {
                self.filter.clear();
                self.update_filtered_indices();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ToggleSauce => {
                self.show_sauce = !self.show_sauce;
                self.update_filtered_indices();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ToggleAnsi => {
                self.show_ansi = !self.show_ansi;
                self.update_filtered_indices();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ToggleLibrary => {
                self.show_library = !self.show_library;
                self.update_filtered_indices();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ToggleDocument => {
                self.show_document = !self.show_document;
                self.update_filtered_indices();
                Some(DialogAction::None)
            }
            FontSelectorMessage::SelectFont(idx) => {
                self.selected_index = *idx;
                self.canvas_cache.clear(); // Invalidate canvas to show new selection
                Some(DialogAction::None)
            }
            FontSelectorMessage::SelectSlot(slot) => {
                self.active_slot = *slot;
                Some(DialogAction::None)
            }
            FontSelectorMessage::Apply => {
                if let Some(result) = self.create_result() {
                    Some(DialogAction::CloseWith(Message::ApplyFontSelection(result)))
                } else {
                    Some(DialogAction::None)
                }
            }
            FontSelectorMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn handle_event(&mut self, event: &iced::Event) -> Option<DialogAction<Message>> {
        match event {
            // Mouse wheel scrolling
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * FONT_ROW_HEIGHT,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };
                
                let mut vp = self.viewport.borrow_mut();
                vp.scroll_y_by(scroll_amount);
                vp.scrollbar.mark_interaction(true);
                drop(vp);
                self.canvas_cache.clear();
                return Some(DialogAction::None);
            }
            
            // Keyboard navigation
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                use iced::keyboard::Key;
                use iced::keyboard::key::Named;
                
                match key {
                    Key::Named(Named::ArrowUp) => {
                        // Select previous font
                        if let Some(pos) = self.filtered_indices.iter().position(|&i| i == self.selected_index) {
                            if pos > 0 {
                                self.selected_index = self.filtered_indices[pos - 1];
                                self.scroll_to_selected();
                                self.canvas_cache.clear();
                                return Some(DialogAction::None);
                            }
                        }
                    }
                    Key::Named(Named::ArrowDown) => {
                        // Select next font
                        if let Some(pos) = self.filtered_indices.iter().position(|&i| i == self.selected_index) {
                            if pos + 1 < self.filtered_indices.len() {
                                self.selected_index = self.filtered_indices[pos + 1];
                                self.scroll_to_selected();
                                self.canvas_cache.clear();
                                return Some(DialogAction::None);
                            }
                        }
                    }
                    Key::Named(Named::PageUp) => {
                        let mut vp = self.viewport.borrow_mut();
                        vp.scroll_y_by_smooth(-LIST_HEIGHT);
                        vp.scrollbar.mark_interaction(true);
                        drop(vp);
                        self.canvas_cache.clear();
                        return Some(DialogAction::None);
                    }
                    Key::Named(Named::PageDown) => {
                        let mut vp = self.viewport.borrow_mut();
                        vp.scroll_y_by_smooth(LIST_HEIGHT);
                        vp.scrollbar.mark_interaction(true);
                        drop(vp);
                        self.canvas_cache.clear();
                        return Some(DialogAction::None);
                    }
                    Key::Named(Named::Home) => {
                        if !self.filtered_indices.is_empty() {
                            self.selected_index = self.filtered_indices[0];
                            let mut vp = self.viewport.borrow_mut();
                            vp.scroll_y_to_smooth(0.0);
                            vp.scrollbar.mark_interaction(true);
                            drop(vp);
                            self.canvas_cache.clear();
                            return Some(DialogAction::None);
                        }
                    }
                    Key::Named(Named::End) => {
                        if !self.filtered_indices.is_empty() {
                            self.selected_index = *self.filtered_indices.last().unwrap();
                            let mut vp = self.viewport.borrow_mut();
                            let max = vp.max_scroll_y();
                            vp.scroll_y_to_smooth(max);
                            vp.scrollbar.mark_interaction(true);
                            drop(vp);
                            self.canvas_cache.clear();
                            return Some(DialogAction::None);
                        }
                    }
                    _ => {}
                }
            }
            
            _ => {}
        }
        None
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if let Some(result) = self.create_result() {
            DialogAction::CloseWith(Message::ApplyFontSelection(result))
        } else {
            DialogAction::None
        }
    }

    fn needs_animation(&self) -> bool {
        self.viewport.borrow().needs_animation()
    }

    fn update_animation(&mut self) {
        self.viewport.borrow_mut().update_animation();
    }
}
