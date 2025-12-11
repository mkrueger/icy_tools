use std::sync::Arc;

use codepages::tables::CP437_TO_UNICODE;
use eframe::{
    egui::{self, RichText, Sense},
    epaint::{Color32, FontId, Pos2, Rect, Vec2},
};
use egui::{Context, Image, TextureHandle, load::SizedTexture, mutex::Mutex};
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, BitFont, Buffer, TextAttribute};

use crate::{AnsiEditor, Document, Message, ToolWindow, create_image};

pub struct CharTableToolWindow {
    font: BitFont,
    hover_char: Option<char>,
    hover_char_image: TextureHandle,
    char_table: TextureHandle,
    buffer_width: usize,
}

impl CharTableToolWindow {
    pub fn new(ctx: &Context, buffer_width: usize) -> Self {
        let font = BitFont::default();
        let char_table = create_font_image(ctx, &font, buffer_width, TextAttribute::default(), TextAttribute::default(), 0);
        let hover_char_image = create_hover_image(ctx, &font, ' ', 14);
        Self {
            font,
            char_table,
            hover_char: None,
            hover_char_image,
            buffer_width,
        }
    }

    pub fn font(&self) -> &BitFont {
        &self.font
    }

    pub fn set_font(&mut self, ctx: &Context, font: BitFont) {
        self.font = font;
        self.char_table = create_font_image(ctx, &self.font, self.buffer_width, TextAttribute::default(), TextAttribute::default(), 0);
        self.hover_char = None;
    }

    pub fn show_plain_char_table(&mut self, ui: &mut egui::Ui) -> Option<char> {
        let mut something_hovered = false;
        let mut result = None;
        egui::ScrollArea::vertical().id_salt("char_table_scroll_area").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let scale = 2.0;
                let sized_texture: SizedTexture = (&self.char_table).into();
                let width = sized_texture.size.x * scale;
                let height = sized_texture.size.y * scale;
                ui.add_space((ui.available_width() - width) / 2.0);

                let (id, rect) = ui.allocate_space([width, height].into());
                let response = ui.interact(rect, id, Sense::click());
                let r = Rect::from_min_size(Pos2::new(rect.left(), rect.top()), Vec2::new(width, height));
                let image = Image::from_texture(sized_texture);

                image.paint_at(ui, r);

                let fw = scale * self.font.size.width as f32;
                let fh = scale * self.font.size.height as f32;
                if response.clicked() {
                    result = self.hover_char;
                }
                if response.hovered() {
                    if let Some(pos) = response.hover_pos() {
                        something_hovered = true;
                        let pos = pos - response.rect.min;
                        let ch = (pos.x / fw) as usize + self.buffer_width * (pos.y / fh) as usize;
                        let ch = unsafe { char::from_u32_unchecked(ch as u32) };
                        let hover_char = Some(ch);
                        if self.hover_char != hover_char {
                            self.hover_char = hover_char;
                            self.hover_char_image = create_hover_image(ui.ctx(), &self.font, ch, 14);
                        }

                        let x = (ch as usize) % self.buffer_width;
                        let y = (ch as usize) / self.buffer_width;

                        let rect = Rect::from_min_size(rect.min + Vec2::new(x as f32 * fw, y as f32 * fh), Vec2::new(fw, fh));
                        let sized_texture: SizedTexture = (&self.hover_char_image).into();
                        let image = Image::from_texture(sized_texture);
                        image.paint_at(ui, rect.expand(2.0));
                    }
                }
            });
        });
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "font-view-font_label")).small());
            ui.label(RichText::new(self.font.name.to_string()).small().color(Color32::WHITE));
        });

        if let Some(ch) = self.hover_char {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "font-view-char_label")).small());
                ui.label(RichText::new(format!("{0}/0x{0:02X}", ch as u32)).small().color(Color32::WHITE));
            });
        } else {
            ui.horizontal(|ui| {
                ui.label("   ");
            });
            ui.horizontal(|ui| {
                ui.label("   ");
            });
        }
        if !something_hovered {
            self.hover_char = None;
        }
        result
    }

    pub fn show_char_table(&mut self, ui: &mut egui::Ui, editor: &AnsiEditor) -> Option<Message> {
        let mut result = None;

        let font_page = editor.buffer_view.lock().get_caret().font_page();
        let font_count = editor.buffer_view.lock().get_buffer().font_count();
        if let Some(cur_font) = editor.buffer_view.lock().get_buffer().font(font_page) {
            if cur_font.name != self.font.name {
                self.font = cur_font.clone();
                self.char_table = create_font_image(ui.ctx(), &self.font, self.buffer_width, TextAttribute::default(), TextAttribute::default(), 0);
                self.hover_char = None;
            }
        }

        if font_count > 1 {
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.add_space(12.0);

                ui.label(fl!(crate::LANGUAGE_LOADER, "font-view-font_page_label"));
                if ui.selectable_label(false, RichText::new("◀").font(FontId::proportional(14.))).clicked() {
                    let mut prev = font_page;
                    let mut last = 0;
                    for (page, _) in editor.buffer_view.lock().get_buffer().font_iter() {
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
                        result = Some(Message::SetFontPage(last));
                    } else {
                        result = Some(Message::SetFontPage(prev));
                    }
                }
                ui.label(RichText::new(font_page.to_string()));

                if ui.selectable_label(false, RichText::new("▶").font(FontId::proportional(14.))).clicked() {
                    let mut next = font_page;
                    let mut first = usize::MAX;
                    for (page, _) in editor.buffer_view.lock().get_buffer().font_iter() {
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
                        result = Some(Message::SetFontPage(first));
                    } else {
                        result = Some(Message::SetFontPage(next));
                    }
                }
            });
        }

        let response = self.show_plain_char_table(ui);

        if let Some(ch) = response {
            result = Some(Message::CharTable(ch));
        }

        result
    }
}

