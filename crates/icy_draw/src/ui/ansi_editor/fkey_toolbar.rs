//! F-Key Toolbar Canvas Component
//!
//! Renders F1-F12 function key slots with characters from the current font.
//! Uses the caret's foreground/background colors and the current font for rendering.

use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse::{self, Cursor},
    widget::{
        Action,
        canvas::{self, Cache, Canvas, Frame, Geometry},
    },
};
use icy_engine::{BitFont, Palette};

use crate::ui::FKeySets;

/// Character display height (32px = 2x font height)
const CHAR_DISPLAY_HEIGHT: f32 = 32.0;

/// Width per F-key slot (label + char)
const SLOT_WIDTH: f32 = 40.0;

/// Label width (01, 02, etc. - 2 chars)
const LABEL_WIDTH: f32 = 20.0;

/// Spacing between slots
const SLOT_SPACING: f32 = 4.0;

/// Nav button size
const NAV_SIZE: f32 = 20.0;

/// Gap before nav section
const NAV_GAP: f32 = 10.0;

/// Messages from the F-key toolbar
#[derive(Clone, Debug)]
pub enum FKeyToolbarMessage {
    /// Click on F-key slot to type character
    TypeFKey(usize),
    /// Click on F-key label to open character selector popup
    OpenCharSelector(usize),
    /// Navigate to previous F-key set
    PrevSet,
    /// Navigate to next F-key set
    NextSet,
}

/// F-key toolbar with render cache
pub struct FKeyToolbar {
    cache: Cache,
}

impl Default for FKeyToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl FKeyToolbar {
    pub fn new() -> Self {
        Self { cache: Cache::new() }
    }

    /// Clear the render cache (forces redraw on next frame)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Render the toolbar (takes owned data to avoid lifetime issues)
    pub fn view(&self, fkeys: FKeySets, font: Option<BitFont>, palette: Palette, fg_color: u32, bg_color: u32) -> Element<'_, FKeyToolbarMessage> {
        Canvas::new(FKeyToolbarProgram {
            fkeys,
            font,
            palette,
            fg_color,
            bg_color,
            cache: &self.cache,
        })
        .width(Length::Fill)
        .height(Length::Fill) // Height is controlled by parent container
        .into()
    }
}

/// Canvas program for drawing the F-key toolbar
struct FKeyToolbarProgram<'a> {
    fkeys: FKeySets,
    font: Option<BitFont>,
    palette: Palette,
    fg_color: u32,
    bg_color: u32,
    cache: &'a Cache,
}

impl<'a> FKeyToolbarProgram<'a> {
    /// Calculate content layout
    fn layout(&self, bounds_width: f32) -> FKeyLayout {
        let content_width = 12.0 * SLOT_WIDTH + 11.0 * SLOT_SPACING + NAV_GAP + NAV_SIZE * 2.0 + 32.0;
        let start_x = (bounds_width - content_width) / 2.0;
        FKeyLayout { start_x }
    }

    /// Get slot index at cursor position, returns (slot, is_on_char)
    fn slot_at(&self, cursor_pos: Point, bounds: Rectangle) -> Option<(usize, bool)> {
        let layout = self.layout(bounds.width);

        for slot in 0..12usize {
            let slot_x = layout.start_x + slot as f32 * (SLOT_WIDTH + SLOT_SPACING);
            let char_x = slot_x + LABEL_WIDTH;

            if cursor_pos.x >= slot_x && cursor_pos.x < slot_x + SLOT_WIDTH && cursor_pos.y >= 0.0 && cursor_pos.y < bounds.height {
                let is_on_char = cursor_pos.x >= char_x;
                return Some((slot, is_on_char));
            }
        }
        None
    }

