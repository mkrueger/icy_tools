//! Charset (F-Key) settings picker widget.
//!
//! This module provides the charset preview and settings UI for configuring
//! the F1-F12 character sets used for quick character insertion.
//! Features:
//! - Visual 16x16 character grid for easy character selection
//! - 12 F-key slots displayed as clickable buttons with actual glyphs
//! - Set navigation (multiple character sets)

use iced::{
    mouse,
    mouse::Cursor,
    widget::{
        button,
        canvas::{self, Action, Cache, Frame, Geometry, Program},
        column, container, row, text,
    },
    Element, Length, Point, Rectangle, Size, Theme,
};
use icy_engine::BitFont;

use crate::fl;
use crate::ui::FKeySets;

use super::SettingsDialogMessage;

/// Grid dimensions (32x8 = 256 characters)
const GRID_COLS: usize = 32;
const GRID_ROWS: usize = 8;

/// Cell padding and spacing
const CELL_PADDING: f32 = 2.0;
const CELL_SPACING: f32 = 1.0;

/// Scale factor for character rendering (1.5 = 50% larger)
const CHAR_SCALE: f32 = 1.5;

/// Number of F-key slots
pub const FKEY_SLOTS: usize = 12;

/// Spacing between UI elements
const UI_SPACING: f32 = 8.0;

/// Create the character grid picker widget.
pub fn char_grid(font: &BitFont, selected_char: Option<u8>, cursor: u8, is_active: bool) -> Element<'static, SettingsDialogMessage> {
    let program = CharGridProgram {
        font: font.clone(),
        selected_char,
        cursor,
        is_active,
        cache: Cache::default(),
    };

    let font_size = font.size();
    let cell_w = font_size.width as f32 * CHAR_SCALE + 2.0 * CELL_PADDING;
    let cell_h = font_size.height as f32 * CHAR_SCALE + 2.0 * CELL_PADDING;
    let total_width = GRID_COLS as f32 * (cell_w + CELL_SPACING) - CELL_SPACING;
    let total_height = GRID_ROWS as f32 * (cell_h + CELL_SPACING) - CELL_SPACING;

    canvas::Canvas::new(program)
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into()
}

struct CharGridProgram {
    font: BitFont,
    selected_char: Option<u8>,
    cursor: u8,
    is_active: bool,
    cache: Cache,
}

impl CharGridProgram {
    fn cell_size(&self) -> (f32, f32) {
        let font_size = self.font.size();
        let w = font_size.width as f32 * CHAR_SCALE + 2.0 * CELL_PADDING;
        let h = font_size.height as f32 * CHAR_SCALE + 2.0 * CELL_PADDING;
        (w, h)
    }

    fn cell_rect(&self, char_code: u8) -> Rectangle {
        let (cw, ch) = self.cell_size();
        let col = (char_code % 32) as usize;
        let row = (char_code / 32) as usize;

        Rectangle {
            x: col as f32 * (cw + CELL_SPACING),
            y: row as f32 * (ch + CELL_SPACING),
            width: cw,
            height: ch,
        }
    }

    fn hit_test(&self, p: Point) -> Option<u8> {
        let (cw, ch) = self.cell_size();
        let col = (p.x / (cw + CELL_SPACING)) as usize;
        let row = (p.y / (ch + CELL_SPACING)) as usize;

        if col < GRID_COLS && row < GRID_ROWS {
            Some((row * GRID_COLS + col) as u8)
        } else {
            None
        }
    }

    fn draw_char_cell(&self, frame: &mut Frame, char_code: u8, rect: Rectangle, fg: iced::Color, bg: iced::Color) {
        let font_size = self.font.size();
        let font_w = font_size.width as usize;
        let font_h = font_size.height as usize;

        // Fill background
        frame.fill_rectangle(Point::new(rect.x, rect.y), rect.size(), bg);

        // Get the glyph from the font
        let ch = char::from(char_code);
        let glyph = self.font.glyph(ch);
        let bitmap_pixels = glyph.to_bitmap_pixels();
        for (y, glyph_row) in bitmap_pixels.iter().enumerate() {
            if y >= font_h {
                break;
            }
            for (x, &pixel) in glyph_row.iter().enumerate() {
                if x >= font_w {
                    break;
                }
                if pixel {
                    let px = rect.x + CELL_PADDING + x as f32 * CHAR_SCALE;
                    let py = rect.y + CELL_PADDING + y as f32 * CHAR_SCALE;
                    frame.fill_rectangle(Point::new(px, py), Size::new(CHAR_SCALE, CHAR_SCALE), fg);
                }
            }
        }
    }
}

