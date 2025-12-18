//! Font Selector Dialog - Split-View Design
//!
//! A unified font selector with:
//! - Left panel: Search + grouped font list (Canvas-based with overlay scrollbar)
//! - Right panel: Large preview + font details

use std::cell::RefCell;
use std::collections::HashMap;

use iced::{
    Alignment, Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme,
    keyboard::{Key, key::Named},
    mouse,
    widget::{
        Space,
        canvas::{self, Canvas, Frame, Geometry, Path, Text},
        column, container, image, row, text, text_input,
    },
};
use icy_engine::{AttributedChar, BitFont, FontMode, RenderOptions, TextAttribute, TextBuffer, get_sauce_font_names};
use icy_engine_edit::FormatMode;
use icy_engine_gui::ui::{DIALOG_SPACING, Dialog, DialogAction, dialog_area, modal_container, primary_button, secondary_button, separator};
use icy_engine_gui::{ButtonType, ScrollbarOverlay, Viewport, focus};

use super::super::{AnsiEditorCoreMessage, AnsiEditorMessage};
use crate::fl;
use crate::ui::Message;

/// Helper to wrap FontSelectorMessage in Message
fn msg(m: FontSelectorMessage) -> Message {
    Message::AnsiEditor(AnsiEditorMessage::FontSelector(m))
}

// ============================================================================
// Constants
// ============================================================================

/// Dialog dimensions
const DIALOG_WIDTH: f32 = 800.0;
const DIALOG_HEIGHT: f32 = 520.0;

/// Left panel (font list) width
const LEFT_PANEL_WIDTH: f32 = 280.0;

/// Preview dimensions - 16x16 grid for all 256 characters
const PREVIEW_CHARS_WIDTH: i32 = 16;
const PREVIEW_CHARS_HEIGHT: i32 = 16;

/// Fixed preview scale factor
const PREVIEW_SCALE: f32 = 2.0;

/// Font list item heights
const CATEGORY_HEADER_HEIGHT: f32 = 28.0;
const FONT_ITEM_HEIGHT: f32 = 24.0;
const FONT_ITEM_INDENT: f32 = 24.0;

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
// Font Category
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontCategory {
    Sauce,
    Ansi,
    Library,
}

impl FontCategory {
    fn label(&self) -> &'static str {
        match self {
            FontCategory::Sauce => "SAUCE Fonts",
            FontCategory::Ansi => "ANSI Fonts",
            FontCategory::Library => "Bibliothek",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            FontCategory::Sauce => "🏷",
            FontCategory::Ansi => "💻",
            FontCategory::Library => "📚",
        }
    }
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
}

impl FontSource {
    fn primary_category(&self) -> FontCategory {
        if self.sauce_name.is_some() {
            FontCategory::Sauce
        } else if self.ansi_slot.is_some() {
            FontCategory::Ansi
        } else {
            FontCategory::Library
        }
    }
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
// List Item - represents a visual row in the font list
// ============================================================================

#[derive(Debug, Clone)]
enum ListItem {
    CategoryHeader { category: FontCategory, count: usize },
    FontItem { font_idx: usize },
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

    /// Toggle category expand/collapse
    ToggleCategory(FontCategory),

    /// Select a font by index
    SelectFont(usize),

    /// Navigate up in the font list
    NavigateUp,
    /// Navigate down in the font list
    NavigateDown,
    /// Navigate to first item in font list
    NavigateHome,
    /// Navigate to last item in font list
    NavigateEnd,
    /// Navigate page up in font list
    NavigatePageUp,
    /// Navigate page down in font list
    NavigatePageDown,

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
    /// Single font selected
    SingleFont(BitFont),
    /// Font selected for a specific slot (for XBin Extended / Unrestricted)
    FontForSlot { slot: usize, font: BitFont },
}

// ============================================================================
// Dialog State
// ============================================================================

/// Category state for collapsible sections
struct CategoryState {
    expanded: bool,
    visible: bool,
    font_indices: Vec<usize>,
}

/// State for the Font Selector dialog
pub struct FontSelectorDialog {
    /// Current format mode - determines dialog behavior
    format_mode: FormatMode,

