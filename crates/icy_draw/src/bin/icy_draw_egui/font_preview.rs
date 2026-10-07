use super::font::{color_key, display_pixel, display_width, hash_glyphs, palette_color};
use eframe::egui;
use icy_draw::fl;
use icy_engine::BufferType;
use icy_engine_edit::bitfont::BitFontEditState;
use std::hash::{Hash, Hasher};

const MAX_COLUMNS: usize = 64;
const MAX_ROWS: usize = 16;
const MAX_CHARACTERS: usize = MAX_COLUMNS * MAX_ROWS;

pub struct FontPreview {
    pub visible: bool,
    text: String,
    preset: Preset,
    scale: u32,
    texture: Option<(u64, egui::TextureHandle)>,
}

impl Default for FontPreview {
    fn default() -> Self {
        Self {
            visible: true,
            text: Preset::Alphabet.text().unwrap().to_owned(),
            preset: Preset::Alphabet,
            scale: 1,
            texture: None,
        }
    }
}

impl FontPreview {
    pub fn show(&mut self, context: &egui::Context, state: &BitFontEditState, colors: (u32, u32), interactive: bool) {
        if !self.visible {
            return;
        }
        egui::TopBottomPanel::bottom("font-live-preview")
            .default_height(160.0)
            .height_range(100.0..=320.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(context.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(context, |ui| {
                if !interactive {
                    ui.disable();
                }
                ui.horizontal_wrapped(|ui| {
                    ui.strong(fl!("font-editor-sample-text"));
                    let before = self.preset;
                    egui::ComboBox::from_id_salt("font-preview-preset")
                        .width(120.0)
                        .selected_text(self.preset.label())
                        .show_ui(ui, |ui| {
                            for preset in [Preset::Alphabet, Preset::BoxDrawing, Preset::Shading, Preset::Custom] {
                                ui.selectable_value(&mut self.preset, preset, preset.label());
                            }
                        });
                    if self.preset != before {
                        if let Some(text) = self.preset.text() {
                            self.text = text.to_owned();
                        }
                    }
                    ui.separator();
                    ui.selectable_value(&mut self.scale, 1, fl!("font-editor-preview-native"));
                    ui.selectable_value(&mut self.scale, 2, "2×");
                });
                ui.add_space(4.0);
                let height = ui.available_height();
                let input_width = (ui.available_width() * 0.35).clamp(120.0, 280.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(input_width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("font-preview-input-scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let response = ui.add(
                                    egui::TextEdit::multiline(&mut self.text)
                                        .id(text_id())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(input_width)
                                        .desired_rows(3)
                                        .char_limit(MAX_CHARACTERS)
                                        .hint_text(fl!("font-editor-preview-text-hint")),
                                );
                                if response.changed() {
                                    self.preset = Preset::Custom;
                                    ui.ctx().request_repaint();
                                }
                                response.on_hover_text(fl!("font-editor-preview-text-tooltip", limit = MAX_CHARACTERS));
                            });
                    });
                    ui.separator();
                    let layout = preview_layout(&self.text);
                    self.update_texture(ui.ctx(), state, colors, &layout);
                    ui.vertical(|ui| {
                        if layout.unsupported > 0 {
                            ui.colored_label(ui.visuals().warn_fg_color, fl!("font-editor-preview-unsupported", count = layout.unsupported));
                        }
                        if layout.clipped {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                fl!("font-editor-preview-clipped", columns = MAX_COLUMNS, rows = MAX_ROWS),
                            );
                        }
                        if let Some((_, texture)) = &self.texture {
                            egui::ScrollArea::both()
                                .id_salt("font-preview-image-scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add(egui::Image::new(texture).fit_to_exact_size(texture.size_vec2() * self.scale as f32));
                                });
                        }
                    });
                });
            });
    }

    fn update_texture(&mut self, context: &egui::Context, state: &BitFontEditState, colors: (u32, u32), layout: &PreviewLayout) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut hasher);
        let key = hasher.finish()
            ^ hash_glyphs(
                state.get_all_glyph_data().iter(),
                display_width(state) as usize,
                state.font_height().max(1) as usize,
            )
            ^ color_key(colors);
        if self.texture.as_ref().is_some_and(|(cached, _)| *cached == key) {
            return;
        }
        let image = preview_image(state, colors, layout);
        if let Some((cached, texture)) = &mut self.texture {
            texture.set(image, egui::TextureOptions::NEAREST);
            *cached = key;
        } else {
            self.texture = Some((key, context.load_texture("font-live-preview", image, egui::TextureOptions::NEAREST)));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Preset {
    Alphabet,
    BoxDrawing,
    Shading,
    Custom,
}

impl Preset {
    fn label(self) -> String {
        match self {
            Self::Alphabet => fl!("font-editor-preview-alphabet"),
            Self::BoxDrawing => fl!("font-editor-preview-box-drawing"),
            Self::Shading => fl!("font-editor-preview-shading"),
            Self::Custom => fl!("font-editor-preview-custom"),
        }
    }

    fn text(self) -> Option<&'static str> {
        match self {
            Self::Alphabet => Some("ABCDEFGHIJKLMNOPQRSTUVWXYZ\nabcdefghijklmnopqrstuvwxyz\n0123456789 !?.,:;+-*/"),
            Self::BoxDrawing => Some("┌───┬───┐\n│   │   │\n├───┼───┤\n│   │   │\n└───┴───┘"),
            Self::Shading => Some("░░░▒▒▒▓▓▓███\n▀▀▀▄▄▄▌▌▌▐▐▐\n█▀█ █▄█ ▄▀▄"),
            Self::Custom => None,
        }
    }
}

