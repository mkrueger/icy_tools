//! New File Dialog - Split-View Design with Canvas List
//!
//! A modern dialog for creating new files with:
//! - Left panel: Canvas-based collapsible category tree with overlay scrollbar
//! - Right panel: Template preview and details with size controls
//! - Full keyboard navigation (Up/Down/Enter/PageUp/PageDown/Home/End)
//! - 4 Editor types: ANSI Art, Bit Font, TDF Font, Animation

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use iced::{
    keyboard::{key::Named, Key},
    mouse,
    widget::{
        canvas::{self, Canvas, Frame, Geometry, Path, Text},
        column, container, row, text, text_input, Space,
    },
    Alignment, Element, Length, Point, Rectangle, Renderer, Size, Theme,
};

use icy_engine::{BitFont, FontMode, IceMode, TextBuffer};
use icy_engine_gui::{
    focus,
    settings::effect_box,
    ui::{
        dialog_area, left_label_small, modal_container, primary_button, secondary_button, separator, Dialog, DialogAction, DIALOG_SPACING, HEADER_TEXT_SIZE,
        TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
    },
    ButtonType, ScrollbarOverlay, Viewport,
};

use crate::{fl, ui::Message};

// ============================================================================
// Constants
// ============================================================================

const DIALOG_WIDTH: f32 = 700.0;
const DIALOG_HEIGHT: f32 = 400.0;
const LEFT_PANEL_WIDTH: f32 = 280.0;
const CATEGORY_HEADER_HEIGHT: f32 = 32.0;
const TEMPLATE_ITEM_HEIGHT: f32 = 28.0;
const TEMPLATE_INDENT: f32 = 24.0;

// ============================================================================
// Editor Type - the 4 main editor categories
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorType {
    AnsiArt,
    BitFont,
    TdfFont,
    Animation,
}

impl EditorType {
    fn label(&self) -> String {
        match self {
            EditorType::AnsiArt => fl!("new-file-editor-ansi"),
            EditorType::BitFont => fl!("new-file-editor-bitfont"),
            EditorType::TdfFont => fl!("new-file-editor-tdf"),
            EditorType::Animation => fl!("new-file-editor-animation"),
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            EditorType::AnsiArt => "🖼",
            EditorType::BitFont => "🔤",
            EditorType::TdfFont => "✏",
            EditorType::Animation => "🎬",
        }
    }

    fn all() -> [EditorType; 4] {
        [EditorType::AnsiArt, EditorType::BitFont, EditorType::TdfFont, EditorType::Animation]
    }
}

// ============================================================================
// File Template
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTemplate {
    // ANSI Art templates
    ModernAnsi,
    Dos16,
    Ice16,
    XBin16,
    XBinExtended,
    // Bit Font
    BitFont,
    // TDF Font templates
    ColorFont,
    BlockFont,
    OutlineFont,
    // Animation
    Animation,
}

impl FileTemplate {
    fn editor_type(&self) -> EditorType {
        match self {
            FileTemplate::ModernAnsi | FileTemplate::Dos16 | FileTemplate::Ice16 | FileTemplate::XBin16 | FileTemplate::XBinExtended => EditorType::AnsiArt,
            FileTemplate::BitFont => EditorType::BitFont,
            FileTemplate::ColorFont | FileTemplate::BlockFont | FileTemplate::OutlineFont => EditorType::TdfFont,
            FileTemplate::Animation => EditorType::Animation,
        }
    }

    fn title(&self) -> String {
        match self {
            FileTemplate::ModernAnsi => fl!("new-file-template-ansi-title"),
            FileTemplate::Dos16 => fl!("new-file-template-cp437-title"),
            FileTemplate::Ice16 => fl!("new-file-template-ice-title"),
            FileTemplate::XBin16 => fl!("new-file-template-xb-title"),
            FileTemplate::XBinExtended => fl!("new-file-template-xb-ext-title"),
            FileTemplate::BitFont => fl!("new-file-template-bit_font-title"),
            FileTemplate::ColorFont => fl!("new-file-template-color_font-title"),
            FileTemplate::BlockFont => fl!("new-file-template-block_font-title"),
            FileTemplate::OutlineFont => fl!("new-file-template-outline_font-title"),
            FileTemplate::Animation => fl!("new-file-template-ansimation-title"),
        }
    }

