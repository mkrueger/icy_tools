//! Font Slot Manager Dialog
//!
//! A dialog for managing font slots in Unrestricted mode.
//! Shows all 43 ANSI slots (0-42) plus any additional custom slots.
//! Allows: Set font, Reset to default, Add new slot, Remove custom slot.

use std::cell::RefCell;
use std::collections::HashMap;

use iced::{
    Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    keyboard::{Key, key::Named},
    mouse,
    widget::{
        Space, button,
        canvas::{self, Canvas, Frame, Geometry, Path, Text},
        column, container, row, text,
    },
};
use icy_engine::BitFont;
use icy_engine_gui::ui::{DIALOG_SPACING, Dialog, DialogAction, dialog_area, modal_container, primary_button, secondary_button, separator};
use icy_engine_gui::{ButtonType, ScrollbarOverlay, Viewport};
use icy_view_gui::ui::focus;

use crate::ui::Message;

// ============================================================================
// Constants
// ============================================================================

const DIALOG_WIDTH: f32 = 500.0;
const DIALOG_HEIGHT: f32 = 480.0;
const SLOT_ITEM_HEIGHT: f32 = 26.0;

// ============================================================================
// Dialog Messages
// ============================================================================

/// Messages for the Font Slot Manager dialog
#[derive(Debug, Clone)]
pub enum FontSlotManagerMessage {
    /// Select a slot
    SelectSlot(usize),

    /// Navigate up
    NavigateUp,
    /// Navigate down
    NavigateDown,
    /// Navigate to first slot
    NavigateHome,
    /// Navigate to last slot
    NavigateEnd,

    /// Open font selector for current slot
    SetFont,

    /// Reset slot to ANSI default
    ResetSlot,

    /// Add a new custom slot
    AddSlot,

    /// Remove custom slot (only for slots > 42)
    RemoveSlot,

    /// Apply selection (select this slot as active)
    Apply,

    /// Cancel dialog
    Cancel,
}

// ============================================================================
// Dialog Result
// ============================================================================

/// Result of the Font Slot Manager dialog
#[derive(Debug, Clone)]
pub enum FontSlotManagerResult {
    /// User selected a slot as active font slot
    SelectSlot { slot: usize },
    /// User wants to set font for a slot - open font selector
    OpenFontSelector { slot: usize },
    /// Slot was reset to default
    ResetSlot { slot: usize, font: Option<BitFont> },
    /// Slot was removed
    RemoveSlot { slot: usize },
    /// New slot was added with font
    AddSlot { slot: usize, font: BitFont },
}

// ============================================================================
// Dialog State
// ============================================================================

/// State for the Font Slot Manager dialog
pub struct FontSlotManagerDialog {
    /// Current font height (8, 14, or 16)
    font_height: u8,

    /// All slots (0-42 always, plus custom)
    slots: Vec<usize>,

    /// Fonts in each slot (None = empty/unavailable for this height)
    slot_fonts: HashMap<usize, Option<BitFont>>,

    /// Currently selected slot
    active_slot: usize,

    /// Viewport for slot list scrolling
    viewport: RefCell<Viewport>,
}

impl FontSlotManagerDialog {
    /// Create a new Font Slot Manager dialog
    pub fn new(state: &icy_engine_edit::EditState) -> Self {
        let buffer = state.get_buffer();
        let current_font_page = state.get_caret().font_page();

        // Determine font height from current font
        let font_height = buffer.font(current_font_page).map(|f| f.size().height as u8).unwrap_or(16);

        // Build slot list: 0-42 (ANSI) + any custom slots from document
        let mut slots: Vec<usize> = (0..icy_engine::ANSI_FONTS).collect();

        // Add custom slots from document (> 42)
        for (slot, _) in buffer.font_iter() {
            if *slot >= icy_engine::ANSI_FONTS && !slots.contains(slot) {
                slots.push(*slot);
            }
        }
        slots.sort();

        // Build font map
        let mut slot_fonts: HashMap<usize, Option<BitFont>> = HashMap::new();

        // Initialize ANSI slots with defaults
        for slot in 0..icy_engine::ANSI_FONTS {
            let default_font = BitFont::from_ansi_font_page(slot, font_height).cloned();
            slot_fonts.insert(slot, default_font);
        }

        // Override with document fonts
        for (slot, font) in buffer.font_iter() {
            slot_fonts.insert(*slot, Some(font.clone()));
        }

        // Calculate content height
        let content_height = slots.len() as f32 * SLOT_ITEM_HEIGHT;

        Self {
            font_height,
            slots,
            slot_fonts,
            active_slot: current_font_page,
            viewport: RefCell::new(Viewport {
                content_height,
                ..Default::default()
            }),
        }
    }

