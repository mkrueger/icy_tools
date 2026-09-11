use super::widgets::{self, Icons};
use eframe::egui::{self, Color32};
use icy_engine::BitFont;
use icy_engine_edit::bitfont::{BitFontAtomicUndoGuard, BitFontClipboardData, BitFontEditState, BitFontUndoState};
use icy_engine_gui::egui::{appearance, screen::ScreenView};
use std::path::{Path, PathBuf};

pub enum Action {
    Apply(BitFont),
    Save,
    Close,
}

pub struct FontEditor {
    pub state: BitFontEditState,
    pub path: Option<PathBuf>,
    baseline: Vec<u8>,
    disk_bytes: Option<Vec<u8>>,
    stroke: Option<BitFontAtomicUndoGuard>,
    drawing: bool,
    last: Option<icy_engine::Position>,
    icons: Icons,
    clipboard: Option<BitFontClipboardData>,
    confirm_close: bool,
    dimensions: [i32; 2],
    preview: Option<ScreenView>,
    error: Option<String>,
}

impl FontEditor {
    pub fn new(font: BitFont) -> Self {
        let baseline = font.to_psf2_bytes().unwrap_or_default();
        let dimensions = [font.size().width, font.size().height];
        Self {
            state: BitFontEditState::from_font(font),
            path: None,
            baseline,
            disk_bytes: None,
            stroke: None,
            drawing: false,
            last: None,
            icons: Icons::default(),
            clipboard: None,
            confirm_close: false,
            dimensions,
            preview: None,
            error: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|error| error.to_string())?;
        let font = BitFont::from_bytes(path.file_name().unwrap_or_default().to_string_lossy().to_string(), &data).map_err(|error| error.to_string())?;
        if font.size().width > 8 {
            return Err("The bitmap editing backend currently supports glyphs up to 8 pixels wide.".into());
        }
        let mut editor = Self::new(font);
        editor.path = Some(path.to_path_buf());
        editor.disk_bytes = Some(data);
        Ok(editor)
    }

    pub fn modified(&self) -> bool {
        self.state.build_font().to_psf2_bytes().is_ok_and(|bytes| bytes != self.baseline)
    }

    pub fn mcp_status(&self) -> icy_draw::mcp::types::BitFontStatus {
        let count = self.state.get_all_glyph_data().len();
        icy_draw::mcp::types::BitFontStatus {
            glyph_width: self.state.font_width(),
            glyph_height: self.state.font_height(),
            glyph_count: count,
            first_char: 0,
            last_char: count.saturating_sub(1) as u32,
            selected_char: self.state.selected_char() as u32,
        }
    }