    fn description(&self) -> String {
        match self {
            FileTemplate::ModernAnsi => fl!("new-file-template-ansi-description"),
            FileTemplate::Dos16 => fl!("new-file-template-cp437-description"),
            FileTemplate::Ice16 => fl!("new-file-template-ice-description"),
            FileTemplate::XBin16 => fl!("new-file-template-xb-description"),
            FileTemplate::XBinExtended => fl!("new-file-template-xb-ext-description"),
            FileTemplate::BitFont => fl!("new-file-template-bit_font-description"),
            FileTemplate::ColorFont => fl!("new-file-template-color_font-description"),
            FileTemplate::BlockFont => fl!("new-file-template-block_font-description"),
            FileTemplate::OutlineFont => fl!("new-file-template-outline_font-description"),
            FileTemplate::Animation => fl!("new-file-template-ansimation-description"),
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            FileTemplate::ModernAnsi => "📄",
            FileTemplate::Dos16 => "💾",
            FileTemplate::Ice16 => "❄",
            FileTemplate::XBin16 => "🎨",
            FileTemplate::XBinExtended => "🎨",
            FileTemplate::BitFont => "🔤",
            FileTemplate::ColorFont => "🌈",
            FileTemplate::BlockFont => "▓",
            FileTemplate::OutlineFont => "□",
            FileTemplate::Animation => "🎬",
        }
    }

    fn default_width(&self) -> i32 {
        match self {
            FileTemplate::BitFont => 8,
            _ => 80,
        }
    }

    fn default_height(&self) -> i32 {
        match self {
            FileTemplate::BitFont => 16,
            _ => 25,
        }
    }

    fn needs_size(&self) -> bool {
        match self {
            FileTemplate::ColorFont | FileTemplate::BlockFont | FileTemplate::OutlineFont | FileTemplate::Animation => false,
            _ => true,
        }
    }

    fn templates_for_editor(editor: EditorType) -> Vec<FileTemplate> {
        match editor {
            EditorType::AnsiArt => vec![
                FileTemplate::ModernAnsi,
                FileTemplate::Dos16,
                FileTemplate::Ice16,
                FileTemplate::XBin16,
                FileTemplate::XBinExtended,
            ],
            EditorType::BitFont => vec![FileTemplate::BitFont],
            EditorType::TdfFont => vec![FileTemplate::ColorFont, FileTemplate::BlockFont, FileTemplate::OutlineFont],
            EditorType::Animation => vec![FileTemplate::Animation],
        }
    }
}

/// Create a TextBuffer for a given template with specified dimensions
pub fn create_buffer_for_template(template: FileTemplate, width: i32, height: i32) -> TextBuffer {
    let mut buf = TextBuffer::new((width.max(1), height.max(1)));
    if let Ok(font) = BitFont::from_sauce_name("IBM VGA") {
        buf.set_font(0, font);
    }

    match template {
        FileTemplate::ModernAnsi => {
            buf.ice_mode = IceMode::Unlimited;
            buf.font_mode = FontMode::Unlimited;
        }
        FileTemplate::Dos16 => {
            buf.ice_mode = IceMode::Blink;
            buf.font_mode = FontMode::Sauce;
        }
        FileTemplate::Ice16 => {
            buf.ice_mode = IceMode::Ice;
            buf.font_mode = FontMode::Sauce;
        }
        FileTemplate::XBin16 => {
            buf.ice_mode = IceMode::Ice;
            buf.font_mode = FontMode::Single;
        }
        FileTemplate::XBinExtended => {
            buf.ice_mode = IceMode::Ice;
            buf.font_mode = FontMode::FixedSize;
            buf.set_font(1, BitFont::default());
        }
        FileTemplate::BitFont | FileTemplate::ColorFont | FileTemplate::BlockFont | FileTemplate::OutlineFont | FileTemplate::Animation => {
            buf.ice_mode = IceMode::Blink;
            buf.font_mode = FontMode::Sauce;
        }
    }

    buf
}

// ============================================================================
// List Item - represents a visual row in the list
// ============================================================================

#[derive(Debug, Clone)]
enum ListItem {
    CategoryHeader { editor: EditorType, count: usize },
    TemplateItem { template: FileTemplate },
}

// ============================================================================
// Dialog Messages
// ============================================================================

#[derive(Debug, Clone)]
pub enum NewFileMessage {
    ToggleEditor(EditorType),
    SelectTemplate(FileTemplate),
    SetWidth(String),
    SetHeight(String),
    NavigateUp,
    NavigateDown,
    NavigateHome,
    NavigateEnd,
    NavigatePageUp,
    NavigatePageDown,
    Create(FileTemplate, i32, i32),
    Cancel,
}