    /// All available fonts
    fonts: Vec<FontEntry>,

    /// Currently selected font index
    selected_index: usize,

    /// Current font (the one in use before opening dialog)
    current_font_size: (i32, i32),

    /// Filter string
    filter: String,

    /// Category states for collapsible sections
    categories: HashMap<FontCategory, CategoryState>,

    /// Which slot is being edited (passed in from caller for Unrestricted/XBinExtended)
    active_slot: usize,

    /// Cached preview for large display
    large_preview_cache: RefCell<Option<(usize, CachedPreview)>>,

    /// Viewport for font list scrolling
    list_viewport: RefCell<Viewport>,

    /// Cached list of visible items (rebuilt when filter/categories change)
    visible_items: RefCell<Vec<ListItem>>,
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

        let font_key = |font: &BitFont| -> String { format!("{}:{}x{}", font.name(), font.size().width, font.size().height) };

        // Add SAUCE fonts (only those actually available in SAUCE_FONT_MAP)
        for sauce_name in get_sauce_font_names() {
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
                if let Some(ansi_font) = BitFont::from_ansi_font_page(slot, 16) {
                    let key = font_key(&ansi_font);
                    if let Some(&existing_idx) = font_key_map.get(&key) {
                        fonts[existing_idx].source.ansi_slot = Some(slot);
                    } else {
                        let idx = fonts.len();
                        font_key_map.insert(key, idx);
                        fonts.push(FontEntry {
                            font: ansi_font.clone(),
                            source: FontSource {
                                ansi_slot: Some(slot),
                                ..Default::default()
                            },
                        });
                    }
                }
            }
        }

        // Track current font from document
        let current_font_page = state.get_caret().font_page();
        let mut selected_index = 0;
        let mut current_font_size = (8, 16);