impl Program<SettingsDialogMessage> for CharGridProgram {
    type State = Option<u8>; // Hovered character

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<SettingsDialogMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| self.hit_test(p));
                if *state != new_hover {
                    *state = new_hover;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                if state.is_some() {
                    *state = None;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(p) = cursor.position_in(bounds) else {
                    return None;
                };
                let char_code = self.hit_test(p)?;
                Some(Action::publish(SettingsDialogMessage::SelectCharFromGrid(char_code)))
            }
            _ => None,
        }
    }

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let hovered = *state;

        let geometry = self.cache.draw(renderer, bounds.size(), |frame: &mut Frame| {
            for char_code in 0u8..=255 {
                let rect = self.cell_rect(char_code);
                let is_selected = self.selected_char == Some(char_code);
                let is_cursor = self.is_active && self.cursor == char_code;
                let is_hovered = hovered == Some(char_code) && self.is_active;

                let (bg, fg) = if !self.is_active {
                    // Inactive state: dim colors
                    (palette.background.weak.color, palette.secondary.weak.color)
                } else if is_selected {
                    (palette.primary.strong.color, palette.primary.strong.text)
                } else if is_cursor {
                    (palette.primary.base.color, palette.primary.base.text)
                } else if is_hovered {
                    (palette.primary.weak.color, palette.primary.weak.text)
                } else {
                    (palette.background.weak.color, palette.background.base.text)
                };

                self.draw_char_cell(frame, char_code, rect, fg, bg);

                // Draw border for cursor (dashed style effect via thicker border)
                if is_cursor && self.is_active {
                    let border = canvas::Path::rectangle(Point::new(rect.x, rect.y), rect.size());
                    frame.stroke(&border, canvas::Stroke::default().with_width(2.0).with_color(palette.primary.strong.color));
                }
            }
        });

        vec![geometry]
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(p) = cursor.position_in(bounds) {
            if self.hit_test(p).is_some() {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

/// Create the F-key slots row widget.
/// Shows 12 buttons (F1-F12) with the actual character glyphs.
pub fn fkey_slots(font: &BitFont, fkeys: &FKeySets, selected_slot: Option<usize>) -> Element<'static, SettingsDialogMessage> {
    let program = FKeySlotsProgram {
        font: font.clone(),
        codes: fkeys.current_set_codes(),
        selected_slot,
        cache: Cache::default(),
    };

    let font_size = font.size();
    // Each slot: F-key label area + character cell
    let slot_width = 32.0_f32.max(font_size.width as f32 + 8.0);
    let slot_height = 16.0 + font_size.height as f32 + 8.0; // label + char + padding
    let total_width = FKEY_SLOTS as f32 * (slot_width + 4.0) - 4.0;

    canvas::Canvas::new(program)
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(slot_height))
        .into()
}

struct FKeySlotsProgram {
    font: BitFont,
    codes: [u16; 12],
    selected_slot: Option<usize>,
    cache: Cache,
}

impl FKeySlotsProgram {
    fn slot_size(&self) -> (f32, f32) {
        let font_size = self.font.size();
        let w = 32.0_f32.max(font_size.width as f32 + 8.0);
        let h = 16.0 + font_size.height as f32 + 8.0;
        (w, h)
    }

    fn slot_rect(&self, slot: usize) -> Rectangle {
        let (sw, sh) = self.slot_size();
        Rectangle {
            x: slot as f32 * (sw + 4.0),
            y: 0.0,
            width: sw,
            height: sh,
        }
    }

    fn hit_test(&self, p: Point) -> Option<usize> {
        for slot in 0..FKEY_SLOTS {
            if self.slot_rect(slot).contains(p) {
                return Some(slot);
            }
        }
        None
    }

    fn draw_slot(&self, frame: &mut Frame, slot: usize, rect: Rectangle, fg: iced::Color, bg: iced::Color, label_color: iced::Color) {
        let font_size = self.font.size();
        let font_w = font_size.width as usize;
        let font_h = font_size.height as usize;

        // Fill background
        frame.fill_rectangle(Point::new(rect.x, rect.y), rect.size(), bg);

        // Draw F-key label (F1, F2, etc.)
        let label = format!("F{}", slot + 1);
        let label_text = canvas::Text {
            content: label,
            position: Point::new(rect.x + rect.width / 2.0, rect.y + 2.0),
            color: label_color,
            size: iced::Pixels(10.0),
            align_x: iced::alignment::Horizontal::Center.into(),
            ..Default::default()
        };
        frame.fill_text(label_text);

        // Draw the character glyph
        let char_code = self.codes[slot] as u8;
        let ch = char::from(char_code);
        let glyph = self.font.glyph(ch);
        let char_y = rect.y + 16.0;
        let char_x = rect.x + (rect.width - font_w as f32) / 2.0;

        let bitmap_pixels = glyph.to_bitmap_pixels();
        for (y, glyph_row) in bitmap_pixels.iter().enumerate() {
            if y >= font_h {
                break;
            }
            for (x, &pixel) in glyph_row.iter().enumerate() {
                if x >= font_w {
                    break;
                }
                if pixel {
                    let px = char_x + x as f32;
                    let py = char_y + y as f32;
                    frame.fill_rectangle(Point::new(px, py), Size::new(1.0, 1.0), fg);
                }
            }
        }
    }
}