    pub fn mcp_get(&self, code: u32) -> Result<icy_draw::mcp::types::GlyphData, String> {
        use base64::Engine;
        let pixels = self.state.get_all_glyph_data().get(code as usize).ok_or("Character out of bounds")?;
        let bits: Vec<_> = pixels.iter().flatten().copied().collect();
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (index, enabled) in bits.into_iter().enumerate() {
            if enabled {
                bytes[index / 8] |= 1 << (7 - index % 8);
            }
        }
        Ok(icy_draw::mcp::types::GlyphData {
            code,
            char: char::from_u32(code).map(|character| character.to_string()),
            width: self.state.font_width(),
            height: self.state.font_height(),
            bitmap: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
    }

    pub fn mcp_set(&mut self, code: u32, data: &icy_draw::mcp::types::GlyphData) -> Result<(), String> {
        use base64::Engine;
        if code as usize >= self.state.get_all_glyph_data().len()
            || code != data.code
            || data.width != self.state.font_width()
            || data.height != self.state.font_height()
        {
            return Err("Glyph code or dimensions mismatch".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data.bitmap)
            .map_err(|error| error.to_string())?;
        let count = (data.width * data.height) as usize;
        if bytes.len() != count.div_ceil(8) {
            return Err("Glyph bitmap size mismatch".into());
        }
        let pixels = (0..data.height as usize)
            .map(|row| {
                (0..data.width as usize)
                    .map(|column| {
                        let index = row * data.width as usize + column;
                        bytes[index / 8] & (1 << (7 - index % 8)) != 0
                    })
                    .collect()
            })
            .collect();
        self.finish();
        self.preview = None;
        self.state
            .set_glyph_pixels(char::from_u32(code).ok_or("Invalid character")?, pixels)
            .map_err(|error| error.to_string())
    }

    pub fn finish(&mut self) {
        if let Some(mut stroke) = self.stroke.take() {
            if let Some((count, description, operation)) = stroke.end_params() {
                self.state.end_atomic_undo(count, description, operation);
            }
        }
        self.last = None;
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> Result<(), String> {
        self.finish();
        if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("psf")) {
            return Err("Save bitmap fonts with a .psf extension.".into());
        }
        let bytes = self.state.build_font().to_psf2_bytes().map_err(|error| error.to_string())?;
        icy_draw::files::save_bytes(path, &bytes, self.path.as_deref().zip(self.disk_bytes.as_deref()), overwrite)?;
        self.disk_bytes = Some(bytes.clone());
        self.baseline = bytes;
        self.path = Some(path.to_path_buf());
        self.state.mark_saved();
        Ok(())
    }

    fn operation(&mut self, operation: impl FnOnce(&mut BitFontEditState) -> icy_engine::Result<()>) {
        self.finish();
        self.preview = None;
        if let Err(error) = operation(&mut self.state) {
            self.error = Some(error.to_string());
        }
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        let mut action = None;
        let response = appearance::Dialog::new("font-editor", "Bitmap Font").max_width(860.0).show(context, |dialog| {
            dialog.content(|ui| {
                if blocked {
                    ui.disable();
                }
                let character = self.state.selected_char();
                ui.horizontal_wrapped(|ui| {
                    if self.icons.button(ui, "arrow_left", "Undo", false).clicked() {
                        self.operation(|state| state.undo());
                    }
                    if self.icons.button(ui, "arrow_right", "Redo", false).clicked() {
                        self.operation(|state| state.redo());
                    }
                    if self.icons.button(ui, "flip_tool", "Flip Horizontally", false).clicked() {
                        self.operation(|state| state.flip_glyph_x(character));
                    }
                    if self.icons.button(ui, "swap", "Flip Vertically", false).clicked() {
                        self.operation(|state| state.flip_glyph_y(character));
                    }
                    if self.icons.button(ui, "invisible", "Invert", false).clicked() {
                        self.operation(|state| state.inverse_glyph(character));
                    }
                    if self.icons.button(ui, "delete", "Clear Glyph", false).clicked() {
                        self.operation(|state| state.clear_glyph(character));
                    }
                    if self.icons.button(ui, "file_copy", "Copy Glyph", false).clicked() {
                        self.clipboard = Some(BitFontClipboardData::new(self.state.get_copy_data()));
                    }
                    if ui.button("Paste").clicked() {
                        if let Some(data) = self.clipboard.clone() {
                            self.operation(|state| state.paste_data(data));
                        }
                    }
                    if ui.button("Preview").clicked() {
                        self.finish();
                        self.preview = Some(ScreenView::new(self.state.build_preview_content_for(character, 7, 0)));
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.dimensions[0]).range(1..=8).prefix("Width "));
                    ui.add(egui::DragValue::new(&mut self.dimensions[1]).range(1..=32).prefix("Height "));
                    if ui.button("Resize").clicked() {
                        let dimensions = self.dimensions;
                        self.operation(|state| state.resize_font(dimensions[0], dimensions[1]));
                    }
                    ui.label(format!("Character {}", character as u32));
                });
                if let Some(preview) = &mut self.preview {
                    let settings = icy_engine_gui::MonitorSettings {
                        scaling_mode: icy_engine_gui::ScalingMode::Auto,
                        ..Default::default()
                    };
                    ui.allocate_ui(egui::vec2(ui.available_width(), 260.0), |ui| {
                        preview.show(ui, &settings);
                    });
                    if ui.button("Edit").clicked() {
                        self.preview = None;
                    }
                } else {
                    egui::ScrollArea::horizontal().id_salt("font-grids").show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            let font = self.state.build_font();
                            egui::Grid::new("font-charset").spacing(egui::Vec2::splat(1.0)).show(ui, |ui| {
                                for code in 0..256 {
                                    let code = char::from_u32(code).unwrap();
                                    if widgets::glyph(ui, &font, code, code == character, 19.0).clicked() {
                                        self.finish();
                                        self.state.set_selected_char(code);
                                    }
                                    if code as u32 % 16 == 15 {
                                        ui.end_row();
                                    }
                                }
                            });
                            ui.separator();
                            let width = self.state.font_width();
                            let height = self.state.font_height();
                            let pixel = (300.0 / height.max(width) as f32).clamp(4.0, 24.0);
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(width as f32, height as f32) * pixel, egui::Sense::click_and_drag());
                            let pixels = self.state.get_glyph_pixels(character);
                            for row in 0..height {
                                for column in 0..width {
                                    let cell = egui::Rect::from_min_size(rect.min + egui::vec2(column as f32, row as f32) * pixel, egui::Vec2::splat(pixel));
                                    let color = if pixels[row as usize][column as usize] {
                                        ui.visuals().text_color()
                                    } else {
                                        Color32::from_gray(32)
                                    };
                                    ui.painter().rect_filled(cell.shrink(0.5), 0, color);
                                }
                            }
                            let pointer = ui.input(|input| input.pointer.clone());
                            if response.hovered() && pointer.any_pressed() {
                                self.finish();
                                self.stroke = Some(self.state.begin_atomic_undo("Draw glyph"));
                                self.drawing = !pointer.secondary_down();
                            }
                            if self.stroke.is_some() {
                                if let Some(position) = pointer.interact_pos().filter(|position| rect.contains(*position)) {
                                    let point =
                                        icy_engine::Position::new(((position.x - rect.left()) / pixel) as i32, ((position.y - rect.top()) / pixel) as i32);
                                    for point in icy_engine_edit::brushes::get_line_points(self.last.unwrap_or(point), point) {
                                        let _ = self.state.set_pixel(character, point.x, point.y, self.drawing);
                                    }
                                    self.last = Some(point);
                                }
                                if !pointer.any_down() {
                                    self.finish();
                                }
                            }
                        });
                    });
                }
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if self.confirm_close {
                    ui.separator();
                    ui.label("Discard unsaved font changes?");
                    ui.horizontal(|ui| {
                        if ui.button("Discard").clicked() {
                            action = Some(Action::Close);
                        }
                        if ui.button("Keep Editing").clicked() {
                            self.confirm_close = false;
                        }
                    });
                }
            });
            dialog.actions(|ui| {
                if blocked {
                    ui.disable();
                }
                if ui.button("Apply to Document").clicked() {
                    self.finish();
                    action = Some(Action::Apply(self.state.build_font()));
                }
                if ui.button("Save Font...").clicked() {
                    self.finish();
                    action = Some(Action::Save);
                }
                if ui.button("Close").clicked() {
                    if self.modified() {
                        self.confirm_close = true;
                    } else {
                        action = Some(Action::Close);
                    }
                }
            });
        });
        if response.closed && !blocked {
            if self.modified() {
                self.confirm_close = true;
            } else {
                action = Some(Action::Close);
            }
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_stroke_is_one_undo_and_psf_roundtrips() {
        let font = BitFont::from_ansi_font_page(0, 16).unwrap().clone();
        let mut editor = FontEditor::new(font);
        editor.state.clear_glyph('A').unwrap();
        editor.state.mark_saved();
        editor.baseline = editor.state.build_font().to_psf2_bytes().unwrap();
        let before = editor.state.undo_stack_len();
        editor.stroke = Some(editor.state.begin_atomic_undo("Draw glyph"));
        editor.state.set_pixel('A', 1, 1, true).unwrap();
        editor.state.set_pixel('A', 2, 1, true).unwrap();
        editor.finish();
        assert_eq!(editor.state.undo_stack_len(), before + 1);
        assert!(editor.modified());
        editor.state.undo().unwrap();
        assert!(!editor.modified());
        editor.state.redo().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.psf");
        editor.save(&path, false).unwrap();
        assert!(!editor.modified());
        let loaded = FontEditor::load(&path).unwrap();
        assert!(loaded.state.get_glyph_pixels('A')[1][1]);
        assert!(loaded.state.get_glyph_pixels('A')[1][2]);
        std::fs::write(&path, b"changed externally").unwrap();
        assert!(editor.save(&path, false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"changed externally");
    }
}