        // Mark document fonts and find current font
        for (slot, doc_font) in buffer.font_iter() {
            let key = font_key(doc_font);
            if *slot == current_font_page {
                current_font_size = (doc_font.size().width, doc_font.size().height);
            }
            if let Some(&existing_idx) = font_key_map.get(&key) {
                fonts[existing_idx].source.document_slot = Some(*slot);
                if *slot == current_font_page {
                    selected_index = existing_idx;
                }
            } else {
                // Document-only fonts that don't match SAUCE/ANSI - add to library
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

        // Build category states
        let mut categories = HashMap::new();
        for cat in [FontCategory::Sauce, FontCategory::Ansi, FontCategory::Library] {
            let font_indices: Vec<usize> = fonts
                .iter()
                .enumerate()
                .filter(|(_, f)| f.source.primary_category() == cat)
                .map(|(i, _)| i)
                .collect();

            let visible = match cat {
                FontCategory::Sauce => true,
                FontCategory::Ansi => !only_sauce_fonts,
                FontCategory::Library => !only_sauce_fonts,
            };

            categories.insert(
                cat,
                CategoryState {
                    expanded: true,
                    visible: visible && !font_indices.is_empty(),
                    font_indices,
                },
            );
        }

        // Active slot is the current caret font page
        let active_slot = current_font_page;

        let dialog = Self {
            format_mode,
            fonts,
            selected_index,
            current_font_size,
            filter: String::new(),
            categories,
            active_slot,
            large_preview_cache: RefCell::new(None),
            list_viewport: RefCell::new(Viewport::default()),
            visible_items: RefCell::new(Vec::new()),
        };

        // Build initial visible items and scroll to selection
        dialog.rebuild_visible_items();
        dialog.scroll_to_selection();

        dialog
    }

    fn selected_font(&self) -> Option<&FontEntry> {
        self.fonts.get(self.selected_index)
    }

    /// Check if a font entry matches the current filter criteria
    fn matches_filter(&self, entry: &FontEntry) -> bool {
        // Filter by font height
        let font_height = entry.font.size().height;
        if font_height != self.current_font_size.1 {
            return false;
        }

        // Then apply text filter
        if self.filter.is_empty() {
            return true;
        }
        entry.font.name().to_lowercase().contains(&self.filter.to_lowercase())
    }

    /// Rebuild the list of visible items based on current filter and category states
    fn rebuild_visible_items(&self) {
        let mut items = Vec::new();

        for cat in [FontCategory::Sauce, FontCategory::Ansi, FontCategory::Library] {
            if let Some(state) = self.categories.get(&cat) {
                if !state.visible {
                    continue;
                }

                // Filter fonts
                let filtered_fonts: Vec<usize> = state
                    .font_indices
                    .iter()
                    .copied()
                    .filter(|&idx| self.matches_filter(&self.fonts[idx]))
                    .collect();

                if filtered_fonts.is_empty() {
                    continue;
                }

                // Add category header
                items.push(ListItem::CategoryHeader {
                    category: cat,
                    count: filtered_fonts.len(),
                });

                // Add font items if expanded
                if state.expanded {
                    for font_idx in filtered_fonts {
                        items.push(ListItem::FontItem { font_idx });
                    }
                }
            }
        }

        // Update content height in viewport
        let content_height = self.calculate_content_height(&items);
        self.list_viewport.borrow_mut().content_height = content_height;

        *self.visible_items.borrow_mut() = items;
    }

    /// Calculate total content height for the list
    fn calculate_content_height(&self, items: &[ListItem]) -> f32 {
        items
            .iter()
            .map(|item| match item {
                ListItem::CategoryHeader { .. } => CATEGORY_HEADER_HEIGHT,
                ListItem::FontItem { .. } => FONT_ITEM_HEIGHT,
            })
            .sum()
    }

    /// Get the Y position of the selected font in the list
    fn get_selection_y_position(&self) -> Option<f32> {
        let items = self.visible_items.borrow();
        let mut y = 0.0;

        for item in items.iter() {
            let height = match item {
                ListItem::CategoryHeader { .. } => CATEGORY_HEADER_HEIGHT,
                ListItem::FontItem { font_idx } => {
                    if *font_idx == self.selected_index {
                        return Some(y);
                    }
                    FONT_ITEM_HEIGHT
                }
            };
            y += height;
        }
        None
    }

    /// Scroll the list to make the selected item visible
    fn scroll_to_selection(&self) {
        if let Some(y) = self.get_selection_y_position() {
            let mut vp = self.list_viewport.borrow_mut();
            let visible_height = vp.visible_height;

            // Check if selection is above visible area
            if y < vp.scroll_y {
                vp.scroll_y = y;
                vp.target_scroll_y = y;
                vp.sync_scrollbar_position();
            }
            // Check if selection is below visible area
            else if y + FONT_ITEM_HEIGHT > vp.scroll_y + visible_height {
                let new_scroll = y + FONT_ITEM_HEIGHT - visible_height;
                vp.scroll_y = new_scroll;
                vp.target_scroll_y = new_scroll;
                vp.sync_scrollbar_position();
            }
        }
    }

    /// Ensure the currently selected font is visible in the list
    fn ensure_selection_visible(&mut self) {
        let visible_fonts = self.get_visible_fonts();

        if !visible_fonts.contains(&self.selected_index) {
            if let Some(&first_visible) = visible_fonts.first() {
                self.selected_index = first_visible;
            }
        }

        self.rebuild_visible_items();
        self.scroll_to_selection();
    }

    /// Generate preview showing all 256 characters in a 16x16 grid
    fn generate_large_preview(&self, font_idx: usize) -> Option<CachedPreview> {
        let entry = self.fonts.get(font_idx)?;
        let font = &entry.font;

        let mut buffer = TextBuffer::new((PREVIEW_CHARS_WIDTH, PREVIEW_CHARS_HEIGHT));
        buffer.set_font(0, font.clone());

        for ch_code in 0..256u32 {
            let x = (ch_code % 16) as i32;
            let y = (ch_code / 16) as i32;
            let ch = unsafe { char::from_u32_unchecked(ch_code) };
            buffer.layers[0].set_char((x, y), AttributedChar::new(ch, TextAttribute::default()));
        }

        let options = RenderOptions::default();
        let region = icy_engine::Rectangle::from(0, 0, PREVIEW_CHARS_WIDTH * font.size().width, PREVIEW_CHARS_HEIGHT * font.size().height);
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

    fn get_large_preview(&self, font_idx: usize) -> Option<CachedPreview> {
        if let Some((cached_idx, preview)) = self.large_preview_cache.borrow().as_ref() {
            if *cached_idx == font_idx {
                return Some(preview.clone());
            }
        }

        if let Some(preview) = self.generate_large_preview(font_idx) {
            *self.large_preview_cache.borrow_mut() = Some((font_idx, preview.clone()));
            return Some(preview);
        }

        None
    }

    fn create_result(&self) -> Option<FontSelectorResult> {
        let entry = self.selected_font()?;

        match self.format_mode {
            FormatMode::LegacyDos | FormatMode::XBin => Some(FontSelectorResult::SingleFont(entry.font.clone())),
            FormatMode::XBinExtended | FormatMode::Unrestricted => Some(FontSelectorResult::FontForSlot {
                slot: self.active_slot,
                font: entry.font.clone(),
            }),
        }
    }

    fn get_visible_fonts(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for cat in [FontCategory::Sauce, FontCategory::Ansi, FontCategory::Library] {
            if let Some(state) = self.categories.get(&cat) {
                if state.visible && state.expanded {
                    for &idx in &state.font_indices {
                        if self.matches_filter(&self.fonts[idx]) {
                            result.push(idx);
                        }
                    }
                }
            }
        }
        result
    }

    fn find_prev_font(&self) -> Option<usize> {
        let visible_fonts = self.get_visible_fonts();
        if visible_fonts.is_empty() {
            return None;
        }

        if let Some(current_pos) = visible_fonts.iter().position(|&i| i == self.selected_index) {
            if current_pos > 0 { Some(visible_fonts[current_pos - 1]) } else { None }
        } else {
            visible_fonts.last().copied()
        }
    }

    fn find_next_font(&self) -> Option<usize> {
        let visible_fonts = self.get_visible_fonts();
        if visible_fonts.is_empty() {
            return None;
        }

        if let Some(current_pos) = visible_fonts.iter().position(|&i| i == self.selected_index) {
            if current_pos + 1 < visible_fonts.len() {
                Some(visible_fonts[current_pos + 1])
            } else {
                None
            }
        } else {
            visible_fonts.first().copied()
        }
    }

    fn find_first_font(&self) -> Option<usize> {
        self.get_visible_fonts().first().copied()
    }

    fn find_last_font(&self) -> Option<usize> {
        self.get_visible_fonts().last().copied()
    }

    /// Move selection up by approximately one page
    fn page_up(&mut self) {
        let visible_fonts = self.get_visible_fonts();
        if visible_fonts.is_empty() {
            return;
        }

        let visible_height = self.list_viewport.borrow().visible_height;
        let items_per_page = (visible_height / FONT_ITEM_HEIGHT).max(1.0) as usize;

        if let Some(current_pos) = visible_fonts.iter().position(|&i| i == self.selected_index) {
            let new_pos = current_pos.saturating_sub(items_per_page);
            self.selected_index = visible_fonts[new_pos];
        } else if let Some(&first) = visible_fonts.first() {
            self.selected_index = first;
        }
        self.scroll_to_selection();
    }

    /// Move selection down by approximately one page
    fn page_down(&mut self) {
        let visible_fonts = self.get_visible_fonts();
        if visible_fonts.is_empty() {
            return;
        }

        let visible_height = self.list_viewport.borrow().visible_height;
        let items_per_page = (visible_height / FONT_ITEM_HEIGHT).max(1.0) as usize;

        if let Some(current_pos) = visible_fonts.iter().position(|&i| i == self.selected_index) {
            let new_pos = (current_pos + items_per_page).min(visible_fonts.len() - 1);
            self.selected_index = visible_fonts[new_pos];
        } else if let Some(&last) = visible_fonts.last() {
            self.selected_index = last;
        }
        self.scroll_to_selection();
    }

    // ========================================================================
    // View Methods
    // ========================================================================

    fn view_split_layout(&self) -> Element<'_, Message> {
        let left_panel = self.view_left_panel();
        let right_panel = self.view_right_panel();

        let content = row![
            container(left_panel)
                .width(Length::Fixed(LEFT_PANEL_WIDTH))
                .height(Length::Fixed(DIALOG_HEIGHT - 80.0)),
            container(right_panel).width(Length::Fill).height(Length::Fixed(DIALOG_HEIGHT - 80.0)),
        ]
        .spacing(0);

        let button_row = row![
            Space::new().width(Length::Fill),
            secondary_button(format!("{}", ButtonType::Cancel), Some(msg(FontSelectorMessage::Cancel))),
            primary_button(format!("{}", ButtonType::Ok), self.selected_font().map(|_| msg(FontSelectorMessage::Apply))),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let button_area = dialog_area(button_row.into());

        let dialog_content = dialog_area(content.into());
        let dialog_column = column![container(dialog_content).height(Length::Shrink), separator(), button_area];

        modal_container(dialog_column.into(), DIALOG_WIDTH).into()
    }

    fn view_left_panel(&self) -> Element<'_, Message> {
        let search_input = text_input(&fl!("font-selector-filter-placeholder"), &self.filter)
            .on_input(|s| msg(FontSelectorMessage::SetFilter(s)))
            .width(Length::Fill)
            .padding(8);

        // Canvas-based font list with overlay scrollbar
        let font_list_canvas: Element<'_, Message> = Canvas::new(FontListCanvas { dialog: self }).width(Length::Fill).height(Length::Fill).into();

        let scrollbar: Element<'_, Message> = ScrollbarOverlay::new(&self.list_viewport).view().map(|_| msg(FontSelectorMessage::Cancel)); // Dummy mapping, scrollbar handles viewport directly

        let list_row = row![font_list_canvas, scrollbar,];

        // Wrap in Focus widget for keyboard navigation
        let list_with_scrollbar: Element<'_, Message> = focus(list_row)
            .on_event(|event, _id| {
                if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                    match key {
                        Key::Named(Named::ArrowUp) => Some(msg(FontSelectorMessage::NavigateUp)),
                        Key::Named(Named::ArrowDown) => Some(msg(FontSelectorMessage::NavigateDown)),
                        Key::Named(Named::Home) => Some(msg(FontSelectorMessage::NavigateHome)),
                        Key::Named(Named::End) => Some(msg(FontSelectorMessage::NavigateEnd)),
                        Key::Named(Named::PageUp) => Some(msg(FontSelectorMessage::NavigatePageUp)),
                        Key::Named(Named::PageDown) => Some(msg(FontSelectorMessage::NavigatePageDown)),
                        Key::Named(Named::Enter) => Some(msg(FontSelectorMessage::Apply)),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .into();

        column![
            container(search_input).padding(8),
            container(list_with_scrollbar).width(Length::Fill).height(Length::Fill),
        ]
        .into()
    }

    fn view_right_panel(&self) -> Element<'_, Message> {
        let Some(entry) = self.selected_font() else {
            return container(text("Keine Schrift ausgewählt").size(14))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };

        let preview: Element<'_, Message> = if let Some(cached) = self.get_large_preview(self.selected_index) {
            let display_width = cached.width as f32 * PREVIEW_SCALE;
            let display_height = cached.height as f32 * PREVIEW_SCALE;

            image(cached.handle)
                .width(Length::Fixed(display_width))
                .height(Length::Fixed(display_height))
                .into()
        } else {
            text("...").size(12).into()
        };

        let preview_box = container(preview)
            .style(preview_container_style)
            .padding(8)
            .center_x(Length::Shrink)
            .center_y(Length::Shrink);

        let font_title = text(entry.font.name()).size(14);

        column![
            Space::new().height(8.0),
            font_title,
            Space::new().height(8.0),
            container(preview_box).center_x(Length::Fill).height(Length::Fill),
            Space::new().height(8.0),
        ]
        .padding([0, 8])
        .into()
    }
}

// ============================================================================
// Font List Canvas
// ============================================================================

struct FontListCanvas<'a> {
    dialog: &'a FontSelectorDialog,
}

impl<'a> canvas::Program<Message> for FontListCanvas<'a> {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let palette = theme.extended_palette();