pub fn create_font_image(
    ctx: &Context,
    font: &BitFont,
    buffer_width: usize,
    attribute: TextAttribute,
    selected_attribute: TextAttribute,
    selected: usize,
) -> TextureHandle {
    let mut buffer = Buffer::new((buffer_width, (font.length as usize) / buffer_width));
    buffer.set_font(0, font.clone());
    for ch in 0..font.length as usize {
        let mut attr = if ch == selected { selected_attribute } else { attribute };
        let is_ctrl_code = ch < b' ' as usize && (CP437_TO_UNICODE[ch] as usize) == ch;
        if is_ctrl_code {
            attr.set_foreground(8);
        }
        buffer.layers[0].set_char(
            (ch % buffer_width, ch / buffer_width),
            AttributedChar::new(unsafe { char::from_u32_unchecked(ch as u32) }, attr),
        );
    }
    create_image(ctx, &buffer)
}

pub fn create_hover_image(ctx: &Context, font: &BitFont, ch: char, color: u32) -> TextureHandle {
    let mut buffer = Buffer::new((1, 1));
    buffer.set_font(0, font.clone());
    let mut attr = TextAttribute::default();
    attr.set_foreground(color);

    buffer.layers[0].set_char((0, 0), AttributedChar::new(unsafe { char::from_u32_unchecked(ch as u32) }, attr));
    create_image(ctx, &buffer)
}

impl ToolWindow for CharTableToolWindow {
    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "char_table_tool_title")
    }

    fn show_ui(&mut self, ui: &mut egui::Ui, active_document: Option<Arc<Mutex<Box<dyn Document>>>>) -> Option<Message> {
        if let Some(doc) = active_document {
            if let Some(editor) = doc.lock().get_ansi_editor() {
                return self.show_char_table(ui, editor);
            }
        }
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "no_document_selected")).small());
        });
        None
    }
}
