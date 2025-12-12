//! Font Selector Dialog - Split-View Design
//!
//! A unified font selector with:
//! - Left panel: Search + grouped font list  
//! - Right panel: Large preview + font details

use std::cell::RefCell;
use std::collections::HashMap;

use iced::{
    Alignment, Color, Element, Length, Theme,
    widget::{
        Space, button, scrollable, rule,
        column, container, image, row, text, text_input,
    },
};
use icy_engine::{AttributedChar, BitFont, FontMode, RenderOptions, SAUCE_FONT_NAMES, TextAttribute, TextBuffer};
use icy_engine_edit::FormatMode;
use icy_engine_gui::ButtonType;
use icy_engine_gui::ui::{
    DIALOG_SPACING, Dialog, DialogAction, dialog_area, modal_container, primary_button,
    secondary_button, separator,
};

use crate::fl;
use crate::ui::Message;

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
pub(crate) enum FontCategory {
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
            FontCategory::Sauce => "🏷️",
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
    /// From font library
    pub is_library: bool,
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
    current_font_name: String,
    current_font_size: (i32, i32),

    /// Filter string
    filter: String,

    /// Category states for collapsible sections
    categories: HashMap<FontCategory, CategoryState>,

    /// Only show SAUCE-compatible fonts
    only_sauce_fonts: bool,

    /// For XBin Extended: which slot is being edited (0 or 1)
    active_slot: usize,

    /// For XBin Extended: current fonts in slots
    slot_fonts: [Option<BitFont>; 2],