pub(super) fn text_id() -> egui::Id {
    egui::Id::new("font-live-preview-text")
}

struct PreviewLayout {
    rows: Vec<Vec<char>>,
    unsupported: usize,
    clipped: bool,
}

fn preview_layout(text: &str) -> PreviewLayout {
    let mut layout = PreviewLayout {
        rows: vec![Vec::new()],
        unsupported: 0,
        clipped: false,
    };
    'characters: for character in text.chars() {
        match character {
            '\r' => continue,
            '\n' => {
                if layout.rows.len() == MAX_ROWS {
                    layout.clipped = true;
                    break;
                }
                layout.rows.push(Vec::new());
                continue;
            }
            _ => {}
        }
        let row = layout.rows.last().unwrap();
        let count = if character == '\t' { 4 - row.len() % 4 } else { 1 };
        let code = if character == '\t' {
            ' '
        } else if let Some(code) = BufferType::CP437.try_convert_from_unicode(character) {
            code
        } else {
            layout.unsupported += 1;
            '?'
        };
        for _ in 0..count {
            if layout.rows.last().unwrap().len() == MAX_COLUMNS {
                if layout.rows.len() == MAX_ROWS {
                    layout.clipped = true;
                    break 'characters;
                }
                layout.rows.push(Vec::new());
            }
            layout.rows.last_mut().unwrap().push(code);
        }
    }
    layout
}