    /// Get the currently selected slot's font
    fn current_font(&self) -> Option<&BitFont> {
        self.slot_fonts.get(&self.active_slot).and_then(|f| f.as_ref())
    }

    /// Check if current slot can be reset (is not already default)
    fn can_reset(&self) -> bool {
        if self.active_slot >= icy_engine::ANSI_FONTS {
            return false; // Custom slots can't be reset
        }

        if let Some(Some(current)) = self.slot_fonts.get(&self.active_slot) {
            if let Some(default) = BitFont::from_ansi_font_page(self.active_slot, self.font_height) {
                return current.name() != default.name();
            }
        }
        false
    }

    /// Check if current slot can be removed (custom slot > 42)
    fn can_remove(&self) -> bool {
        self.active_slot >= icy_engine::ANSI_FONTS
    }

    /// Navigate to previous slot
    fn select_prev_slot(&mut self) {
        if let Some(idx) = self.slots.iter().position(|&s| s == self.active_slot) {
            if idx > 0 {
                self.active_slot = self.slots[idx - 1];
                self.scroll_to_active();
            }
        }
    }

    /// Navigate to next slot
    fn select_next_slot(&mut self) {
        if let Some(idx) = self.slots.iter().position(|&s| s == self.active_slot) {
            if idx + 1 < self.slots.len() {
                self.active_slot = self.slots[idx + 1];
                self.scroll_to_active();
            }
        }
    }

    /// Scroll to make active slot visible
    fn scroll_to_active(&self) {
        if let Some(idx) = self.slots.iter().position(|&s| s == self.active_slot) {
            let y = idx as f32 * SLOT_ITEM_HEIGHT;
            let mut vp = self.viewport.borrow_mut();
            let visible_height = vp.visible_height;

            if y < vp.scroll_y {
                vp.scroll_y = y;
                vp.target_scroll_y = y;
                vp.sync_scrollbar_position();
            } else if y + SLOT_ITEM_HEIGHT > vp.scroll_y + visible_height {
                let new_scroll = y + SLOT_ITEM_HEIGHT - visible_height;
                vp.scroll_y = new_scroll;
                vp.target_scroll_y = new_scroll;
                vp.sync_scrollbar_position();
            }
        }
    }

    /// Find next available custom slot number
    fn find_next_custom_slot(&self) -> usize {
        for slot in icy_engine::ANSI_FONTS..=255 {
            if !self.slots.contains(&slot) {
                return slot;
            }
        }
        icy_engine::ANSI_FONTS
    }

    // ========================================================================
    // View Methods
    // ========================================================================

    fn view_dialog(&self) -> Element<'_, Message> {
        let title = text("Font Slot Manager").size(16);

        let slot_list = self.view_slot_list();
        let buttons_panel = self.view_buttons();
        let info_panel = self.view_info();

