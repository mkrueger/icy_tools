use eframe::egui::{self, RichText};
use egui::{load::SizedTexture, Color32, FontId, Image, Rect, Rounding, Sense, Stroke, TextureHandle, Vec2, Widget};
use i18n_embed_fl::fl;
use icy_engine::{paint::HalfBlock, AttributedChar, Position, TextAttribute, TextPane, TheDrawFont};
use icy_engine_gui::BufferView;

use crate::{create_font_image, create_hover_image, AnsiEditor, Message};

mod rectangle;
pub use rectangle::*;
mod line;
pub use line::*;
mod ellipse;
pub use ellipse::*;

#[derive(Clone, Debug, PartialEq)]
pub enum BrushMode {
    Block,
    HalfBlock,
    Outline,
    Char(std::rc::Rc<std::cell::RefCell<char>>),
    Shade,
    Colorize,
    Custom,
}

static mut HOVER_CHAR: Option<char> = None;
static mut FONT_CHECKSUM: u32 = 0;
static mut FONT_IMG: Option<TextureHandle> = None;
static mut SEL_CHAR: usize = 0;

pub enum BrushUi {
    All,
    HideOutline,
    Brush,
    Fill,
}
impl BrushUi {
    fn has_half_block(&self) -> bool {
        matches!(self, BrushUi::All | BrushUi::HideOutline | BrushUi::Fill)
    }

    fn has_outline(&self) -> bool {
        matches!(self, BrushUi::All)
    }

    fn has_full_block(&self) -> bool {
        matches!(self, BrushUi::All | BrushUi::HideOutline)
    }

    fn has_shade(&self) -> bool {
        !matches!(self, BrushUi::Fill)
    }
}

impl BrushMode {
    pub fn show_ui(
        &mut self,
        ui: &mut egui::Ui,
        editor_opt: Option<&mut AnsiEditor>,
        char_code: std::rc::Rc<std::cell::RefCell<char>>,
        brush_ui: BrushUi,
    ) -> Option<Message> {
        let mut msg = None;
        if brush_ui.has_half_block() {
            ui.radio_value(self, BrushMode::HalfBlock, fl!(crate::LANGUAGE_LOADER, "tool-half-block"));
        }
        if brush_ui.has_full_block() {
            ui.radio_value(self, BrushMode::Block, fl!(crate::LANGUAGE_LOADER, "tool-full-block"));
        }
        if brush_ui.has_outline() {
            ui.horizontal(|ui| {
                ui.radio_value(self, BrushMode::Outline, fl!(crate::LANGUAGE_LOADER, "tool-outline"));
                if ui.button(fl!(crate::LANGUAGE_LOADER, "font_tool_select_outline_button")).clicked() {
                    msg = Some(Message::ShowOutlineDialog);
                }
            });
        }

        if brush_ui.has_shade() {
            ui.radio_value(self, BrushMode::Shade, fl!(crate::LANGUAGE_LOADER, "tool-shade"));
        }

        ui.radio_value(self, BrushMode::Colorize, fl!(crate::LANGUAGE_LOADER, "tool-colorize"));

        ui.horizontal(|ui| {
            ui.radio_value(self, BrushMode::Char(char_code.clone()), fl!(crate::LANGUAGE_LOADER, "tool-character"));
            /*  if let Some(editor) = editor_opt {
                let msg2 = draw_glyph(ui, editor, &char_code);
                if msg.is_none() {
                    msg = msg2;
                }
            }*/
        });

        if let Some(editor) = editor_opt {
            let scale = 2.0;
            let lock = &editor.buffer_view.lock();
            let font_page = lock.get_caret().get_font_page();
            let font_count = lock.get_buffer().font_count();
            if font_count > 1 {
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.add_space(12.0);

                    ui.label(fl!(crate::LANGUAGE_LOADER, "font-view-font_page_label"));
                    if ui.selectable_label(false, RichText::new("◀").font(FontId::proportional(14.))).clicked() {
                        let mut prev = font_page;
                        let mut last = 0;
                        for (page, _) in lock.get_buffer().font_iter() {
                            last = last.max(*page);
                            if *page < font_page {
                                if prev == font_page {
                                    prev = *page;
                                } else {
                                    prev = prev.max(*page);
                                }
                            }
                        }
                        if prev == font_page {
                            msg = Some(Message::SetFontPage(last));
                        } else {
                            msg = Some(Message::SetFontPage(prev));
                        }
                    }
                    ui.label(RichText::new(font_page.to_string()));

                    if ui.selectable_label(false, RichText::new("▶").font(FontId::proportional(14.))).clicked() {
                        let mut next = font_page;
                        let mut first = usize::MAX;
                        for (page, _) in lock.get_buffer().font_iter() {
                            first = first.min(*page);
                            if *page > font_page {
                                if next == font_page {
                                    next = *page;
                                } else {
                                    next = next.min(*page);
                                }
                            }
                        }
                        if next == font_page {
                            msg = Some(Message::SetFontPage(first));
                        } else {
                            msg = Some(Message::SetFontPage(next));
                        }
                    }
                });
            }