// ============================================================================
// Category State
// ============================================================================

struct CategoryState {
    expanded: bool,
    templates: Vec<FileTemplate>,
}

// ============================================================================
// Dialog
// ============================================================================

pub struct NewFileDialog {
    selected_template: FileTemplate,
    categories: HashMap<EditorType, CategoryState>,
    width: i32,
    height: i32,
    width_input: String,
    height_input: String,
    list_viewport: RefCell<Viewport>,
    visible_items: RefCell<Vec<ListItem>>,
    last_click: RefCell<Option<(Instant, FileTemplate)>>,
}

impl Default for NewFileDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl NewFileDialog {
    pub fn new() -> Self {
        let template = FileTemplate::ModernAnsi;
        let mut categories = HashMap::new();

        // Build category states
        for editor in EditorType::all() {
            let templates = FileTemplate::templates_for_editor(editor);
            categories.insert(editor, CategoryState { expanded: true, templates });
        }

        let dialog = Self {
            selected_template: template,
            categories,
            width: template.default_width(),
            height: template.default_height(),
            width_input: template.default_width().to_string(),
            height_input: template.default_height().to_string(),
            list_viewport: RefCell::new(Viewport::default()),
            visible_items: RefCell::new(Vec::new()),
            last_click: RefCell::new(None),
        };

        dialog.rebuild_visible_items();
        dialog
    }