        let content = row![
            container(slot_list).width(Length::Fixed(280.0)).height(Length::Fixed(DIALOG_HEIGHT - 120.0)),
            Space::new().width(16.0),
            column![info_panel, Space::new().height(Length::Fill), buttons_panel,]
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .spacing(0);

        let button_row = row![
            Space::new().width(Length::Fill),
            secondary_button(
                format!("{}", ButtonType::Cancel),
                Some(Message::FontSlotManager(FontSlotManagerMessage::Cancel))
            ),
            primary_button(format!("{}", ButtonType::Ok), Some(Message::FontSlotManager(FontSlotManagerMessage::Apply))),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let dialog_content = dialog_area(column![title, Space::new().height(12.0), content,].into());

        let button_area = dialog_area(button_row.into());

        let dialog_column = column![container(dialog_content).height(Length::Shrink), separator(), button_area];

        modal_container(dialog_column.into(), DIALOG_WIDTH).into()
    }

    fn view_slot_list(&self) -> Element<'_, Message> {
        // Canvas-based slot list with overlay scrollbar
        let slot_canvas: Element<'_, Message> = Canvas::new(SlotListCanvas { dialog: self }).width(Length::Fill).height(Length::Fill).into();

        let scrollbar: Element<'_, Message> = ScrollbarOverlay::new(&self.viewport)
            .view()
            .map(|_| Message::FontSlotManager(FontSlotManagerMessage::Cancel));

        let list_row = row![slot_canvas, scrollbar,];

        // Wrap in Focus widget for keyboard navigation
        focus(list_row)
            .on_event(|event, _id| {
                if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                    match key {
                        Key::Named(Named::ArrowUp) => Some(Message::FontSlotManager(FontSlotManagerMessage::NavigateUp)),
                        Key::Named(Named::ArrowDown) => Some(Message::FontSlotManager(FontSlotManagerMessage::NavigateDown)),
                        Key::Named(Named::Home) => Some(Message::FontSlotManager(FontSlotManagerMessage::NavigateHome)),
                        Key::Named(Named::End) => Some(Message::FontSlotManager(FontSlotManagerMessage::NavigateEnd)),
                        Key::Named(Named::Enter) => Some(Message::FontSlotManager(FontSlotManagerMessage::SetFont)),
                        Key::Named(Named::Delete) => Some(Message::FontSlotManager(FontSlotManagerMessage::ResetSlot)),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .into()
    }

    fn view_info(&self) -> Element<'_, Message> {
        let slot_label = format!("Slot {}", self.active_slot);

        let font_info = if let Some(font) = self.current_font() {
            column![
                text(font.name()).size(14),
                Space::new().height(4.0),
                text(format!("{}×{} px", font.size().width, font.size().height))
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
        } else {
            column![
                text("(leer)").size(14).color(Color::from_rgb(0.5, 0.5, 0.5)),
                Space::new().height(4.0),
                text("Kein Font für diese Höhe").size(12).color(Color::from_rgb(0.4, 0.4, 0.4)),
            ]
        };

        let slot_type = if self.active_slot < icy_engine::ANSI_FONTS {
            text("ANSI Slot").size(11).color(Color::from_rgb(0.5, 0.5, 0.5))
        } else {
            text("Custom Slot").size(11).color(Color::from_rgb(0.7, 0.6, 0.3))
        };

        column![text(slot_label).size(16), slot_type, Space::new().height(12.0), font_info,].into()
    }

    fn view_buttons(&self) -> Element<'_, Message> {
        let set_font_btn = button(text("Font setzen...").size(12))
            .width(Length::Fill)
            .padding([8, 16])
            .on_press(Message::FontSlotManager(FontSlotManagerMessage::SetFont))
            .style(button::primary);

        let reset_btn = button(text("Zurücksetzen").size(12))
            .width(Length::Fill)
            .padding([8, 16])
            .on_press_maybe(self.can_reset().then_some(Message::FontSlotManager(FontSlotManagerMessage::ResetSlot)))
            .style(button::secondary);

        let remove_btn = button(text("Slot entfernen").size(12))
            .width(Length::Fill)
            .padding([8, 16])
            .on_press_maybe(self.can_remove().then_some(Message::FontSlotManager(FontSlotManagerMessage::RemoveSlot)))
            .style(button::danger);

        let add_btn = button(text("Neuer Slot...").size(12))
            .width(Length::Fill)
            .padding([8, 16])
            .on_press(Message::FontSlotManager(FontSlotManagerMessage::AddSlot))
            .style(button::secondary);

        column![
            set_font_btn,
            Space::new().height(8.0),
            reset_btn,
            Space::new().height(8.0),
            remove_btn,
            Space::new().height(16.0),
            add_btn,
        ]
        .width(Length::Fill)
        .into()
    }
}

// ============================================================================
// Slot List Canvas
// ============================================================================

struct SlotListCanvas<'a> {
    dialog: &'a FontSlotManagerDialog,
}

impl<'a> canvas::Program<Message> for SlotListCanvas<'a> {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let palette = theme.extended_palette();