            let font = lock.get_buffer().get_font(font_page).unwrap();

            unsafe {
                if FONT_CHECKSUM != font.get_checksum() || FONT_IMG.is_none() || SEL_CHAR != *char_code.borrow() as usize {
                    FONT_CHECKSUM = font.get_checksum();
                    let attr = TextAttribute::new(8, 0);
                    let sel_attr = TextAttribute::new(15, 0);
                    SEL_CHAR = *char_code.borrow() as usize;
                    FONT_IMG = Some(create_font_image(ui.ctx(), font, 16, attr, sel_attr, SEL_CHAR));
                }
            }
            let img_handle = unsafe { FONT_IMG.as_ref().unwrap() };
            let sized_texture: SizedTexture = img_handle.into();
            let image = Image::from_texture(sized_texture).fit_to_original_size(scale).sense(Sense::click());
            let response = image.ui(ui);

            let fw = scale * font.size.width as f32;
            let fh = scale * font.size.height as f32;
            let x = *char_code.borrow() as u32;
            let stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 175));
            let min = response.rect.min;
            let r = Rect::from_min_size(
                min + Vec2::new((fw * (x % 16) as f32).floor() + 0.5, (fh * (x / 16) as f32).floor() + 0.5),
                Vec2::new(fw, fh),
            );
            ui.painter().rect_stroke(r, Rounding::ZERO, stroke);

            unsafe {
                HOVER_CHAR = None;
            }
            if response.hovered() {
                if let Some(pos) = response.hover_pos() {
                    let pos = pos - response.rect.min;
                    let ch = (pos.x / fw) as usize + 16 * (pos.y / fh) as usize;
                    let ch = unsafe { char::from_u32_unchecked(ch as u32) };
                    let hover_char_image = create_hover_image(ui.ctx(), font, ch, 14);

                    let x = (ch as usize) % 16;
                    let y = (ch as usize) / 16;

                    let rect = Rect::from_min_size(response.rect.min + Vec2::new(x as f32 * fw, y as f32 * fh), Vec2::new(fw, fh));
                    let sized_texture: SizedTexture = (&hover_char_image).into();
                    let image = Image::from_texture(sized_texture);
                    image.paint_at(ui, rect.expand(2.0));

                    unsafe {
                        HOVER_CHAR = Some(ch);
                    }
                }
            }
            if response.clicked() {
                unsafe {
                    if let Some(ch) = HOVER_CHAR {
                        *char_code.borrow_mut() = ch;
                        *self = BrushMode::Char(char_code.clone());
                    }
                }
            }

            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "font-view-font_label")).small());
                ui.label(RichText::new(font.name.to_string()).small().color(Color32::WHITE));
            });
        }

        msg
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorMode {
    None,
    UseFg,
    UseBg,
    Both,
}

impl ColorMode {
    pub fn use_fore(&self) -> bool {
        matches!(self, ColorMode::UseFg | ColorMode::Both)
    }

    pub fn use_back(&self) -> bool {
        matches!(self, ColorMode::UseBg | ColorMode::Both)
    }

    pub fn show_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(84.0);
            let mut use_fore = self.use_fore();
            let mut use_back = self.use_back();

            if ui
                .selectable_label(use_fore, RichText::new(fl!(crate::LANGUAGE_LOADER, "tool-fg")).size(20.0))
                .clicked()
            {
                use_fore = !use_fore;
            }
            if ui
                .selectable_label(use_back, RichText::new(fl!(crate::LANGUAGE_LOADER, "tool-bg")).size(20.0))
                .clicked()
            {
                use_back = !use_back;
            }

            if use_fore && use_back {
                *self = ColorMode::Both;
            } else if use_fore {
                *self = ColorMode::UseFg;
            } else if use_back {
                *self = ColorMode::UseBg;
            } else {
                *self = ColorMode::None;
            }
        });
    }
}