        // Update viewport dimensions
        {
            let mut vp = self.dialog.list_viewport.borrow_mut();
            vp.visible_height = bounds.height;
            vp.visible_width = bounds.width;
        }

        let scroll_y = self.dialog.list_viewport.borrow().scroll_y;
        let items = self.dialog.visible_items.borrow();

        let geometry = iced::widget::canvas::Cache::new().draw(renderer, bounds.size(), |frame: &mut Frame| {
            let mut y = -scroll_y;

            for item in items.iter() {
                let height = match item {
                    ListItem::CategoryHeader { .. } => CATEGORY_HEADER_HEIGHT,
                    ListItem::FontItem { .. } => FONT_ITEM_HEIGHT,
                };

                // Skip items above visible area
                if y + height < 0.0 {
                    y += height;
                    continue;
                }

                // Stop drawing items below visible area
                if y > bounds.height {
                    break;
                }

                match item {
                    ListItem::CategoryHeader { category, count } => {
                        // Draw category header background
                        let bg_rect = Path::rectangle(Point::new(0.0, y), Size::new(bounds.width, height));
                        frame.fill(&bg_rect, palette.background.weak.color);

                        // Draw arrow and text
                        let expanded = self.dialog.categories.get(category).map(|s| s.expanded).unwrap_or(true);
                        let arrow = if expanded { "▼" } else { "▶" };
                        let label = format!("{} {} {} ({})", arrow, category.icon(), category.label(), count);

                        frame.fill_text(Text {
                            content: label,
                            position: Point::new(12.0, y + (height - 13.0) / 2.0),
                            color: palette.background.base.text,
                            size: iced::Pixels(13.0),
                            ..Default::default()
                        });
                    }
                    ListItem::FontItem { font_idx } => {
                        let is_selected = self.dialog.selected_index == *font_idx;

                        // Draw selection background
                        if is_selected {
                            let bg_rect = Path::rectangle(Point::new(0.0, y), Size::new(bounds.width, height));
                            frame.fill(&bg_rect, palette.primary.weak.color);
                        }

                        // Draw font name
                        let font_name = &self.dialog.fonts[*font_idx].font.name();
                        let text_color = if is_selected {
                            palette.primary.weak.text
                        } else {
                            palette.background.base.text
                        };

                        frame.fill_text(Text {
                            content: font_name.to_string(),
                            position: Point::new(FONT_ITEM_INDENT + 12.0, y + (height - 12.0) / 2.0),
                            color: text_color,
                            size: iced::Pixels(12.0),
                            ..Default::default()
                        });
                    }
                }

                y += height;
            }
        });