        // Update viewport dimensions
        {
            let mut vp = self.dialog.viewport.borrow_mut();
            vp.visible_height = bounds.height;
            vp.visible_width = bounds.width;
            vp.content_height = self.dialog.slots.len() as f32 * SLOT_ITEM_HEIGHT;
        }

        let scroll_y = self.dialog.viewport.borrow().scroll_y;

        let geometry = iced::widget::canvas::Cache::new().draw(renderer, bounds.size(), |frame: &mut Frame| {
            let mut y = -scroll_y;

            for &slot in &self.dialog.slots {
                if y + SLOT_ITEM_HEIGHT < 0.0 {
                    y += SLOT_ITEM_HEIGHT;
                    continue;
                }
                if y > bounds.height {
                    break;
                }

                let is_active = slot == self.dialog.active_slot;

                // Get font info
                let (font_name, is_empty, is_custom) = match self.dialog.slot_fonts.get(&slot) {
                    Some(Some(font)) => {
                        let is_ansi_default = if slot < icy_engine::ANSI_FONTS {
                            BitFont::from_ansi_font_page(slot, self.dialog.font_height)
                                .map(|default| default.name() == font.name())
                                .unwrap_or(false)
                        } else {
                            false
                        };
                        (font.name().to_string(), false, !is_ansi_default)
                    }
                    Some(None) | None => ("(leer)".to_string(), true, false),
                };

                // Background
                let bg_color = if is_active {
                    palette.primary.base.color
                } else if is_empty {
                    Color::from_rgb(0.08, 0.08, 0.08)
                } else if is_custom {
                    Color::from_rgb(0.12, 0.10, 0.06)
                } else {
                    Color::TRANSPARENT
                };

                if bg_color != Color::TRANSPARENT {
                    frame.fill(&Path::rectangle(Point::new(0.0, y), Size::new(bounds.width, SLOT_ITEM_HEIGHT)), bg_color);
                }

                // Slot number
                let slot_color = if is_active { Color::WHITE } else { Color::from_rgb(0.5, 0.5, 0.5) };
                frame.fill_text(Text {
                    content: format!("[{:2}]", slot),
                    position: Point::new(8.0, y + (SLOT_ITEM_HEIGHT - 13.0) / 2.0),
                    color: slot_color,
                    size: iced::Pixels(13.0),
                    ..Default::default()
                });

                // Font name
                let name_color = if is_active {
                    Color::WHITE
                } else if is_empty {
                    Color::from_rgb(0.4, 0.4, 0.4)
                } else if is_custom {
                    Color::from_rgb(0.9, 0.75, 0.4)
                } else {
                    Color::from_rgb(0.7, 0.7, 0.7)
                };

                frame.fill_text(Text {
                    content: font_name,
                    position: Point::new(52.0, y + (SLOT_ITEM_HEIGHT - 13.0) / 2.0),
                    color: name_color,
                    size: iced::Pixels(13.0),
                    ..Default::default()
                });

                y += SLOT_ITEM_HEIGHT;
            }
        });

        vec![geometry]
    }

