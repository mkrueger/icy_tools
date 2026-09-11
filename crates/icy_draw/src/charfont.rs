use crate::document::{Document, DrawResult};
use icy_engine::{char_set::TdfBufferRenderer, AttributedChar, Position, Size, TextBuffer, TextPane};
use icy_engine_edit::charset::{CharSetEditState, TdfFontType};
use retrofont::{Glyph, GlyphPart, RenderOptions};
use std::path::{Path, PathBuf};

pub struct CharFontDocument {
    pub state: CharSetEditState,
    pub path: Option<PathBuf>,
    baseline: Vec<u8>,
    disk_bytes: Option<Vec<u8>>,
}

impl CharFontDocument {
    pub fn new(font_type: TdfFontType) -> Self {
        let mut state = CharSetEditState::new_with_font_type(font_type);
        state.select_char('A');
        let baseline = state.get_autosave_bytes().unwrap_or_default();
        Self {
            state,
            path: None,
            baseline,
            disk_bytes: None,
        }
    }

    pub fn load(path: &Path) -> DrawResult<Self> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let fonts = icy_engine_edit::charset::load_tdf_fonts(&bytes).map_err(|error| error.to_string())?;
        let mut state = CharSetEditState::with_fonts(fonts, Some(path.to_path_buf()));
        state.select_char('A');
        let baseline = state.get_autosave_bytes()?;
        Ok(Self {
            state,
            path: Some(path.to_path_buf()),
            baseline,
            disk_bytes: Some(bytes),
        })
    }

    pub fn modified(&self) -> bool {
        self.state.get_autosave_bytes().is_ok_and(|bytes| bytes != self.baseline)
    }

    pub fn commit(&mut self, document: &mut Document) {
        document.finish();
        if !document.modified() {
            return;
        }
        if let (Some(character), Some(font)) = (self.state.selected_char(), self.state.selected_font()) {
            let glyph = document.with_state(|state| buffer_to_glyph(state.get_buffer(), font.font_type));
            if let Some(glyph) = glyph {
                self.state.set_glyph(character, glyph);
            } else {
                self.state.clear_glyph(character);
            }
        }
        document.with_state(|state| state.get_undo_stack().lock().unwrap().mark_saved());
    }

    pub fn document(&self) -> Document {
        let document = Document::new(Size::new(30, 12));
        document.with_state(|state| {
            if let Some(glyph) = self.state.selected_char().and_then(|character| self.state.get_glyph(character)) {
                let buffer = state.get_buffer_mut();
                let size = Size::new((glyph.width as i32).max(30), (glyph.height as i32).max(12));
                buffer.set_size(size);
                buffer.layers[0].set_size(size);
                let mut renderer = TdfBufferRenderer::new(buffer, 0, 0);
                let _ = glyph.render(
                    &mut renderer,
                    &RenderOptions {
                        render_mode: retrofont::RenderMode::Edit,
                        outline_style: 0,
                    },
                );
            }
            state.get_buffer_mut().mark_dirty();
        });
        document
    }

    pub fn save(&mut self, document: &mut Document, path: &Path) -> DrawResult<()> {
        self.save_with_overwrite(document, path, false)
    }

    pub fn save_with_overwrite(&mut self, document: &mut Document, path: &Path, overwrite: bool) -> DrawResult<()> {
        self.commit(document);
        if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("tdf")) {
            return Err("Save TheDraw font collections with a .tdf extension.".into());
        }
        let bytes = self.state.get_autosave_bytes()?;
        crate::files::save_bytes(path, &bytes, self.path.as_deref().zip(self.disk_bytes.as_deref()), overwrite)?;
        self.path = Some(path.to_path_buf());
        self.state.set_file_path(Some(path.to_path_buf()));
        self.baseline = bytes;
        self.disk_bytes = Some(self.baseline.clone());
        Ok(())
    }
}

pub fn buffer_to_glyph(buffer: &TextBuffer, font_type: TdfFontType) -> Option<Glyph> {
    buffer.layers.first()?;
    let mut width = 0;
    let mut height = 0;
    for row in 0..buffer.height() {
        for column in 0..buffer.width() {
            if !matches!(buffer.char_at(Position::new(column, row)).ch, ' ' | '\0') {
                width = width.max(column + 1);
                height = height.max(row + 1);
            }
        }
    }
    if width == 0 || height == 0 {
        return None;
    }
    let mut parts = Vec::new();
    for row in 0..height {
        let line_width = (0..width)
            .rev()
            .find(|&column| !matches!(buffer.char_at(Position::new(column, row)).ch, ' ' | '\0'))
            .map_or(0, |column| column + 1);
        for column in 0..line_width {
            parts.push(cell_part(buffer.char_at(Position::new(column, row)), font_type));
        }
        if row + 1 < height {
            parts.push(GlyphPart::NewLine);
        }
    }
    Some(Glyph {
        width: width as usize,
        height: height as usize,
        parts,
    })
}

fn cell_part(cell: AttributedChar, font_type: TdfFontType) -> GlyphPart {
    let code = cell.ch as u32;
    let character = codepages::tables::CP437_TO_UNICODE.get(code as usize).copied().unwrap_or(cell.ch);
    if font_type == TdfFontType::Outline {
        if (b'A' as u32..=b'R' as u32).contains(&code) {
            return GlyphPart::OutlinePlaceholder(code as u8);
        }
        if cell.ch == '@' {
            return GlyphPart::FillMarker;
        }
        if cell.ch == '&' {
            return GlyphPart::EndMarker;
        }
    }
    if cell.ch == '\u{00A0}' || code == 255 {
        return GlyphPart::HardBlank;
    }
    if font_type == TdfFontType::Color {
        let foreground = cell.attribute.foreground();
        let foreground = if foreground < 8 && cell.attribute.is_bold() {
            foreground + 8
        } else {
            foreground
        };
        GlyphPart::AnsiChar {
            ch: character,
            fg: foreground as u8,
            bg: cell.attribute.background() as u8,
            blink: cell.attribute.is_blinking(),
        }
    } else {
        GlyphPart::Char(character)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tdf_glyph_changes_survive_switching_and_save() {
        let mut font = CharFontDocument::new(TdfFontType::Color);
        let mut document = font.document();
        document.type_text("HELLO").unwrap();
        font.commit(&mut document);
        font.state.select_char('B');
        let mut document = font.document();
        document.type_text("B").unwrap();
        font.commit(&mut document);
        font.state.select_char('A');
        let mut document = font.document();
        assert_eq!(document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'H');
        assert!(font.modified());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.tdf");
        font.save(&mut document, &path).unwrap();
        assert!(!font.modified());
        let loaded = CharFontDocument::load(&path).unwrap();
        assert!(loaded.state.has_glyph('A'));
        assert!(loaded.state.has_glyph('B'));
    }
}