pub fn plot_point(buffer_view: &mut BufferView, pos: impl Into<Position>, mut mode: BrushMode, color_mode: ColorMode, point_role: PointRole) {
    let pos = pos.into();
    let text_pos = Position::new(pos.x, pos.y / 2);
    let mut ch = if let Some(layer) = buffer_view.get_edit_state().get_cur_layer() {
        layer.get_char(text_pos)
    } else {
        return;
    };

    let editor_attr = buffer_view.get_caret().get_attribute();
    let mut attribute = ch.attribute;
    if !ch.is_visible() {
        attribute = TextAttribute::default();
    }
    attribute.set_font_page(editor_attr.get_font_page());
    if color_mode.use_fore() {
        attribute.set_foreground(editor_attr.get_foreground());
    }
    if color_mode.use_back() {
        attribute.set_background(editor_attr.get_background());
    }

    if let Some(layer) = buffer_view.get_edit_state_mut().get_overlay_layer() {
        let overlay_ch = layer.get_char(text_pos);
        if overlay_ch.is_visible() {
            ch = overlay_ch;
        }
    } else {
        return;
    }

    if matches!(mode, BrushMode::HalfBlock) && matches!(point_role, PointRole::Fill) {
        mode = BrushMode::Block;
    }
    match mode {
        BrushMode::HalfBlock => {
            let half_block: HalfBlock = HalfBlock::from(buffer_view.get_buffer(), pos);
            if let Some(layer) = buffer_view.get_edit_state_mut().get_overlay_layer() {
                layer.set_char(text_pos, half_block.get_half_block_char(attribute.get_foreground()));
            }
        }
        BrushMode::Block => {
            if let Some(layer) = buffer_view.get_edit_state_mut().get_overlay_layer() {
                layer.set_char(text_pos, AttributedChar::new(219 as char, attribute));
            }
        }
        BrushMode::Char(ch) => {
            if let Some(layer) = buffer_view.get_edit_state_mut().get_overlay_layer() {
                layer.set_char(text_pos, AttributedChar::new(*ch.borrow(), attribute));
            }
        }

        BrushMode::Outline => {
            if let Some(layer) = buffer_view.get_edit_state_mut().get_overlay_layer() {
                layer.set_char(text_pos, AttributedChar::new(get_outline_char(ch, point_role), attribute));
            }
        }

        BrushMode::Shade => {
            let mut char_code = SHADE_GRADIENT[0];
            if ch.ch == SHADE_GRADIENT[SHADE_GRADIENT.len() - 1] {
                char_code = SHADE_GRADIENT[SHADE_GRADIENT.len() - 1];
            } else {
                for i in 0..SHADE_GRADIENT.len() - 1 {
                    if ch.ch == SHADE_GRADIENT[i] {
                        char_code = SHADE_GRADIENT[i + 1];
                        break;
                    }
                }
            }
            if let Some(layer) = buffer_view.get_edit_state_mut().get_overlay_layer() {
                layer.set_char(text_pos, AttributedChar::new(char_code, attribute));
            }
        }
        BrushMode::Colorize => {
            if let Some(layer) = buffer_view.get_edit_state_mut().get_overlay_layer() {
                layer.set_char(text_pos, AttributedChar::new(ch.ch, attribute));
            }
        }
        _ => {}
    }
}

fn get_outline_char(_ch: AttributedChar, point_role: PointRole) -> char {
    let ch = match point_role {
        PointRole::NWCorner => 'E',
        PointRole::NECorner => 'F',
        PointRole::SWCorner => 'K',
        PointRole::SECorner => 'L',
        PointRole::LeftSide => 'C',
        PointRole::RightSide => 'D',
        PointRole::TopSide => 'A',
        PointRole::BottomSide => 'B',
        _ => {
            return ' ';
        }
    };
    let outline_style = crate::Settings::get_font_outline_style();
    TheDrawFont::transform_outline(outline_style, ch as u8) as char
}

pub static SHADE_GRADIENT: [char; 4] = ['\u{00B0}', '\u{00B1}', '\u{00B2}', '\u{00DB}'];

#[derive(Debug)]
pub enum PointRole {
    NWCorner,
    NECorner,
    SWCorner,
    SECorner,
    LeftSide,
    RightSide,
    TopSide,
    BottomSide,
    Fill,
    Line,
}
