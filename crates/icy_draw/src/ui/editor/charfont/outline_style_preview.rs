//! Outline Style Selector Widget (egui-like)
//!
//! Shows the 19 TheDraw outline styles as an 8×6 preview pattern per style.
//! Used by the CharFont (TDF) editor outline preview panel.

use codepages::tables::UNICODE_TO_CP437;
use iced::{
    mouse::{self, Cursor},
    widget::{
        canvas::{self, Canvas, Frame, Geometry},
        Action,
    },
    Color, Element, Length, Point, Rectangle, Size, Theme,
};
use icy_engine::BitFont;

/// Number of available outline styles
pub const OUTLINE_STYLE_COUNT: usize = 19;

/// Preview pattern dimensions (characters)
const OUTLINE_WIDTH: usize = 8;
const OUTLINE_HEIGHT: usize = 6;

/// Preview pattern using TheDraw outline placeholders (A-Q = 65-81)
/// Copied from the ANSI outline selector.
const OUTLINE_FONT_CHAR: [u8; 48] = [
    69, 65, 65, 65, 65, 65, 65, 70, 67, 79, 71, 66, 66, 72, 79, 68, 67, 79, 73, 65, 65, 74, 79, 68, 67, 79, 71, 66, 66, 72, 79, 68, 67, 79, 68, 64, 64, 67, 79,
    68, 75, 66, 76, 64, 64, 75, 66, 76,
];

/// Styles per row in the selector grid (compact)
const PER_ROW: usize = 4;

/// Padding/spacing tuned for the CharFont side panel
const POPUP_PADDING: f32 = 8.0;
const CELL_PADDING: f32 = 4.0;
const CELL_SPACING: f32 = 6.0;

/// Messages emitted by the selector
#[derive(Clone, Debug)]
pub enum OutlineStyleSelectorMessage {
    Select(usize),
}

fn cell_size(font: &BitFont) -> (f32, f32) {
    let font_size = font.size();
    let w = font_size.width as f32 * OUTLINE_WIDTH as f32 + 2.0 * CELL_PADDING;
    let h = font_size.height as f32 * OUTLINE_HEIGHT as f32 + 2.0 * CELL_PADDING;
    (w, h)
}

pub fn selector_width() -> f32 {
    let font = BitFont::default();
    let (cell_w, _) = cell_size(&font);
    PER_ROW as f32 * (cell_w + CELL_SPACING) - CELL_SPACING + 2.0 * POPUP_PADDING
}

pub fn selector_height() -> f32 {
    let font = BitFont::default();
    let (_, cell_h) = cell_size(&font);
    let rows = (OUTLINE_STYLE_COUNT + PER_ROW - 1) / PER_ROW;
    rows as f32 * (cell_h + CELL_SPACING) - CELL_SPACING + 2.0 * POPUP_PADDING
}

pub fn view_style_selector<'a, Message: 'a>(
    current_style: usize,
    map_msg: impl Fn(OutlineStyleSelectorMessage) -> Message + Copy + 'a,
) -> Element<'a, Message> {
    let selector: Element<'a, OutlineStyleSelectorMessage> = Canvas::new(OutlineStyleSelectorProgram {
        current_style,
        font: BitFont::default(),
    })
    .width(Length::Fixed(selector_width()))
    .height(Length::Fixed(selector_height()))
    .into();

    selector.map(map_msg)
}

struct OutlineStyleSelectorProgram {
    current_style: usize,
    font: BitFont,
}

#[derive(Debug, Clone, Default)]
struct OutlineStyleSelectorState {
    hovered: Option<usize>,
}

impl OutlineStyleSelectorProgram {
    fn cell_rect(&self, style: usize) -> Rectangle {
        let (cell_w, cell_h) = cell_size(&self.font);
        let row = style / PER_ROW;
        let col = style % PER_ROW;
        Rectangle {
            x: POPUP_PADDING + col as f32 * (cell_w + CELL_SPACING),
            y: POPUP_PADDING + row as f32 * (cell_h + CELL_SPACING),
            width: cell_w,
            height: cell_h,
        }
    }

    fn hit_test(&self, p: Point) -> Option<usize> {
        for style in 0..OUTLINE_STYLE_COUNT {
            if self.cell_rect(style).contains(p) {
                return Some(style);
            }
        }
        None
    }

    fn draw_cell(&self, frame: &mut Frame, style: usize, rect: Rectangle, fg: Color, bg: Color) {
        let font_size = self.font.size();
        let font_w = font_size.width as usize;
        let font_h = font_size.height as usize;

        frame.fill_rectangle(Point::new(rect.x, rect.y), rect.size(), bg);

        for row in 0..OUTLINE_HEIGHT {
            for col in 0..OUTLINE_WIDTH {
                let src_char = OUTLINE_FONT_CHAR[col + row * OUTLINE_WIDTH];
                let unicode_ch = retrofont::transform_outline(style, src_char);
                let cp437_ch = if let Some(&cp437) = UNICODE_TO_CP437.get(&unicode_ch) {
                    char::from(cp437)
                } else {
                    unicode_ch
                };

                let glyph = self.font.glyph(cp437_ch);
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
                            let px = rect.x + CELL_PADDING + col as f32 * font_w as f32 + x as f32;
                            let py = rect.y + CELL_PADDING + row as f32 * font_h as f32 + y as f32;
                            frame.fill_rectangle(Point::new(px, py), Size::new(1.0, 1.0), fg);
                        }
                    }
                }
            }
        }
    }
}

impl canvas::Program<OutlineStyleSelectorMessage> for OutlineStyleSelectorProgram {
    type State = OutlineStyleSelectorState;

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let panel_bg = Color::from_rgb8(40, 40, 45);
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), panel_bg);

        let hovered = state.hovered;

        for style in 0..OUTLINE_STYLE_COUNT {
            let rect = self.cell_rect(style);
            let is_selected = style == self.current_style;
            let is_hovered = hovered == Some(style);

            let fg = if is_selected {
                Color::from_rgb8(240, 240, 240)
            } else {
                Color::from_rgb8(210, 210, 210)
            };

            let bg = if is_selected {
                Color::from_rgb8(60, 60, 70)
            } else if is_hovered {
                Color::from_rgb8(55, 55, 60)
            } else {
                Color::from_rgb8(45, 45, 50)
            };

            self.draw_cell(&mut frame, style, rect, fg, bg);
        }

        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<OutlineStyleSelectorMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| self.hit_test(p));
                if state.hovered != new_hover {
                    state.hovered = new_hover;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed { button: mouse::Button::Left, .. }) => {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };

                if let Some(style) = self.hit_test(cursor_pos) {
                    return Some(Action::publish(OutlineStyleSelectorMessage::Select(style)));
                }

                None
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(bounds) {
            if self.hit_test(pos).is_some() || state.hovered.is_some() {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}