impl Program<SettingsDialogMessage> for FKeySlotsProgram {
    type State = Option<usize>; // Hovered slot

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<SettingsDialogMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| self.hit_test(p));
                if *state != new_hover {
                    *state = new_hover;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                if state.is_some() {
                    *state = None;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(p) = cursor.position_in(bounds) else {
                    return None;
                };
                let slot = self.hit_test(p)?;
                Some(Action::publish(SettingsDialogMessage::SelectCharsetSlot(slot)))
            }
            _ => None,
        }
    }

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let hovered = *state;

        let geometry = self.cache.draw(renderer, bounds.size(), |frame: &mut Frame| {
            for slot in 0..FKEY_SLOTS {
                let rect = self.slot_rect(slot);
                let is_selected = self.selected_slot == Some(slot);
                let is_hovered = hovered == Some(slot);

                let (bg, fg, label_color) = if is_selected {
                    (palette.primary.strong.color, palette.primary.strong.text, palette.primary.strong.text)
                } else if is_hovered {
                    (palette.primary.weak.color, palette.primary.weak.text, palette.primary.weak.text)
                } else {
                    (palette.background.weak.color, palette.background.base.text, palette.background.strong.color)
                };

                self.draw_slot(frame, slot, rect, fg, bg, label_color);

                // Draw border
                let border_color = if is_selected {
                    palette.primary.base.color
                } else {
                    palette.background.strong.color
                };
                let border = canvas::Path::rectangle(Point::new(rect.x, rect.y), rect.size());
                frame.stroke(
                    &border,
                    canvas::Stroke::default()
                        .with_width(if is_selected { 2.0 } else { 1.0 })
                        .with_color(border_color),
                );
            }
        });

        vec![geometry]
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(p) = cursor.position_in(bounds) {
            if self.hit_test(p).is_some() {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

/// Build the complete charset settings view.
pub fn view_charset<'a>(font: &BitFont, fkeys: &FKeySets, selected_slot: Option<usize>, cursor: u8) -> Element<'a, crate::ui::main_window::Message> {
    let set_idx = fkeys.current_set();
    let set_count = fkeys.set_count();

    // Set navigation (< 1/20 >) - compact style like toolbar
    let set_nav = row![
        button(text("◀").size(icy_engine_gui::ui::TEXT_SIZE_NORMAL))
            .padding([4, 8])
            .style(button::secondary)
            .on_press(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::PrevCharsetSet)),
        text(format!("{}/{}", set_idx + 1, set_count)).size(icy_engine_gui::ui::TEXT_SIZE_NORMAL),
        button(text("▶").size(icy_engine_gui::ui::TEXT_SIZE_NORMAL))
            .padding([4, 8])
            .style(button::secondary)
            .on_press(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::NextCharsetSet)),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    // F-key slots
    let slots_widget = fkey_slots(font, fkeys, selected_slot).map(|msg| crate::ui::main_window::Message::SettingsDialog(msg));

    // Combined row: F-key slots + set navigation
    let slots_row = row![slots_widget, iced::widget::Space::new().width(Length::Fixed(12.0)), set_nav,].align_y(iced::Alignment::Center);

    // Character grid (active only when a slot is selected)
    let is_active = selected_slot.is_some();
    let selected_char = selected_slot.map(|slot| fkeys.code_at(set_idx, slot) as u8);
    let grid_widget = char_grid(font, selected_char, cursor, is_active).map(|msg| crate::ui::main_window::Message::SettingsDialog(msg));

    // Help label for keyboard shortcuts
    let help_text = text("F1-F12: Select slot  |  ←↑↓→: Navigate  |  Space: Assign")
        .size(icy_engine_gui::ui::TEXT_SIZE_SMALL)
        .color(iced::Color::from_rgb(0.5, 0.5, 0.5));

    let inner = column![
        container(slots_row).width(Length::Fill).center_x(Length::Fill),
        iced::widget::Space::new().height(Length::Fixed(4.0)),
        container(grid_widget).width(Length::Fill).center_x(Length::Fill),
        iced::widget::Space::new().height(Length::Fixed(4.0)),
        help_text,
    ]
    .spacing(UI_SPACING)
    .into();

    column![
        icy_engine_gui::ui::section_header(fl!("settings-charset-header")),
        icy_engine_gui::settings::effect_box(inner),
    ]
    .spacing(0)
    .into()
}