    /// Draw a single glyph from the font
    fn draw_glyph(&self, frame: &mut Frame, x: f32, y: f32, ch: char, fg: Color, bg: Color, scale: f32) {
        // Floor coordinates for crisp pixel-perfect rendering
        let x = x.floor();
        let y = y.floor();

        let Some(font) = &self.font else {
            // Fallback: draw a placeholder rectangle
            frame.fill_rectangle(Point::new(x, y), Size::new(8.0 * scale, 16.0 * scale), bg);
            return;
        };

        let font_width = font.size().width as f32;
        let font_height = font.size().height as f32;
        let char_width = (font_width * scale).floor();
        let char_height = (font_height * scale).floor();
        let pixel_w = scale.floor().max(1.0);
        let pixel_h = scale.floor().max(1.0);

        // Fill background
        frame.fill_rectangle(Point::new(x, y), Size::new(char_width, char_height), bg);

        // Get glyph and draw pixels
        if let Some(glyph) = font.glyph(ch) {
            for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
                for (col_idx, &pixel) in row.iter().enumerate() {
                    if pixel {
                        frame.fill_rectangle(
                            Point::new(x + (col_idx as f32 * pixel_w).floor(), y + (row_idx as f32 * pixel_h).floor()),
                            Size::new(pixel_w, pixel_h),
                            fg,
                        );
                    }
                }
            }
        }
    }

    /// Computes a Y offset so the *ink* (set pixels) of the glyph is vertically centered
    /// within the glyph box at the given scale.
    fn glyph_content_y_offset(&self, ch: char, scale: f32) -> f32 {
        let Some(font) = &self.font else {
            return 0.0;
        };
        let Some(glyph) = font.glyph(ch) else {
            return 0.0;
        };

        let font_height = font.size().height as f32;
        let char_height = (font_height * scale).floor();
        let pixel_h = scale.floor().max(1.0);

        let mut min_row: Option<usize> = None;
        let mut max_row: Option<usize> = None;

        for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
            if row.iter().any(|&p| p) {
                min_row = Some(min_row.map_or(row_idx, |m| m.min(row_idx)));
                max_row = Some(max_row.map_or(row_idx, |m| m.max(row_idx)));
            }
        }

        let (Some(min_row), Some(max_row)) = (min_row, max_row) else {
            return 0.0;
        };

        let used_height = ((max_row - min_row + 1) as f32) * pixel_h;
        let desired_top = ((char_height - used_height) / 2.0).floor();
        let current_top = (min_row as f32) * pixel_h;
        (desired_top - current_top).floor()
    }

    /// Draw F-key label using the font (01, 02, etc.)
    fn draw_label(&self, frame: &mut Frame, x: f32, y: f32, slot: usize, color: Color, scale: f32, char_w: f32) {
        let num = slot + 1;
        let label_chars: Vec<char> = if num < 10 {
            vec!['0', char::from_digit(num as u32, 10).unwrap_or('?')]
        } else if num == 10 {
            vec!['1', '0']
        } else if num == 11 {
            vec!['1', '1']
        } else {
            vec!['1', '2']
        };

        let label_scale = scale * 0.6;

        let bg_transparent = Color::TRANSPARENT;

        for (i, ch) in label_chars.iter().enumerate() {
            let glyph_y_offset = self.glyph_content_y_offset(*ch, label_scale);
            self.draw_glyph(frame, x + i as f32 * char_w, y + glyph_y_offset, *ch, color, bg_transparent, label_scale);
        }
    }

    /// Draw navigation arrow
    fn draw_nav_arrow(&self, frame: &mut Frame, x: f32, y: f32, pointing_left: bool, color: Color) {
        let size = NAV_SIZE;
        let center_x = x + size / 2.0;
        let center_y = y + size / 2.0;
        let arrow_size = size * 0.4;

        use iced::widget::canvas::{Path, Stroke};

        let path = if pointing_left {
            Path::new(|builder| {
                builder.move_to(Point::new(center_x + arrow_size / 2.0, center_y - arrow_size));
                builder.line_to(Point::new(center_x - arrow_size / 2.0, center_y));
                builder.line_to(Point::new(center_x + arrow_size / 2.0, center_y + arrow_size));
            })
        } else {
            Path::new(|builder| {
                builder.move_to(Point::new(center_x - arrow_size / 2.0, center_y - arrow_size));
                builder.line_to(Point::new(center_x + arrow_size / 2.0, center_y));
                builder.line_to(Point::new(center_x - arrow_size / 2.0, center_y + arrow_size));
            })
        };

        frame.stroke(&path, Stroke::default().with_color(color).with_width(2.0));
    }

    /// Draw set number using the font
    fn draw_set_number(&self, frame: &mut Frame, x: f32, y: f32, set_num: usize, color: Color, scale: f32, char_w: f32) {
        let num_str = format!("{}", set_num);
        let num_scale = scale * 0.6; // Same scale as labels for consistency
        let bg_transparent = Color::TRANSPARENT;

        for (i, ch) in num_str.chars().enumerate() {
            let glyph_y_offset = self.glyph_content_y_offset(ch, num_scale);
            self.draw_glyph(frame, x + i as f32 * char_w, y + glyph_y_offset, ch, color, bg_transparent, num_scale);
        }
    }
}