    /// Cached preview for large display
    large_preview_cache: RefCell<Option<(usize, CachedPreview)>>,
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
                if let Ok(ansi_font) = BitFont::from_ansi_font_page(slot, 25) {
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

        // Track current font from document
        let current_font_page = state.get_caret().font_page();
        let mut selected_index = 0;
        let mut current_font_name = String::new();
        let mut current_font_size = (8, 16);

        // Mark document fonts and find current font
        for (slot, doc_font) in buffer.font_iter() {
            let key = font_key(doc_font);
            if *slot == current_font_page {
                current_font_name = doc_font.name().to_string();
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
                        is_library: true, // Treat as library font
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

        // Build category states (without Document category)
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

        Self {
            format_mode,
            fonts,
            selected_index,
            current_font_name,
            current_font_size,
            filter: String::new(),
            categories,
            only_sauce_fonts,
            active_slot: 0,
            slot_fonts,
            large_preview_cache: RefCell::new(None),
        }
    }

    fn selected_font(&self) -> Option<&FontEntry> {
        self.fonts.get(self.selected_index)
    }

    fn matches_filter(&self, entry: &FontEntry) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        entry
            .font
            .name()
            .to_lowercase()
            .contains(&self.filter.to_lowercase())
    }

    /// Generate preview showing all 256 characters in a 16x16 grid
    fn generate_large_preview(&self, font_idx: usize) -> Option<CachedPreview> {
        let entry = self.fonts.get(font_idx)?;
        let font = &entry.font;

        let mut buffer = TextBuffer::new((PREVIEW_CHARS_WIDTH, PREVIEW_CHARS_HEIGHT));
        buffer.set_font(0, font.clone());

        // Fill with all 256 characters in a 16x16 grid
        for ch_code in 0..256u32 {
            let x = (ch_code % 16) as i32;
            let y = (ch_code / 16) as i32;
            let ch = unsafe { char::from_u32_unchecked(ch_code) };
            buffer.layers[0].set_char(
                (x, y),
                AttributedChar::new(ch, TextAttribute::default()),
            );
        }

        let options = RenderOptions::default();
        let region = icy_engine::Rectangle::from(
            0,
            0,
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

    fn get_large_preview(&self, font_idx: usize) -> Option<CachedPreview> {
        // Check cache
        if let Some((cached_idx, preview)) = self.large_preview_cache.borrow().as_ref() {
            if *cached_idx == font_idx {
                return Some(preview.clone());
            }
        }

        // Generate and cache
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
            FormatMode::XBinExtended => Some(FontSelectorResult::FontForSlot {
                slot: self.active_slot,
                font: entry.font.clone(),
            }),
            FormatMode::Unrestricted => {
                let slot = entry.source.document_slot.unwrap_or(0);
                Some(FontSelectorResult::FontForSlot {
                    slot,
                    font: entry.font.clone(),
                })
            }
        }
    }

    fn get_visible_fonts(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for cat in [
            FontCategory::Sauce,
            FontCategory::Ansi,
            FontCategory::Library,
        ] {
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
        let current_pos = visible_fonts.iter().position(|&i| i == self.selected_index)?;
        if current_pos > 0 {
            Some(visible_fonts[current_pos - 1])
        } else {
            None
        }
    }

    fn find_next_font(&self) -> Option<usize> {
        let visible_fonts = self.get_visible_fonts();
        let current_pos = visible_fonts.iter().position(|&i| i == self.selected_index)?;
        if current_pos + 1 < visible_fonts.len() {
            Some(visible_fonts[current_pos + 1])
        } else {
            None
        }
    }

    // ========================================================================
    // View Methods
    // ========================================================================

    fn view_split_layout(&self) -> Element<'_, Message> {
        // Left panel: Search + Font List
        let left_panel = self.view_left_panel();

        // Right panel: Preview + Details
        let right_panel = self.view_right_panel();

        // Main content with split
        let content = row![
            container(left_panel)
                .width(Length::Fixed(LEFT_PANEL_WIDTH))
                .height(Length::Fixed(DIALOG_HEIGHT - 80.0)), // Leave room for current font + buttons
            rule::vertical(1),
            container(right_panel)
                .width(Length::Fill)
                .height(Length::Fixed(DIALOG_HEIGHT - 80.0)),
        ]
        .spacing(0);

        // Current font info bar
        let current_font_bar = self.view_current_font();

        // Buttons
        let button_row = row![
            Space::new().width(Length::Fill),
            secondary_button(
                format!("{}", ButtonType::Cancel),
                Some(Message::FontSelector(FontSelectorMessage::Cancel))
            ),
            primary_button(
                format!("{}", ButtonType::Ok),
                self.selected_font()
                    .map(|_| Message::FontSelector(FontSelectorMessage::Apply))
            ),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let dialog_content = dialog_area(content.into());
        let current_font_area = dialog_area(current_font_bar.into());
        let button_area = dialog_area(button_row.into());

        modal_container(
            column![
                container(dialog_content).height(Length::Shrink),
                separator(),
                current_font_area,
                separator(),
                button_area
            ]
            .into(),
            DIALOG_WIDTH,
        )
        .into()
    }

    fn view_current_font(&self) -> Element<'_, Message> {
        row![
            text("Aktueller Font:").size(12),
            Space::new().width(8.0),
            text(&self.current_font_name).size(12),
            Space::new().width(Length::Fill),
            text(format!("{}×{}", self.current_font_size.0, self.current_font_size.1))
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
        ]
        .align_y(Alignment::Center)
        .into()
    }

    fn view_left_panel(&self) -> Element<'_, Message> {
        // Search input
        let search_input = text_input(&fl!("font-selector-filter-placeholder"), &self.filter)
            .on_input(|s| Message::FontSelector(FontSelectorMessage::SetFilter(s)))
            .width(Length::Fill)
            .padding(8);

        // Font list with categories
        let font_list = self.view_font_list();

        column![container(search_input).padding(8), font_list,].into()
    }

    fn view_font_list(&self) -> Element<'_, Message> {
        let mut list_content = column![].spacing(0);

        for cat in [
            FontCategory::Sauce,
            FontCategory::Ansi,
            FontCategory::Library,
        ] {
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

                // Category header
                let header = self.view_category_header(cat, filtered_fonts.len());
                list_content = list_content.push(header);

                // Font items (if expanded)
                if state.expanded {
                    for &font_idx in &filtered_fonts {
                        let item = self.view_font_item(font_idx);
                        list_content = list_content.push(item);
                    }
                }
            }
        }