    /// Rebuild the list of visible items based on category expansion states
    fn rebuild_visible_items(&self) {
        let mut items = Vec::new();

        for editor in EditorType::all() {
            if let Some(state) = self.categories.get(&editor) {
                // Add category header
                items.push(ListItem::CategoryHeader {
                    editor,
                    count: state.templates.len(),
                });

                // Add template items if expanded
                if state.expanded {
                    for &template in &state.templates {
                        items.push(ListItem::TemplateItem { template });
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
                ListItem::TemplateItem { .. } => TEMPLATE_ITEM_HEIGHT,
            })
            .sum()
    }

    /// Get all visible templates (in expanded categories)
    fn get_visible_templates(&self) -> Vec<FileTemplate> {
        let mut result = Vec::new();
        for editor in EditorType::all() {
            if let Some(state) = self.categories.get(&editor) {
                if state.expanded {
                    result.extend(state.templates.iter().copied());
                }
            }
        }
        result
    }

    /// Get the Y position of the selected template in the list
    fn get_selection_y_position(&self) -> Option<f32> {
        let items = self.visible_items.borrow();
        let mut y = 0.0;

        for item in items.iter() {
            let height = match item {
                ListItem::CategoryHeader { .. } => CATEGORY_HEADER_HEIGHT,
                ListItem::TemplateItem { template } => {
                    if *template == self.selected_template {
                        return Some(y);
                    }
                    TEMPLATE_ITEM_HEIGHT
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
            else if y + TEMPLATE_ITEM_HEIGHT > vp.scroll_y + visible_height {
                let new_scroll = y + TEMPLATE_ITEM_HEIGHT - visible_height;
                vp.scroll_y = new_scroll;
                vp.target_scroll_y = new_scroll;
                vp.sync_scrollbar_position();
            }
        }
    }

    fn find_prev_template(&self) -> Option<FileTemplate> {
        let visible = self.get_visible_templates();
        if visible.is_empty() {
            return None;
        }

        if let Some(pos) = visible.iter().position(|&t| t == self.selected_template) {
            if pos > 0 {
                Some(visible[pos - 1])
            } else {
                None
            }
        } else {
            visible.last().copied()
        }
    }

    fn find_next_template(&self) -> Option<FileTemplate> {
        let visible = self.get_visible_templates();
        if visible.is_empty() {
            return None;
        }

        if let Some(pos) = visible.iter().position(|&t| t == self.selected_template) {
            if pos + 1 < visible.len() {
                Some(visible[pos + 1])
            } else {
                None
            }
        } else {
            visible.first().copied()
        }
    }

    fn find_first_template(&self) -> Option<FileTemplate> {
        self.get_visible_templates().first().copied()
    }

    fn find_last_template(&self) -> Option<FileTemplate> {
        self.get_visible_templates().last().copied()
    }

    fn page_up(&mut self) {
        let visible = self.get_visible_templates();
        if visible.is_empty() {
            return;
        }

        let visible_height = self.list_viewport.borrow().visible_height;
        let items_per_page = (visible_height / TEMPLATE_ITEM_HEIGHT).max(1.0) as usize;

        if let Some(pos) = visible.iter().position(|&t| t == self.selected_template) {
            let new_pos = pos.saturating_sub(items_per_page);
            self.selected_template = visible[new_pos];
            self.update_size_for_template();
        } else if let Some(&first) = visible.first() {
            self.selected_template = first;
            self.update_size_for_template();
        }
        self.scroll_to_selection();
    }

    fn page_down(&mut self) {
        let visible = self.get_visible_templates();
        if visible.is_empty() {
            return;
        }

        let visible_height = self.list_viewport.borrow().visible_height;
        let items_per_page = (visible_height / TEMPLATE_ITEM_HEIGHT).max(1.0) as usize;

        if let Some(pos) = visible.iter().position(|&t| t == self.selected_template) {
            let new_pos = (pos + items_per_page).min(visible.len() - 1);
            self.selected_template = visible[new_pos];
            self.update_size_for_template();
        } else if let Some(&last) = visible.last() {
            self.selected_template = last;
            self.update_size_for_template();
        }
        self.scroll_to_selection();
    }

    fn update_size_for_template(&mut self) {
        self.width = self.selected_template.default_width();
        self.height = self.selected_template.default_height();
        self.width_input = self.width.to_string();
        self.height_input = self.height.to_string();
    }

    // ========================================================================
    // View Methods
    // ========================================================================

    fn view_left_panel(&self) -> Element<'_, Message> {
        // Canvas-based list with overlay scrollbar
        let list_canvas: Element<'_, Message> = Canvas::new(TemplateListCanvas { dialog: self }).width(Length::Fill).height(Length::Fill).into();

        let scrollbar: Element<'_, Message> = ScrollbarOverlay::new(&self.list_viewport)
            .view()
            .map(|_| Message::NewFileDialog(NewFileMessage::Cancel)); // Dummy mapping

        let list_with_scrollbar = row![list_canvas, scrollbar];

        // Wrap in Focus widget for keyboard navigation
        let focusable_list: Element<'_, Message> = focus(list_with_scrollbar)
            .on_event(|event, _id| {
                if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                    match key {
                        Key::Named(Named::ArrowUp) => Some(Message::NewFileDialog(NewFileMessage::NavigateUp)),
                        Key::Named(Named::ArrowDown) => Some(Message::NewFileDialog(NewFileMessage::NavigateDown)),
                        Key::Named(Named::Home) => Some(Message::NewFileDialog(NewFileMessage::NavigateHome)),
                        Key::Named(Named::End) => Some(Message::NewFileDialog(NewFileMessage::NavigateEnd)),
                        Key::Named(Named::PageUp) => Some(Message::NewFileDialog(NewFileMessage::NavigatePageUp)),
                        Key::Named(Named::PageDown) => Some(Message::NewFileDialog(NewFileMessage::NavigatePageDown)),
                        Key::Named(Named::Enter) => Some(Message::NewFileDialog(NewFileMessage::Create(
                            // Will be filled with current selection in update
                            FileTemplate::ModernAnsi,
                            80,
                            25,
                        ))),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .into();

        container(focusable_list)
            .width(Length::Fixed(LEFT_PANEL_WIDTH))
            .height(Length::Fill)
            .style(|theme: &iced::Theme| {
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
            .into()
    }

    fn view_right_panel(&self) -> Element<'_, Message> {
        let template = self.selected_template;

        // Icon
        let icon = text(template.icon()).size(32);

        // Title with standard header size
        let title = text(template.title()).size(HEADER_TEXT_SIZE).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        });

        // Editor type badge with consistent styling
        let editor_type = template.editor_type();
        let badge = container(
            row![
                text(editor_type.icon()).size(TEXT_SIZE_SMALL),
                Space::new().width(4.0),
                text(editor_type.label()).size(TEXT_SIZE_SMALL),
            ]
            .align_y(Alignment::Center),
        )
        .style(|theme: &iced::Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background.strong.color)),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .padding([3, 8]);

        // Description
        let description = text(template.description())
            .size(TEXT_SIZE_NORMAL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.base.text.scale_alpha(0.8)),
            });

        // Size inputs (only for templates that need it)
        let size_section: Element<'_, Message> = if template.needs_size() {
            let width_input = text_input("", &self.width_input)
                .on_input(|s| Message::NewFileDialog(NewFileMessage::SetWidth(s)))
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fixed(80.0));

            let width_row = row![left_label_small(fl!("new-file-width")), width_input]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center);

            let height_input = text_input("", &self.height_input)
                .on_input(|s| Message::NewFileDialog(NewFileMessage::SetHeight(s)))
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fixed(80.0));