        vec![geometry]
    }

    fn update(&self, _state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    // Find which item was clicked
                    let scroll_y = self.dialog.list_viewport.borrow().scroll_y;
                    let click_y = pos.y + scroll_y;
                    let items = self.dialog.visible_items.borrow();
                    let mut current_y = 0.0;

                    for item in items.iter() {
                        let height = match item {
                            ListItem::CategoryHeader { .. } => CATEGORY_HEADER_HEIGHT,
                            ListItem::FontItem { .. } => FONT_ITEM_HEIGHT,
                        };

                        if click_y >= current_y && click_y < current_y + height {
                            match item {
                                ListItem::CategoryHeader { category, .. } => {
                                    return Some(canvas::Action::publish(msg(FontSelectorMessage::ToggleCategory(*category))));
                                }
                                ListItem::FontItem { font_idx } => {
                                    return Some(canvas::Action::publish(msg(FontSelectorMessage::SelectFont(*font_idx))));
                                }
                            }
                        }

                        current_y += height;
                    }
                }
            }
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    let scroll_amount = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => -y * 30.0,
                        mouse::ScrollDelta::Pixels { y, .. } => -y,
                    };

                    let mut vp = self.dialog.list_viewport.borrow_mut();
                    let max_scroll = (vp.content_height - vp.visible_height).max(0.0);
                    vp.scroll_y = (vp.scroll_y + scroll_amount).clamp(0.0, max_scroll);
                    vp.target_scroll_y = vp.scroll_y;

                    return Some(canvas::Action::request_redraw());
                }
            }
            _ => {}
        }

        None
    }
}

