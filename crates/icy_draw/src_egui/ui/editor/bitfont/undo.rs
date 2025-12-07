use i18n_embed_fl::fl;
use icy_engine::{BitFont, EngineResult, Glyph};

use crate::BitFontEditor;

pub trait UndoOperation: Send {
    fn get_description(&self) -> String;

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()>;
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()>;
}

pub struct Paste {
    ch: char,
    glyph: Glyph,
    old_data: Vec<u32>,
}

impl Paste {
    pub(crate) fn new(ch: char, glyph: Glyph) -> Self {
        Self {
            ch,
            glyph,
            old_data: Vec::new(),
        }
    }
}

impl UndoOperation for Paste {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-paste-glyph")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.old_data.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        let len = edit_state.font.size.height as usize;
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            self.old_data = glyph.data.clone();
            glyph.data = self.glyph.data.clone();
            glyph.data.resize(len, 0);
        }
        Ok(())
    }
}

pub struct FlipY {
    ch: char,
}

impl FlipY {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch }
    }
}

impl UndoOperation for FlipY {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-flip-y")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = glyph.data.iter().rev().copied().collect();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = glyph.data.iter().rev().copied().collect();
        }
        Ok(())
    }
}

pub struct FlipX {
    ch: char,
}

impl FlipX {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch }
    }
}

impl UndoOperation for FlipX {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-flip-x")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        let w = 8 - edit_state.font.size.width;
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            for i in 0..glyph.data.len() {
                glyph.data[i] = glyph.data[i].reverse_bits() << w;
            }
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        let w = 8 - edit_state.font.size.width;
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            for i in 0..glyph.data.len() {
                glyph.data[i] = glyph.data[i].reverse_bits() << w;
            }
        }
        Ok(())
    }
}

pub struct DownGlyph {
    ch: char,
    old_data: Vec<u32>,
}

impl DownGlyph {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch, old_data: Vec::new() }
    }
}

impl UndoOperation for DownGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-down")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.old_data.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            self.old_data = glyph.data.clone();
            glyph.data.insert(0, 0);
            glyph.data.pop();
        }
        Ok(())
    }
}

pub struct UpGlyph {
    ch: char,
    old_data: Vec<u32>,
}

impl UpGlyph {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch, old_data: Vec::new() }
    }
}

impl UndoOperation for UpGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-up")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.old_data.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            self.old_data = glyph.data.clone();
            glyph.data.remove(0);
            glyph.data.push(0);
        }
        Ok(())
    }
}

pub struct RightGlyph {
    ch: char,
    old_data: Vec<u32>,
}

impl RightGlyph {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch, old_data: Vec::new() }
    }
}

impl UndoOperation for RightGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-right")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.old_data.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            self.old_data = glyph.data.clone();
            for i in 0..glyph.data.len() {
                glyph.data[i] >>= 1;
            }
        }
        Ok(())
    }
}

pub struct LeftGlyph {
    ch: char,
    old_data: Vec<u32>,
}

impl LeftGlyph {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch, old_data: Vec::new() }
    }
}

impl UndoOperation for LeftGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-left")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.old_data.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            self.old_data = glyph.data.clone();
            for i in 0..glyph.data.len() {
                glyph.data[i] <<= 1;
            }
        }
        Ok(())
    }
}

pub struct ClearGlyph {
    ch: char,
    old_data: Vec<u32>,
}

impl ClearGlyph {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch, old_data: Vec::new() }
    }
}

impl UndoOperation for ClearGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-clear")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.old_data.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            self.old_data = glyph.data.clone();
            glyph.data.fill(0);
        }
        Ok(())
    }
}

pub struct InverseGlyph {
    ch: char,
}

impl InverseGlyph {
    pub(crate) fn new(ch: char) -> Self {
        Self { ch }
    }
}

impl UndoOperation for InverseGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-inverse")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            for i in 0..glyph.data.len() {
                glyph.data[i] ^= 0xFF;
            }
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            for i in 0..glyph.data.len() {
                glyph.data[i] ^= 0xFF;
            }
        }
        Ok(())
    }
}

pub struct Edit {
    ch: char,
    old_data: Vec<u32>,
    data: Vec<u32>,
}

impl Edit {
    pub(crate) fn new(ch: char, data: Vec<u32>, old_data: Vec<u32>) -> Self {
        Self { ch, data, old_data }
    }
}

impl UndoOperation for Edit {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-edit")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.old_data.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        if let Some(glyph) = edit_state.font.get_glyph_mut(self.ch) {
            glyph.data = self.data.clone();
        }
        Ok(())
    }
}

pub struct ResizeFont {
    old_font: BitFont,
    new_font: BitFont,
}

impl ResizeFont {
    pub(crate) fn new(old_font: BitFont, new_font: BitFont) -> Self {
        Self { old_font, new_font }
    }
}

impl UndoOperation for ResizeFont {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-resize")
    }

    fn undo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        edit_state.font = self.old_font.clone();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut BitFontEditor) -> EngineResult<()> {
        edit_state.font = self.new_font.clone();
        Ok(())
    }
}