        scrollable(list_content).height(Length::Fill).into()
    }

    fn view_category_header(&self, category: FontCategory, count: usize) -> Element<'_, Message> {
        let state = self.categories.get(&category);
        let expanded = state.map(|s| s.expanded).unwrap_or(true);
        let arrow = if expanded { "▼" } else { "▶" };

        let header_content = row![
            text(format!("{} {} {}", arrow, category.icon(), category.label())).size(13),
            Space::new().width(Length::Fill),
            text(format!("({})", count)).size(11),
        ]
        .align_y(Alignment::Center)
        .padding([6, 12]);

        button(header_content)
            .on_press(Message::FontSelector(FontSelectorMessage::ToggleCategory(
                category,
            )))
            .style(category_header_style)
            .width(Length::Fill)
            .into()
    }

    fn view_font_item(&self, font_idx: usize) -> Element<'_, Message> {
        let entry = &self.fonts[font_idx];
        let is_selected = self.selected_index == font_idx;

        let item_content = row![
            Space::new().width(Length::Fixed(24.0)),
            text(entry.font.name()).size(12),
            Space::new().width(Length::Fill),
            text(format!(
                "{}×{}",
                entry.font.size().width,
                entry.font.size().height
            ))
            .size(10)
            .color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .align_y(Alignment::Center)
        .padding([4, 12]);

        let style = if is_selected {
            selected_item_style
        } else {
            normal_item_style
        };

        button(item_content)
            .on_press(Message::FontSelector(FontSelectorMessage::SelectFont(
                font_idx,
            )))
            .style(style)
            .width(Length::Fill)
            .into()
    }

    fn view_right_panel(&self) -> Element<'_, Message> {
        let Some(entry) = self.selected_font() else {
            return container(text("Keine Schrift ausgewählt").size(14))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };

        // Preview image with fixed scaling showing all 256 chars
        let preview: Element<'_, Message> = if let Some(cached) = self.get_large_preview(self.selected_index) {
            // Fixed 2x scaling for crisp pixel display
            let display_width = cached.width as f32 * PREVIEW_SCALE;
            let display_height = cached.height as f32 * PREVIEW_SCALE;

            container(
                image(cached.handle)
                    .width(Length::Fixed(display_width))
                    .height(Length::Fixed(display_height)),
            )
            .style(preview_container_style)
            .padding(12)
            .into()
        } else {
            container(text("Vorschau wird generiert...").size(12))
                .center_x(Length::Fill)
                .height(Length::Fixed(200.0))
                .into()
        };

        // Compact font info line: Name left, dimensions right
        let font_info = row![
            text(entry.font.name()).size(14),
            Space::new().width(Length::Fill),
            text(format!("{}×{}", entry.font.size().width, entry.font.size().height))
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
        ]
        .align_y(Alignment::Center)
        .padding([0, 16]);

        column![
            Space::new().height(16.0),
            container(preview).center_x(Length::Fill),
            Space::new().height(Length::Fill),
            font_info,
            Space::new().height(8.0),
        ]
        .into()
    }
}

// ============================================================================
// Helper functions
// ============================================================================

// ============================================================================
// Styles
// ============================================================================

fn category_header_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(iced::Background::Color(palette.background.weak.color)),
        text_color: palette.background.base.text,
        border: iced::Border::default(),
        ..Default::default()
    }
}

fn selected_item_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(iced::Background::Color(palette.primary.weak.color)),
        text_color: palette.primary.weak.text,
        border: iced::Border::default(),
        ..Default::default()
    }
}

fn normal_item_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Hovered => Some(iced::Background::Color(palette.background.weak.color)),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: palette.background.base.text,
        border: iced::Border::default(),
        ..Default::default()
    }
}

fn preview_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(iced::Background::Color(Color::BLACK)),
        border: iced::Border {
            radius: 4.0.into(),
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
        let Message::FontSelector(msg) = message else {
            return None;
        };

        match msg {
            FontSelectorMessage::SetFilter(f) => {
                self.filter = f.clone();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ClearFilter => {
                self.filter.clear();
                Some(DialogAction::None)
            }
            FontSelectorMessage::ToggleCategory(cat) => {
                if let Some(state) = self.categories.get_mut(cat) {
                    state.expanded = !state.expanded;
                }
                Some(DialogAction::None)
            }
            FontSelectorMessage::SelectFont(idx) => {
                self.selected_index = *idx;
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
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                use iced::keyboard::key::Named;
                use iced::keyboard::Key;

                match key {
                    Key::Named(Named::ArrowUp) => {
                        if let Some(prev) = self.find_prev_font() {
                            self.selected_index = prev;
                            return Some(DialogAction::None);
                        }
                    }
                    Key::Named(Named::ArrowDown) => {
                        if let Some(next) = self.find_next_font() {
                            self.selected_index = next;
                            return Some(DialogAction::None);
                        }
                    }
                    Key::Named(Named::Enter) => {
                        if let Some(result) = self.create_result() {
                            return Some(DialogAction::CloseWith(Message::ApplyFontSelection(result)));
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
}