// ============================================================================
// Styles
// ============================================================================

fn preview_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgb(0.05, 0.05, 0.08))),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        ..Default::default()
    }
}

// ============================================================================
// Dialog Implementation
// ============================================================================

impl Dialog<Message> for FontSelectorDialog {
    fn view(&self) -> Element<'_, Message> {
        self.view_split_layout()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::AnsiEditor(AnsiEditorMessage::FontSelector(msg)) = message else {
            return None;
        };

        match msg {
            FontSelectorMessage::SetFilter(f) => {
                self.filter = f.clone();
                self.ensure_selection_visible();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ClearFilter => {
                self.filter.clear();
                self.rebuild_visible_items();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ToggleCategory(cat) => {
                if let Some(state) = self.categories.get_mut(cat) {
                    state.expanded = !state.expanded;
                }
                self.ensure_selection_visible();
                Some(DialogAction::None)
            }
            FontSelectorMessage::SelectFont(idx) => {
                self.selected_index = *idx;
                self.scroll_to_selection();
                Some(DialogAction::None)
            }
            FontSelectorMessage::NavigateUp => {
                if let Some(prev) = self.find_prev_font() {
                    self.selected_index = prev;
                    self.scroll_to_selection();
                }
                Some(DialogAction::None)
            }
            FontSelectorMessage::NavigateDown => {
                if let Some(next) = self.find_next_font() {
                    self.selected_index = next;
                    self.scroll_to_selection();
                }
                Some(DialogAction::None)
            }
            FontSelectorMessage::NavigateHome => {
                if let Some(first) = self.find_first_font() {
                    self.selected_index = first;
                    self.scroll_to_selection();
                }
                Some(DialogAction::None)
            }
            FontSelectorMessage::NavigateEnd => {
                if let Some(last) = self.find_last_font() {
                    self.selected_index = last;
                    self.scroll_to_selection();
                }
                Some(DialogAction::None)
            }
            FontSelectorMessage::NavigatePageUp => {
                self.page_up();
                Some(DialogAction::None)
            }
            FontSelectorMessage::NavigatePageDown => {
                self.page_down();
                Some(DialogAction::None)
            }
            FontSelectorMessage::Apply => {
                if let Some(result) = self.create_result() {
                    Some(DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::Core(
                        AnsiEditorCoreMessage::ApplyFontSelection(result),
                    ))))
                } else {
                    Some(DialogAction::None)
                }
            }
            FontSelectorMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn handle_event(&mut self, event: &iced::Event) -> Option<DialogAction<Message>> {
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                use iced::keyboard::Key;
                use iced::keyboard::key::Named;

                match key {
                    // Only handle Enter globally for applying the selection
                    Key::Named(Named::Enter) => {
                        if let Some(result) = self.create_result() {
                            return Some(DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::Core(
                                AnsiEditorCoreMessage::ApplyFontSelection(result),
                            ))));
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
            DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ApplyFontSelection(result))))
        } else {
            DialogAction::None
        }
    }
}