    fn update(&self, _state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let scroll_y = self.dialog.viewport.borrow().scroll_y;
                    let click_y = pos.y + scroll_y;
                    let slot_idx = (click_y / SLOT_ITEM_HEIGHT) as usize;

                    if slot_idx < self.dialog.slots.len() {
                        let slot = self.dialog.slots[slot_idx];
                        return Some(canvas::Action::publish(Message::FontSlotManager(FontSlotManagerMessage::SelectSlot(slot))));
                    }
                }
            }
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    let scroll_amount = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => -y * 26.0,
                        mouse::ScrollDelta::Pixels { y, .. } => -y,
                    };

                    let mut vp = self.dialog.viewport.borrow_mut();
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

impl Dialog<Message> for FontSlotManagerDialog {
    fn view(&self) -> Element<'_, Message> {
        self.view_dialog()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::FontSlotManager(msg) = message else {
            return None;
        };

        match msg {
            FontSlotManagerMessage::SelectSlot(slot) => {
                self.active_slot = *slot;
                self.scroll_to_active();
                Some(DialogAction::None)
            }
            FontSlotManagerMessage::NavigateUp => {
                self.select_prev_slot();
                Some(DialogAction::None)
            }
            FontSlotManagerMessage::NavigateDown => {
                self.select_next_slot();
                Some(DialogAction::None)
            }
            FontSlotManagerMessage::NavigateHome => {
                if let Some(&first) = self.slots.first() {
                    self.active_slot = first;
                    self.scroll_to_active();
                }
                Some(DialogAction::None)
            }
            FontSlotManagerMessage::NavigateEnd => {
                if let Some(&last) = self.slots.last() {
                    self.active_slot = last;
                    self.scroll_to_active();
                }
                Some(DialogAction::None)
            }
            FontSlotManagerMessage::SetFont => {
                // Open font selector for active slot (keep dialog open)
                Some(DialogAction::SendMessage(Message::OpenFontSelectorForSlot(self.active_slot)))
            }
            FontSlotManagerMessage::ResetSlot => {
                if self.can_reset() {
                    let default_font = BitFont::from_ansi_font_page(self.active_slot, self.font_height).cloned();
                    self.slot_fonts.insert(self.active_slot, default_font.clone());
                    Some(DialogAction::SendMessage(Message::ApplyFontSlotChange(FontSlotManagerResult::ResetSlot {
                        slot: self.active_slot,
                        font: default_font,
                    })))
                } else {
                    Some(DialogAction::None)
                }
            }
            FontSlotManagerMessage::RemoveSlot => {
                if self.can_remove() {
                    // Remove the slot from the dialog's internal state as well
                    if let Some(pos) = self.slots.iter().position(|&s| s == self.active_slot) {
                        let removed_slot = self.active_slot;
                        self.slots.remove(pos);
                        self.slot_fonts.remove(&removed_slot);

                        // Select adjacent slot
                        if !self.slots.is_empty() {
                            self.active_slot = self.slots[pos.min(self.slots.len() - 1)];
                        }

                        // Update content height
                        self.viewport.borrow_mut().content_height = self.slots.len() as f32 * SLOT_ITEM_HEIGHT;

                        return Some(DialogAction::SendMessage(Message::ApplyFontSlotChange(FontSlotManagerResult::RemoveSlot {
                            slot: removed_slot,
                        })));
                    }
                }
                Some(DialogAction::None)
            }
            FontSlotManagerMessage::AddSlot => {
                let new_slot = self.find_next_custom_slot();
                // Open font selector for new slot (keep dialog open)
                Some(DialogAction::SendMessage(Message::OpenFontSelectorForSlot(new_slot)))
            }
            FontSlotManagerMessage::Apply => {
                // Select this slot as the active font slot
                Some(DialogAction::CloseWith(Message::ApplyFontSlotChange(FontSlotManagerResult::SelectSlot {
                    slot: self.active_slot,
                })))
            }
            FontSlotManagerMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn handle_event(&mut self, _event: &iced::Event) -> Option<DialogAction<Message>> {
        None
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        // Confirm = Apply = select this slot
        DialogAction::CloseWith(Message::ApplyFontSlotChange(FontSlotManagerResult::SelectSlot { slot: self.active_slot }))
    }
}