struct FKeyLayout {
    start_x: f32,
}

/// Hover state: which element is currently hovered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HoverState {
    #[default]
    None,
    /// Slot hover (slot_index, is_on_char)
    Slot(usize, bool),
    /// Hover over previous-set navigation arrow
    NavPrev,
    /// Hover over next-set navigation arrow
    NavNext,
}

impl canvas::Program<FKeyToolbarMessage> for FKeyToolbarProgram<'_> {
    type State = HoverState;

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        // Use theme colors instead of hardcoded values
        let theme_palette = theme.extended_palette();
        let label_color = theme_palette.secondary.base.color;
        let label_hover_color = theme_palette.background.base.text;
        let char_hover_bg = Color {
            a: 0.1,
            ..theme_palette.background.base.color
        };
        let nav_hover_color = theme_palette.background.base.text;

        // Cache the geometry since it only changes on hover or data change
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            // Colors from palette
            let (fg_r, fg_g, fg_b) = self.palette.rgb(self.fg_color);
            let (bg_r, bg_g, bg_b) = self.palette.rgb(self.bg_color);
            let fg = Color::from_rgb8(fg_r, fg_g, fg_b);
            let bg = Color::from_rgb8(bg_r, bg_g, bg_b);

            let set_idx = self.fkeys.current_set();
            let hovered = *state;

            // Calculate font scale to fit CHAR_DISPLAY_HEIGHT
            let font_height = self.font.as_ref().map(|f| f.size().height as f32).unwrap_or(16.0);
            let scale = CHAR_DISPLAY_HEIGHT / font_height;
            let font_width = self.font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0);

            // Shared char_w for labels and set number
            let label_char_w = font_width * scale * 0.6;

            let layout = self.layout(bounds.width);

            // Center char display vertically (floor for crisp rendering)
            let char_display_y = ((bounds.height - CHAR_DISPLAY_HEIGHT) / 2.0).floor();

            // Center labels vertically (they are smaller: scale * 0.6)
            let label_height = font_height * scale * 0.6;
            let label_y = ((bounds.height - label_height) / 2.0).floor();

            // Draw each F-key slot
            for slot in 0..12usize {
                let slot_x = (layout.start_x + slot as f32 * (SLOT_WIDTH + SLOT_SPACING)).floor();
                let char_x = (slot_x + LABEL_WIDTH).floor();
                let label_x = (slot_x - 2.0).floor(); // Shift labels 2px left for more spacing

                let is_label_hovered = matches!(hovered, HoverState::Slot(s, false) if s == slot);
                let is_char_hovered = matches!(hovered, HoverState::Slot(s, true) if s == slot);

                // Get character code
                let code = self.fkeys.code_at(set_idx, slot);
                let ch = char::from_u32(code as u32).unwrap_or(' ');

                // Draw label (01, 02, etc.) - use theme color
                let current_label_color = if is_label_hovered { label_hover_color } else { label_color };
                self.draw_label(frame, label_x, label_y, slot, current_label_color, scale, label_char_w);

                // Draw character with hover effect
                let char_bg = if is_char_hovered { char_hover_bg } else { bg };
                self.draw_glyph(frame, char_x, char_display_y, ch, fg, char_bg, scale);
            }

            // Draw navigation section
            let nav_x = (layout.start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) + NAV_GAP).floor();
            let nav_y = ((bounds.height - NAV_SIZE) / 2.0).floor();

            // Prev arrow (with hover effect)
            let is_prev_hovered = matches!(hovered, HoverState::NavPrev);
            let prev_color = if is_prev_hovered { nav_hover_color } else { label_color };
            self.draw_nav_arrow(frame, nav_x, nav_y, true, prev_color);

            // Set number - centered between arrows (uses same y as labels)
            let set_num = set_idx + 1;
            let num_str = format!("{}", set_num);
            let num_width = num_str.len() as f32 * label_char_w;

            // Space between arrows: from nav_x + NAV_SIZE to next_x
            let next_x = nav_x + NAV_SIZE + 28.0;
            let space_between = next_x - (nav_x + NAV_SIZE);
            let num_x = nav_x + NAV_SIZE + (space_between - num_width) / 2.0;

            self.draw_set_number(frame, num_x, label_y, set_num, label_color, scale, label_char_w);

            // Next arrow (with hover effect)
            let is_next_hovered = matches!(hovered, HoverState::NavNext);
            let next_color = if is_next_hovered { nav_hover_color } else { label_color };
            self.draw_nav_arrow(frame, next_x, nav_y, false, next_color);
        });

        vec![geometry]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<FKeyToolbarMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = if let Some(pos) = cursor.position_in(bounds) {
                    // Check slots first
                    if let Some((slot, is_on_char)) = self.slot_at(pos, bounds) {
                        HoverState::Slot(slot, is_on_char)
                    } else {
                        // Check nav buttons
                        let layout = self.layout(bounds.width);
                        let nav_x = layout.start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) + NAV_GAP;
                        let nav_y = (bounds.height - NAV_SIZE) / 2.0;
                        let next_x = nav_x + NAV_SIZE + 28.0;

                        if pos.x >= nav_x && pos.x < nav_x + NAV_SIZE && pos.y >= nav_y && pos.y < nav_y + NAV_SIZE {
                            HoverState::NavPrev
                        } else if pos.x >= next_x && pos.x < next_x + NAV_SIZE && pos.y >= nav_y && pos.y < nav_y + NAV_SIZE {
                            HoverState::NavNext
                        } else {
                            HoverState::None
                        }
                    }
                } else {
                    HoverState::None
                };

                if *state != new_hover {
                    *state = new_hover;
                    self.cache.clear(); // Clear cache when hover changes
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };

                // Check F-key slots - different action for label vs char area
                if let Some((slot, is_on_char)) = self.slot_at(cursor_pos, bounds) {
                    if is_on_char {
                        // Click on char area: type the character
                        return Some(Action::publish(FKeyToolbarMessage::TypeFKey(slot)));
                    } else {
                        // Click on label area: open character selector popup
                        return Some(Action::publish(FKeyToolbarMessage::OpenCharSelector(slot)));
                    }
                }

                // Check navigation buttons
                let layout = self.layout(bounds.width);
                let nav_x = layout.start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) + NAV_GAP;
                let nav_y = (bounds.height - NAV_SIZE) / 2.0;

                // Prev button
                if cursor_pos.x >= nav_x && cursor_pos.x < nav_x + NAV_SIZE && cursor_pos.y >= nav_y && cursor_pos.y < nav_y + NAV_SIZE {
                    return Some(Action::publish(FKeyToolbarMessage::PrevSet));
                }

                // Next button
                let next_x = nav_x + NAV_SIZE + 28.0;
                if cursor_pos.x >= next_x && cursor_pos.x < next_x + NAV_SIZE && cursor_pos.y >= nav_y && cursor_pos.y < nav_y + NAV_SIZE {
                    return Some(Action::publish(FKeyToolbarMessage::NextSet));
                }

                None
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                if *state != HoverState::None {
                    *state = HoverState::None;
                    self.cache.clear(); // Clear cache when hover changes
                    return Some(Action::request_redraw());
                }
                None
            }
            _ => None,
        }
    }
}