fn preview_image(state: &BitFontEditState, colors: (u32, u32), layout: &PreviewLayout) -> egui::ColorImage {
    let width = state.font_width().max(1) as usize;
    let columns = display_width(state) as usize;
    let height = state.font_height().max(1) as usize;
    let text_width = layout.rows.iter().map(Vec::len).max().unwrap().max(1);
    let foreground = palette_color(colors.0);
    let mut image = egui::ColorImage::filled([text_width * columns, layout.rows.len() * height], palette_color(colors.1));
    for (row, line) in layout.rows.iter().enumerate() {
        for (column, character) in line.iter().enumerate() {
            for (y, pixels) in state.get_glyph_pixels(*character).iter().enumerate() {
                for x in 0..columns {
                    if display_pixel(pixels, x, width, *character as u32) {
                        image[(column * columns + x, row * height + y)] = foreground;
                    }
                }
            }
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::BitFont;
    use icy_engine_edit::bitfont::BitFontUndoState;

    #[test]
    fn preview_layout_converts_cp437_and_handles_line_breaks_and_tabs() {
        let layout = preview_layout("A\tB\r\n╬\u{1f600}");
        assert_eq!(layout.rows, vec![vec!['A', ' ', ' ', ' ', 'B'], vec![char::from(0xCE), '?']]);
        assert_eq!(layout.unsupported, 1);
        assert!(!layout.clipped);
        for preset in [Preset::Alphabet, Preset::BoxDrawing, Preset::Shading] {
            let layout = preview_layout(preset.text().unwrap());
            assert_eq!(layout.unsupported, 0);
            assert!(!layout.clipped);
        }
    }

    #[test]
    fn preview_layout_wraps_and_reports_the_exact_display_limit() {
        let layout = preview_layout(&"A".repeat(MAX_COLUMNS * MAX_ROWS));
        assert_eq!(layout.rows.len(), MAX_ROWS);
        assert!(layout.rows.iter().all(|row| row.len() == MAX_COLUMNS));
        assert!(!layout.clipped);
        let layout = preview_layout(&"A".repeat(MAX_COLUMNS * MAX_ROWS + 1));
        assert_eq!(layout.rows.len(), MAX_ROWS);
        assert!(layout.clipped);
        let layout = preview_layout(&"\n".repeat(MAX_ROWS - 1));
        assert_eq!(layout.rows.len(), MAX_ROWS);
        assert!(!layout.clipped);
        assert!(preview_layout(&"\n".repeat(MAX_ROWS)).clipped);
        let layout = preview_layout(&format!("{}\tB", "A".repeat(MAX_COLUMNS - 1)));
        assert_eq!(layout.rows[0].len(), MAX_COLUMNS);
        assert_eq!(layout.rows[0][MAX_COLUMNS - 1], ' ');
        assert_eq!(layout.rows[1], vec!['B']);
    }

    #[test]
    fn preview_image_renders_exact_pixels_including_nine_dot_box_drawing() {
        let mut state = BitFontEditState::from_font(BitFont::from_ansi_font_page(0, 16).unwrap().clone());
        state.set_letter_spacing(true);
        for ch in ['A', char::from(0xC4), '?'] {
            state.clear_glyph(ch).unwrap();
        }
        state.set_pixel('A', 1, 2, true).unwrap();
        state.set_pixel('A', 7, 3, true).unwrap();
        state.set_pixel(char::from(0xC4), 7, 3, true).unwrap();
        state.set_pixel('?', 2, 1, true).unwrap();
        let layout = preview_layout("A─\n\u{1f600}");
        let image = preview_image(&state, (7, 0), &layout);
        assert_eq!(image.size, [18, 32]);
        let set = [(1, 2), (7, 3), (16, 3), (17, 3), (2, 17)];
        for y in 0..image.size[1] {
            for x in 0..image.size[0] {
                assert_eq!(image[(x, y)], palette_color(if set.contains(&(x, y)) { 7 } else { 0 }), "pixel {x}, {y}");
            }
        }
        state.set_letter_spacing(false);
        assert_eq!(preview_image(&state, (7, 0), &layout).size, [16, 32]);
        let empty = preview_image(&state, (7, 1), &preview_layout(""));
        assert_eq!(empty.size, [8, 16]);
        assert!(empty.pixels.iter().all(|color| *color == palette_color(1)));
    }

    #[test]
    fn preview_texture_refreshes_on_content_changes_but_not_on_focus_or_zoom() {
        let context = egui::Context::default();
        let mut state = BitFontEditState::from_font(BitFont::from_ansi_font_page(0, 16).unwrap().clone());
        state.set_letter_spacing(false);
        let mut preview = FontPreview::default();
        let update = |preview: &mut FontPreview, state: &BitFontEditState, colors| {
            let layout = preview_layout(&preview.text);
            context.run(Default::default(), |context| preview.update_texture(context, state, colors, &layout))
        };
        let first = update(&mut preview, &state, (7, 0));
        let id = preview.texture.as_ref().unwrap().1.id();
        assert!(first.textures_delta.set.iter().any(|(texture, _)| *texture == id));
        let original_key = preview.texture.as_ref().unwrap().0;
        state.set_selected_char('B');
        preview.scale = 2;
        assert!(!update(&mut preview, &state, (7, 0))
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        let pixel = state.get_glyph_pixels('A')[0][0];
        state.set_pixel('A', 0, 0, !pixel).unwrap();
        assert!(update(&mut preview, &state, (7, 0))
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        let changed_key = preview.texture.as_ref().unwrap().0;
        assert_ne!(changed_key, original_key);
        state.undo().unwrap();
        update(&mut preview, &state, (7, 0));
        assert_eq!(preview.texture.as_ref().unwrap().0, original_key);
        state.redo().unwrap();
        update(&mut preview, &state, (7, 0));
        assert_eq!(preview.texture.as_ref().unwrap().0, changed_key);
        update(&mut preview, &state, (0, 7));
        assert_ne!(preview.texture.as_ref().unwrap().0, changed_key);
        state.resize_font(6, 8).unwrap();
        update(&mut preview, &state, (7, 0));
        assert_eq!(preview.texture.as_ref().unwrap().1.size(), [26 * 6, 3 * 8]);
        state.resize_font(8, 16).unwrap();
        state.set_letter_spacing(true);
        update(&mut preview, &state, (7, 0));
        assert_eq!(preview.texture.as_ref().unwrap().1.size(), [26 * 9, 3 * 16]);
        preview.text = "A".to_owned();
        update(&mut preview, &state, (7, 0));
        assert_eq!(preview.texture.as_ref().unwrap().1.size(), [9, 16]);
        assert_eq!(preview.texture.as_ref().unwrap().1.id(), id, "updates reuse the texture");
    }

    #[test]
    fn preview_controls_switch_presets_and_render_exact_native_and_double_size() {
        for size in [egui::vec2(1280.0, 820.0), egui::vec2(740.0, 600.0)] {
            let context = egui::Context::default();
            let mut state = BitFontEditState::from_font(BitFont::from_ansi_font_page(0, 16).unwrap().clone());
            state.set_letter_spacing(false);
            let mut preview = FontPreview::default();
            let render = |preview: &mut FontPreview, events| {
                context.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        events,
                        time: Some(context.input(|input| input.time) + 0.05),
                        ..Default::default()
                    },
                    |context| preview.show(context, &state, (7, 0), true),
                )
            };
            let click = |preview: &mut FontPreview, label: &str| {
                let output = render(preview, vec![]);
                let position = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing preview control {label}"));
                render(preview, vec![egui::Event::PointerMoved(position)]);
                for pressed in [true, false] {
                    render(
                        preview,
                        vec![egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                }
            };
            for _ in 0..3 {
                render(&mut preview, vec![]);
            }
            let image_bounds = |preview: &FontPreview, output: &egui::FullOutput| {
                let id = preview.texture.as_ref().unwrap().1.id();
                output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect) if rect.fill_texture_id() == id => Some(rect.rect),
                        _ => None,
                    })
                    .unwrap()
            };
            let output = render(&mut preview, vec![]);
            assert_eq!(image_bounds(&preview, &output).size(), preview.texture.as_ref().unwrap().1.size_vec2());
            click(&mut preview, "2×");
            let output = render(&mut preview, vec![]);
            assert_eq!(image_bounds(&preview, &output).size(), preview.texture.as_ref().unwrap().1.size_vec2() * 2.0);
            click(&mut preview, &Preset::Alphabet.label());
            click(&mut preview, &Preset::BoxDrawing.label());
            assert!(preview.preset == Preset::BoxDrawing);
            assert_eq!(preview.text, Preset::BoxDrawing.text().unwrap());
            assert_eq!(preview.texture.as_ref().unwrap().1.size(), [9 * 8, 5 * 16]);
            click(&mut preview, &Preset::BoxDrawing.label());
            click(&mut preview, &Preset::Shading.label());
            assert!(preview.preset == Preset::Shading);
            assert_eq!(preview.text, Preset::Shading.text().unwrap());
        }
    }
}