            let height_row = row![left_label_small(fl!("new-file-height")), height_input]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center);

            let size_content = column![width_row, height_row].spacing(DIALOG_SPACING);

            effect_box(size_content.into())
        } else {
            Space::new().height(0.0).into()
        };

        let content = column![
            row![
                icon,
                Space::new().width(12.0),
                column![title, Space::new().height(4.0), badge].align_x(Alignment::Start)
            ]
            .align_y(Alignment::Center),
            Space::new().height(DIALOG_SPACING),
            description,
            Space::new().height(DIALOG_SPACING * 2.0),
            size_section,
        ]
        .padding(DIALOG_SPACING);

        container(content).width(Length::Fill).height(Length::Fill).into()
    }
}

// ============================================================================
// Template List Canvas
// ============================================================================

struct TemplateListCanvas<'a> {
    dialog: &'a NewFileDialog,
}

impl<'a> canvas::Program<Message> for TemplateListCanvas<'a> {
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
                    ListItem::TemplateItem { .. } => TEMPLATE_ITEM_HEIGHT,
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
                    ListItem::CategoryHeader { editor, count } => {
                        // Draw category header background
                        let bg_rect = Path::rectangle(Point::new(0.0, y), Size::new(bounds.width, height));
                        frame.fill(&bg_rect, palette.background.strong.color);

                        // Draw arrow and text
                        let expanded = self.dialog.categories.get(editor).map(|s| s.expanded).unwrap_or(true);
                        let arrow = if expanded { "▼" } else { "▶" };
                        let label = format!("{} {} {} ({})", arrow, editor.icon(), editor.label(), count);

                        frame.fill_text(Text {
                            content: label,
                            position: Point::new(12.0, y + (height - 14.0) / 2.0),
                            color: palette.background.base.text,
                            size: iced::Pixels(14.0),
                            ..Default::default()
                        });
                    }
                    ListItem::TemplateItem { template } => {
                        let is_selected = self.dialog.selected_template == *template;

                        // Draw selection background
                        if is_selected {
                            let bg_rect = Path::rectangle(Point::new(0.0, y), Size::new(bounds.width, height));
                            frame.fill(&bg_rect, palette.primary.weak.color);
                        }

                        // Draw icon and template name
                        let text_color = if is_selected {
                            palette.primary.weak.text
                        } else {
                            palette.background.base.text
                        };

                        // Icon
                        frame.fill_text(Text {
                            content: template.icon().to_string(),
                            position: Point::new(TEMPLATE_INDENT, y + (height - 14.0) / 2.0),
                            color: text_color,
                            size: iced::Pixels(14.0),
                            ..Default::default()
                        });

                        // Template name
                        frame.fill_text(Text {
                            content: template.title(),
                            position: Point::new(TEMPLATE_INDENT + 24.0, y + (height - 13.0) / 2.0),
                            color: text_color,
                            size: iced::Pixels(13.0),
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
                            ListItem::TemplateItem { .. } => TEMPLATE_ITEM_HEIGHT,
                        };

                        if click_y >= current_y && click_y < current_y + height {
                            match item {
                                ListItem::CategoryHeader { editor, .. } => {
                                    return Some(canvas::Action::publish(Message::NewFileDialog(NewFileMessage::ToggleEditor(*editor))));
                                }
                                ListItem::TemplateItem { template } => {
                                    // Check for double-click (within 500ms on same template)
                                    let now = Instant::now();
                                    let is_double_click = {
                                        let last = self.dialog.last_click.borrow();
                                        if let Some((last_time, last_template)) = *last {
                                            last_template == *template && now.duration_since(last_time).as_millis() < 500
                                        } else {
                                            false
                                        }
                                    };

                                    if is_double_click {
                                        // Double-click: create the file
                                        *self.dialog.last_click.borrow_mut() = None;
                                        return Some(canvas::Action::publish(Message::NewFileDialog(NewFileMessage::Create(
                                            *template,
                                            template.default_width(),
                                            template.default_height(),
                                        ))));
                                    } else {
                                        // Single click: select and record time
                                        *self.dialog.last_click.borrow_mut() = Some((now, *template));
                                        return Some(canvas::Action::publish(Message::NewFileDialog(NewFileMessage::SelectTemplate(*template))));
                                    }
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
// Dialog Implementation
// ============================================================================

impl Dialog<Message> for NewFileDialog {
    fn view(&self) -> Element<'_, Message> {
        // Split view: left panel (tree) + right panel (details)
        let main_content = row![self.view_left_panel(), Space::new().width(DIALOG_SPACING), self.view_right_panel(),]
            .spacing(0)
            .height(Length::Fixed(DIALOG_HEIGHT));

        let dialog_content = dialog_area(main_content.into());

        // Buttons
        let cancel_btn = secondary_button(format!("{}", ButtonType::Cancel), Some(Message::NewFileDialog(NewFileMessage::Cancel)));

        let create_btn = primary_button(
            fl!("new-file-create"),
            Some(Message::NewFileDialog(NewFileMessage::Create(self.selected_template, self.width, self.height))),
        );

        let buttons = row![Space::new().width(Length::Fill), cancel_btn, create_btn].spacing(DIALOG_SPACING);

        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        if let Message::NewFileDialog(msg) = message {
            match msg {
                NewFileMessage::ToggleEditor(editor) => {
                    if let Some(state) = self.categories.get_mut(editor) {
                        state.expanded = !state.expanded;
                    }
                    self.rebuild_visible_items();
                    // If selection is now hidden, select first visible
                    let visible = self.get_visible_templates();
                    if !visible.contains(&self.selected_template) {
                        if let Some(&first) = visible.first() {
                            self.selected_template = first;
                            self.update_size_for_template();
                        }
                    }
                    return Some(DialogAction::None);
                }
                NewFileMessage::SelectTemplate(template) => {
                    self.selected_template = *template;
                    self.update_size_for_template();
                    self.scroll_to_selection();
                    return Some(DialogAction::None);
                }
                NewFileMessage::SetWidth(w) => {
                    self.width_input = w.chars().take(5).collect();
                    if let Ok(v) = self.width_input.parse::<i32>() {
                        self.width = v.max(1).min(9999);
                    }
                    return Some(DialogAction::None);
                }
                NewFileMessage::SetHeight(h) => {
                    self.height_input = h.chars().take(5).collect();
                    if let Ok(v) = self.height_input.parse::<i32>() {
                        self.height = v.max(1).min(9999);
                    }
                    return Some(DialogAction::None);
                }
                NewFileMessage::NavigateUp => {
                    if let Some(prev) = self.find_prev_template() {
                        self.selected_template = prev;
                        self.update_size_for_template();
                        self.scroll_to_selection();
                    }
                    return Some(DialogAction::None);
                }
                NewFileMessage::NavigateDown => {
                    if let Some(next) = self.find_next_template() {
                        self.selected_template = next;
                        self.update_size_for_template();
                        self.scroll_to_selection();
                    }
                    return Some(DialogAction::None);
                }
                NewFileMessage::NavigateHome => {
                    if let Some(first) = self.find_first_template() {
                        self.selected_template = first;
                        self.update_size_for_template();
                        self.scroll_to_selection();
                    }
                    return Some(DialogAction::None);
                }
                NewFileMessage::NavigateEnd => {
                    if let Some(last) = self.find_last_template() {
                        self.selected_template = last;
                        self.update_size_for_template();
                        self.scroll_to_selection();
                    }
                    return Some(DialogAction::None);
                }
                NewFileMessage::NavigatePageUp => {
                    self.page_up();
                    return Some(DialogAction::None);
                }
                NewFileMessage::NavigatePageDown => {
                    self.page_down();
                    return Some(DialogAction::None);
                }
                NewFileMessage::Create(_, _, _) => {
                    // Use current selection, not the passed values (which may be dummy from keyboard)
                    return Some(DialogAction::CloseWith(Message::NewFileCreated(
                        self.selected_template,
                        self.width,
                        self.height,
                    )));
                }
                NewFileMessage::Cancel => return Some(DialogAction::Close),
            }
        }
        None
    }

    fn handle_event(&mut self, event: &iced::Event) -> Option<DialogAction<Message>> {
        if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
            if let Key::Named(Named::Enter) = key {
                return Some(DialogAction::CloseWith(Message::NewFileCreated(
                    self.selected_template,
                    self.width,
                    self.height,
                )));
            }
        }
        None
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        DialogAction::CloseWith(Message::NewFileCreated(self.selected_template, self.width, self.height))
    }
}
