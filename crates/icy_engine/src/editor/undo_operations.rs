use std::{
    mem,
    sync::{Arc, Mutex},
};

use i18n_embed_fl::fl;
use icy_sauce::SauceMetaInformation;

use crate::{
    AddType, AttributedChar, BitFont, EngineResult, IceMode, Layer, Line, Palette, PaletteMode, Position, Properties, Selection, SelectionMask, Size, Tag,
    TextPane,
};

use super::{EditState, EditorError, OperationType, UndoOperation};

pub(crate) struct AtomicUndo {
    description: String,
    stack: Arc<Mutex<Vec<Box<dyn UndoOperation>>>>,
    operation_type: OperationType,
}

impl AtomicUndo {
    pub(crate) fn new(description: String, stack: Arc<Mutex<Vec<Box<dyn UndoOperation>>>>, operation_type: OperationType) -> Self {
        Self {
            description,
            stack,
            operation_type,
        }
    }
}

impl UndoOperation for AtomicUndo {
    fn get_description(&self) -> String {
        self.description.clone()
    }

    fn changes_data(&self) -> bool {
        for op in self.stack.lock().unwrap().iter() {
            if op.changes_data() {
                return true;
            }
        }
        false
    }

    fn get_operation_type(&self) -> OperationType {
        self.operation_type
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        for op in self.stack.lock().unwrap().iter_mut().rev() {
            op.undo(edit_state)?;
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        for op in self.stack.lock().unwrap().iter_mut() {
            op.redo(edit_state)?;
        }
        Ok(())
    }

    fn try_clone(&self) -> Option<Box<dyn UndoOperation>> {
        Some(Box::new(AtomicUndo::new(self.description.clone(), self.stack.clone(), self.operation_type)))
    }
}

pub struct UndoSetChar {
    pub pos: Position,
    pub layer: usize,
    pub old: AttributedChar,
    pub new: AttributedChar,
}

impl UndoOperation for UndoSetChar {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_char")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers[self.layer].set_char(self.pos, self.old);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers[self.layer].set_char(self.pos, self.new);
        Ok(())
    }
}

pub struct UndoSwapChar {
    pub layer: usize,
    pub pos1: Position,
    pub pos2: Position,
}
impl UndoOperation for UndoSwapChar {
    fn get_description(&self) -> String {
        String::new() // No stand alone operation.
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers[self.layer].swap_char(self.pos1, self.pos2);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers[self.layer].swap_char(self.pos1, self.pos2);
        Ok(())
    }
}

pub struct ClearLayerOperation {
    layer_num: usize,
    lines: Vec<crate::Line>,
}

impl ClearLayerOperation {
    pub fn _new(layer_num: usize) -> Self {
        Self { layer_num, lines: Vec::new() }
    }
}

impl UndoOperation for ClearLayerOperation {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-clear-layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        mem::swap(&mut self.lines, &mut edit_state.buffer.layers[self.layer_num].lines);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        mem::swap(&mut self.lines, &mut edit_state.buffer.layers[self.layer_num].lines);
        Ok(())
    }
}

#[derive(Default)]
pub struct AddLayer {
    index: usize,
    layer: Option<Layer>,
}

impl AddLayer {
    pub(crate) fn new(index: usize, new_layer: Layer) -> Self {
        Self { index, layer: Some(new_layer) }
    }
}

impl UndoOperation for AddLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-add_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        self.layer = Some(edit_state.buffer.layers.remove(self.index));
        edit_state.clamp_current_layer();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = self.layer.take() {
            edit_state.buffer.layers.insert(self.index, layer);
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct RemoveLayer {
    layer_index: usize,
    layer: Option<Layer>,
}

impl RemoveLayer {
    pub fn new(layer_index: usize) -> Self {
        Self { layer_index, layer: None }
    }
}

impl UndoOperation for RemoveLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-remove_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = self.layer.take() {
            edit_state.buffer.layers.insert(self.layer_index, layer);
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if self.layer_index < edit_state.buffer.layers.len() {
            self.layer = Some(edit_state.buffer.layers.remove(self.layer_index));
            edit_state.clamp_current_layer();
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer_index).into())
        }
    }
}

#[derive(Default)]
pub struct RaiseLayer {
    layer_index: usize,
}

impl RaiseLayer {
    pub fn new(layer_index: usize) -> Self {
        Self { layer_index }
    }
}

impl UndoOperation for RaiseLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-raise_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers.swap(self.layer_index, self.layer_index + 1);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers.swap(self.layer_index, self.layer_index + 1);
        Ok(())
    }
}

#[derive(Default)]
pub struct LowerLayer {
    layer_index: usize,
}

impl LowerLayer {
    pub fn new(layer_index: usize) -> Self {
        Self { layer_index }
    }
}

impl UndoOperation for LowerLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-lower_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers.swap(self.layer_index, self.layer_index - 1);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers.swap(self.layer_index, self.layer_index - 1);
        Ok(())
    }
}

#[derive(Default)]
pub struct MergeLayerDown {
    index: usize,
    merged_layer: Option<Layer>,
    orig_layers: Option<Vec<Layer>>,
}

impl MergeLayerDown {
    pub(crate) fn new(index: usize, merged_layer: Layer) -> Self {
        Self {
            index,
            merged_layer: Some(merged_layer),
            orig_layers: None,
        }
    }
}

impl UndoOperation for MergeLayerDown {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-merge_down_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(mut orig_layers) = self.orig_layers.take() {
            while let Some(layer) = orig_layers.pop() {
                edit_state.buffer.layers.insert(self.index - 1, layer);
            }
            self.merged_layer = Some(edit_state.buffer.layers.remove(self.index + 1));
            edit_state.set_current_layer(self.index);
            edit_state.clamp_current_layer();
            Ok(())
        } else {
            Err(EditorError::MergeLayerDownHasNoMergeLayer.into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = self.merged_layer.take() {
            self.orig_layers = Some(edit_state.buffer.layers.drain((self.index - 1)..=self.index).collect());
            edit_state.buffer.layers.insert(self.index - 1, layer);
            edit_state.set_current_layer(self.index - 1);
            Ok(())
        } else {
            Err(EditorError::MergeLayerDownHasNoMergeLayer.into())
        }
    }
}

#[derive(Default)]
pub struct ToggleLayerVisibility {
    index: usize,
}

impl ToggleLayerVisibility {
    pub(crate) fn new(index: usize) -> Self {
        Self { index }
    }
}

impl UndoOperation for ToggleLayerVisibility {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-toggle_layer_visibility")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            layer.properties.is_visible = !layer.properties.is_visible;
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            layer.properties.is_visible = !layer.properties.is_visible;
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }
}

#[derive(Default)]
pub struct MoveLayer {
    index: usize,
    from: Position,
    to: Position,
}

impl MoveLayer {
    pub(crate) fn new(index: usize, from: Position, to: Position) -> Self {
        Self { index, from, to }
    }
}

impl UndoOperation for MoveLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-move_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            layer.set_offset(self.from);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            layer.set_offset(self.to);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }
}

#[derive(Default)]
pub struct SetLayerSize {
    index: usize,
    from: Size,
    to: Size,
}

impl SetLayerSize {
    pub(crate) fn new(index: usize, to: Size) -> Self {
        Self { index, from: to, to }
    }
}

impl UndoOperation for SetLayerSize {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_layer_size")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            layer.set_size(self.from);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            self.from = layer.get_size();
            layer.set_size(self.to);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }
}

#[derive(Default)]
pub struct Paste {
    layer: Option<Layer>,
    current_layer: usize,
}

impl Paste {
    pub(crate) fn new(current_layer: usize, paste_layer: Layer) -> Self {
        Self {
            layer: Some(paste_layer),
            current_layer,
        }
    }
}

impl UndoOperation for Paste {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-paste")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        self.layer = Some(edit_state.buffer.layers.remove(self.current_layer + 1));
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = self.layer.take() {
            edit_state.buffer.layers.insert(self.current_layer + 1, layer);
            Ok(())
        } else {
            Err(EditorError::CurrentLayerInvalid.into())
        }
    }
}

#[derive(Default)]
pub struct AddFloatingLayer {
    current_layer: usize,
}

impl AddFloatingLayer {
    pub(crate) fn new(current_layer: usize) -> Self {
        Self { current_layer }
    }
}

impl UndoOperation for AddFloatingLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-add_floating_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.current_layer) {
            if matches!(layer.role, crate::Role::Image) {
                layer.role = crate::Role::PasteImage;
            } else {
                layer.role = crate::Role::PastePreview;
            }
            layer.properties.title = fl!(crate::LANGUAGE_LOADER, "layer-pasted-name");
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.current_layer) {
            if matches!(layer.role, crate::Role::PasteImage) {
                layer.role = crate::Role::Image;
            } else {
                layer.role = crate::Role::Normal;
            }
            layer.properties.title = fl!(crate::LANGUAGE_LOADER, "layer-new-name");
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct ResizeBuffer {
    orig_size: Size,
    size: Size,
}

impl ResizeBuffer {
    pub fn new(orig_size: impl Into<Size>, size: impl Into<Size>) -> Self {
        Self {
            orig_size: orig_size.into(),
            size: size.into(),
        }
    }
}

impl UndoOperation for ResizeBuffer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-resize_buffer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_size(self.orig_size);
        edit_state.set_mask_size();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_size(self.size);
        edit_state.set_mask_size();
        Ok(())
    }
}

#[derive(Clone)]
pub struct UndoLayerChange {
    pub layer: usize,
    pub pos: Position,
    pub old_chars: Layer,
    pub new_chars: Layer,
}

impl UndoLayerChange {
    pub fn new(layer: usize, pos: Position, old_chars: Layer, new_chars: Layer) -> Self {
        Self {
            layer,
            pos,
            old_chars,
            new_chars,
        }
    }
}

impl UndoOperation for UndoLayerChange {
    fn get_description(&self) -> String {
        String::new() // No stand alone operation.
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.layer) {
            if layer.get_size() == self.old_chars.get_size() {
                layer.lines = self.old_chars.lines.clone();
            } else {
                layer.stamp(self.pos, &self.old_chars);
            }
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.layer) {
            if layer.get_size() == self.new_chars.get_size() {
                layer.lines = self.new_chars.lines.clone();
            } else {
                layer.stamp(self.pos, &self.new_chars);
            }
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }
}

#[derive(Default)]
pub struct Crop {
    orig_size: Size,
    size: Size,
    layers: Vec<Layer>,
}

impl Crop {
    pub fn new(orig_size: impl Into<Size>, size: impl Into<Size>, layers: Vec<Layer>) -> Self {
        Self {
            orig_size: orig_size.into(),
            size: size.into(),
            layers,
        }
    }
}

impl UndoOperation for Crop {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-crop")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_size(self.orig_size);
        edit_state.set_mask_size();
        mem::swap(&mut edit_state.get_buffer_mut().layers, &mut self.layers);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_size(self.size);
        edit_state.set_mask_size();
        mem::swap(&mut edit_state.get_buffer_mut().layers, &mut self.layers);
        Ok(())
    }
}

#[derive(Default)]
pub struct DeleteRow {
    layer: usize,
    line: i32,
    deleted_row: Line,
}

impl DeleteRow {
    pub fn new(layer: usize, line: i32) -> Self {
        Self {
            layer,
            line,
            deleted_row: Line::default(),
        }
    }
}

impl UndoOperation for DeleteRow {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-delete_row")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let mut deleted_row = Line::default();
            mem::swap(&mut self.deleted_row, &mut deleted_row);
            layer.lines.insert(self.line as usize, deleted_row);
            layer.set_height(layer.get_height() + 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            if layer.lines.len() < self.line as usize + 1 {
                layer.lines.resize(self.line as usize + 1, Line::default());
            }
            self.deleted_row = layer.lines.remove(self.line as usize);
            layer.set_height(layer.get_height() - 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }
}

#[derive(Default)]
pub struct InsertRow {
    layer: usize,
    line: i32,
    inserted_row: Line,
}

impl InsertRow {
    pub fn new(layer: usize, line: i32) -> Self {
        Self {
            layer,
            line,
            inserted_row: Line::default(),
        }
    }
}

impl UndoOperation for InsertRow {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-insert_row")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            self.inserted_row = layer.lines.remove(self.line as usize);
            layer.set_height(layer.get_height() - 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let mut insert_row = Line::default();
            mem::swap(&mut self.inserted_row, &mut insert_row);
            if layer.lines.len() < self.line as usize + 1 {
                layer.lines.resize(self.line as usize + 1, Line::default());
            }
            layer.lines.insert(self.line as usize, insert_row);
            layer.set_height(layer.get_height() + 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }
}

#[derive(Default)]
pub struct DeleteColumn {
    layer: usize,
    column: i32,
    deleted_chars: Vec<Option<AttributedChar>>,
}

impl DeleteColumn {
    pub fn new(layer: usize, column: i32) -> Self {
        Self {
            layer,
            column,
            deleted_chars: Vec::new(),
        }
    }
}

impl UndoOperation for DeleteColumn {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-delete_column")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let offset: usize = self.column as usize;
            for (i, ch) in self.deleted_chars.iter().enumerate() {
                if let Some(ch) = ch {
                    layer.lines[i].chars.insert(offset, *ch);
                }
            }
            layer.set_width(layer.get_width() + 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let mut deleted_row = Vec::new();
            let offset: usize = self.column as usize;
            for line in &mut layer.lines {
                if offset < line.chars.len() {
                    deleted_row.push(Some(line.chars.remove(offset)));
                } else {
                    deleted_row.push(None);
                }
            }
            self.deleted_chars = deleted_row;
            layer.set_width(layer.get_width() - 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }
}

#[derive(Default)]
pub struct InsertColumn {
    layer: usize,
    column: i32,
}

impl InsertColumn {
    pub fn new(layer: usize, column: i32) -> Self {
        Self { layer, column }
    }
}

impl UndoOperation for InsertColumn {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-insert_column")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let offset: usize = self.column as usize;
            for line in &mut layer.lines {
                if line.chars.len() > offset {
                    line.chars.remove(offset);
                }
            }
            layer.set_width(layer.get_width() - 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let offset: usize = self.column as usize;
            for line in &mut layer.lines {
                if line.chars.len() >= offset {
                    line.chars.insert(offset, AttributedChar::invisible());
                }
            }
            layer.set_width(layer.get_width() + 1);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer).into())
        }
    }
}

mod scroll_util {
    use crate::{EngineResult, editor::EditorError};

    pub(crate) fn scroll_layer_up(edit_state: &mut crate::editor::EditState, layer: usize) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(layer) {
            let lines = layer.lines.remove(0);
            layer.lines.push(lines);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(layer).into())
        }
    }
    pub(crate) fn scroll_layer_down(edit_state: &mut crate::editor::EditState, layer: usize) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(layer) {
            if let Some(lines) = layer.lines.pop() {
                layer.lines.insert(0, lines);
            } else {
                log::error!("Layer {layer} has no lines");
            }
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(layer).into())
        }
    }
}

#[derive(Default)]
pub struct UndoScrollWholeLayerUp {
    layer: usize,
}

impl UndoScrollWholeLayerUp {
    pub fn new(layer: usize) -> Self {
        Self { layer }
    }
}

impl UndoOperation for UndoScrollWholeLayerUp {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-scroll_layer_up")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        scroll_util::scroll_layer_down(edit_state, self.layer)
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        scroll_util::scroll_layer_up(edit_state, self.layer)
    }
}

#[derive(Default)]
pub struct UndoScrollWholeLayerDown {
    layer: usize,
}

impl UndoScrollWholeLayerDown {
    pub fn new(layer: usize) -> Self {
        Self { layer }
    }
}

impl UndoOperation for UndoScrollWholeLayerDown {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-scroll_layer_down")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        scroll_util::scroll_layer_up(edit_state, self.layer)
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        scroll_util::scroll_layer_down(edit_state, self.layer)
    }
}

#[derive(Default)]
pub struct RotateLayer {
    layer: usize,
    old_lines: Vec<Line>,
    new_lines: Vec<Line>,
}

impl RotateLayer {
    pub fn new(layer: usize, old_lines: Vec<Line>, new_lines: Vec<Line>) -> Self {
        Self { layer, old_lines, new_lines }
    }
}

impl UndoOperation for RotateLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-rotate_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let size = layer.get_size();
            layer.set_size((size.height, size.width));
            layer.lines = self.old_lines.clone();
        }
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(self.layer) {
            let size = layer.get_size();
            layer.set_size((size.height, size.width));
            layer.lines = self.new_lines.clone();
        }
        Ok(())
    }
}

pub(crate) struct ReversedUndo {
    description: String,
    op: Box<dyn UndoOperation>,
    operation_type: OperationType,
}

impl ReversedUndo {
    pub(crate) fn new(description: String, op: Box<dyn UndoOperation>, operation_type: OperationType) -> Self {
        Self {
            description,
            op,
            operation_type,
        }
    }
}

impl UndoOperation for ReversedUndo {
    fn get_description(&self) -> String {
        self.description.clone()
    }

    fn get_operation_type(&self) -> OperationType {
        self.operation_type
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        self.op.redo(edit_state)
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        self.op.undo(edit_state)
    }
}

pub(crate) struct ReverseCaretPosition {
    pos: Position,
    old_pos: Position,
}

impl ReverseCaretPosition {
    pub(crate) fn new(pos: Position) -> Self {
        Self { pos, old_pos: pos }
    }
}

impl UndoOperation for ReverseCaretPosition {
    fn get_description(&self) -> String {
        "Reverse caret position".into()
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        self.old_pos = edit_state.caret.pos;
        edit_state.caret.pos = self.pos;
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.caret.pos = self.old_pos;
        Ok(())
    }
}

#[derive(Default)]
pub struct ClearLayer {
    layer_index: usize,
    layer: Vec<Line>,
}

impl ClearLayer {
    pub fn new(layer_index: usize) -> Self {
        Self {
            layer_index,
            layer: Vec::new(),
        }
    }
}

impl UndoOperation for ClearLayer {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-clear_layer")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        self.redo(edit_state)
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if self.layer_index < edit_state.buffer.layers.len() {
            mem::swap(&mut self.layer, &mut edit_state.buffer.layers[self.layer_index].lines);
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.layer_index).into())
        }
    }
}

#[derive(Default)]
pub struct Deselect {
    sel: Selection,
}

impl Deselect {
    pub fn new(sel: Selection) -> Self {
        Self { sel }
    }
}

impl UndoOperation for Deselect {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-deselect")
    }

    fn changes_data(&self) -> bool {
        false
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = Some(self.sel);
        edit_state.set_is_buffer_dirty();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = None;
        edit_state.set_is_buffer_dirty();
        Ok(())
    }
}

#[derive(Default)]
pub struct SelectNothing {
    sel: Option<Selection>,
    mask: SelectionMask,
}

impl SelectNothing {
    pub fn new(sel: Option<Selection>, mask: crate::SelectionMask) -> Self {
        Self { sel, mask }
    }
}

impl UndoOperation for SelectNothing {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-select-nothing")
    }

    fn changes_data(&self) -> bool {
        false
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = self.sel;
        edit_state.selection_mask = self.mask.clone();
        edit_state.set_is_buffer_dirty();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = None;
        edit_state.selection_mask.clear();
        edit_state.set_is_buffer_dirty();
        Ok(())
    }
}

#[derive(Default)]
pub struct SetSelection {
    old: Option<Selection>,
    new: Option<Selection>,
}

impl SetSelection {
    pub fn new(old: Option<Selection>, new: Option<Selection>) -> Self {
        Self { old, new }
    }
}

impl UndoOperation for SetSelection {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_selection")
    }

    fn changes_data(&self) -> bool {
        false
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = self.old;
        edit_state.set_is_buffer_dirty();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = self.new;
        edit_state.set_is_buffer_dirty();
        Ok(())
    }
}

#[derive(Default)]
pub struct SetSelectionMask {
    description: String,
    old: SelectionMask,
    new: SelectionMask,
}

impl SetSelectionMask {
    pub fn new(description: String, old: crate::SelectionMask, new: crate::SelectionMask) -> Self {
        Self { description, old, new }
    }
}

impl UndoOperation for SetSelectionMask {
    fn get_description(&self) -> String {
        self.description.clone()
    }

    fn changes_data(&self) -> bool {
        false
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_mask = self.old.clone();
        edit_state.set_is_buffer_dirty();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_mask = self.new.clone();
        edit_state.set_is_buffer_dirty();
        Ok(())
    }
}

#[derive(Default)]
pub struct AddSelectionToMask {
    old: SelectionMask,
    selection: Selection,
}

impl AddSelectionToMask {
    pub fn new(old: crate::SelectionMask, selection: Selection) -> Self {
        Self { old, selection }
    }
}

impl UndoOperation for AddSelectionToMask {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_selection")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_mask = self.old.clone();
        edit_state.set_is_buffer_dirty();
        Ok(())
    }

    fn changes_data(&self) -> bool {
        false
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        match self.selection.add_type {
            AddType::Default | AddType::Add => {
                edit_state.selection_mask.add_selection(self.selection);
            }
            AddType::Subtract => {
                edit_state.selection_mask.remove_selection(self.selection);
            }
        }
        edit_state.set_is_buffer_dirty();
        Ok(())
    }
}

#[derive(Default)]
pub struct InverseSelection {
    sel: Option<Selection>,
    old: SelectionMask,
    new: SelectionMask,
}

impl InverseSelection {
    pub fn new(sel: Option<Selection>, old: crate::SelectionMask, new: crate::SelectionMask) -> Self {
        Self { sel, old, new }
    }
}

impl UndoOperation for InverseSelection {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-inverse_selection")
    }

    fn changes_data(&self) -> bool {
        false
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = self.sel;
        edit_state.selection_mask = self.old.clone();
        edit_state.set_is_buffer_dirty();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.selection_opt = None;
        edit_state.selection_mask = self.new.clone();
        edit_state.set_is_buffer_dirty();
        Ok(())
    }
}

#[derive(Default)]
pub struct SwitchPalettte {
    pal: Palette,
}

impl SwitchPalettte {
    pub fn new(pal: Palette) -> Self {
        Self { pal }
    }
}

impl UndoOperation for SwitchPalettte {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-switch_palette")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        mem::swap(&mut edit_state.get_buffer_mut().palette, &mut self.pal);
        edit_state.is_palette_dirty = true;
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        mem::swap(&mut edit_state.get_buffer_mut().palette, &mut self.pal);
        edit_state.is_palette_dirty = true;
        Ok(())
    }
}

#[derive(Default)]
pub struct SetSauceData {
    new: SauceMetaInformation,
    old: SauceMetaInformation,
}

impl SetSauceData {
    pub fn new(new: SauceMetaInformation, old: SauceMetaInformation) -> Self {
        Self { new, old }
    }
}

impl UndoOperation for SetSauceData {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-change_sauce")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_sauce_meta(self.old.clone());
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_sauce_meta(self.new.clone());
        Ok(())
    }
}

#[derive(Default)]
pub struct SwitchToFontPage {
    old: usize,
    new: usize,
}

impl SwitchToFontPage {
    pub fn new(old: usize, new: usize) -> Self {
        Self { old, new }
    }
}

impl UndoOperation for SwitchToFontPage {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-switch_font_page")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.caret.set_font_page(self.old);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.caret.set_font_page(self.new);
        Ok(())
    }
}

#[derive(Default)]
pub struct SetFont {
    font_page: usize,
    old: BitFont,
    new: BitFont,
}

impl SetFont {
    pub fn new(font_page: usize, old: BitFont, new: BitFont) -> Self {
        Self { font_page, old, new }
    }
}

impl UndoOperation for SetFont {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-switch_font_page")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_font(self.font_page, self.old.clone());
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.get_buffer_mut().set_font(self.font_page, self.new.clone());
        Ok(())
    }
}

#[derive(Default)]
pub struct AddFont {
    old_font_page: usize,
    new_font_page: usize,
    font: BitFont,
}

impl AddFont {
    pub fn new(old_font_page: usize, new_font_page: usize, font: BitFont) -> Self {
        Self {
            old_font_page,
            new_font_page,
            font,
        }
    }
}

impl UndoOperation for AddFont {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-switch_font_page")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.remove_font(self.new_font_page);
        edit_state.caret.set_font_page(self.old_font_page);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.set_font(self.new_font_page, self.font.clone());
        edit_state.caret.set_font_page(self.new_font_page);
        Ok(())
    }
}

pub struct SwitchPalette {
    old_mode: PaletteMode,
    old_palette: Palette,
    old_layers: Vec<Layer>,

    new_mode: PaletteMode,
    new_palette: Palette,
    new_layers: Vec<Layer>,
}

impl SwitchPalette {
    pub fn new(
        old_mode: PaletteMode,
        old_palette: Palette,
        old_layers: Vec<Layer>,
        new_mode: PaletteMode,
        new_palette: Palette,
        new_layers: Vec<Layer>,
    ) -> Self {
        Self {
            old_mode,
            old_palette,
            old_layers,

            new_mode,
            new_palette,
            new_layers,
        }
    }
}

impl UndoOperation for SwitchPalette {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-switch_palette_mode")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.palette = self.old_palette.clone();
        edit_state.buffer.palette_mode = self.old_mode;
        edit_state.buffer.layers = self.old_layers.clone();
        edit_state.is_palette_dirty = true;
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.palette = self.new_palette.clone();
        edit_state.buffer.palette_mode = self.new_mode;
        edit_state.buffer.layers = self.new_layers.clone();
        edit_state.is_palette_dirty = true;
        Ok(())
    }
}

pub struct SetIceMode {
    old_mode: IceMode,
    old_layers: Vec<Layer>,
    new_mode: IceMode,
    new_layers: Vec<Layer>,
}

impl SetIceMode {
    pub fn new(old_mode: IceMode, old_layers: Vec<Layer>, new_mode: IceMode, new_layers: Vec<Layer>) -> Self {
        Self {
            old_mode,
            old_layers,
            new_mode,
            new_layers,
        }
    }
}

impl UndoOperation for SetIceMode {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-switch_ice_mode")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers = self.old_layers.clone();
        edit_state.buffer.ice_mode = self.old_mode;
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers = self.new_layers.clone();
        edit_state.buffer.ice_mode = self.new_mode;
        Ok(())
    }
}

pub struct ReplaceFontUsage {
    old_caret_page: usize,
    old_layers: Vec<Layer>,
    new_caret_page: usize,
    new_layers: Vec<Layer>,
}

impl ReplaceFontUsage {
    pub fn new(old_caret_page: usize, old_layers: Vec<Layer>, new_caret_page: usize, new_layers: Vec<Layer>) -> Self {
        Self {
            old_caret_page,
            old_layers,
            new_caret_page,
            new_layers,
        }
    }
}

impl UndoOperation for ReplaceFontUsage {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-replace_font")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers = self.old_layers.clone();
        edit_state.get_caret_mut().set_font_page(self.old_caret_page);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.layers = self.new_layers.clone();
        edit_state.get_caret_mut().set_font_page(self.new_caret_page);
        Ok(())
    }
}

pub struct RemoveFont {
    font_slot: usize,
    font: Option<BitFont>,
}

impl RemoveFont {
    pub fn new(font_slot: usize) -> Self {
        Self { font_slot, font: None }
    }
}

impl UndoOperation for RemoveFont {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-remove_font")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(font) = self.font.take() {
            edit_state.buffer.set_font(self.font_slot, font);
            Ok(())
        } else {
            Err(anyhow::anyhow!("no font for RemoveFont undo."))
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        self.font = edit_state.buffer.remove_font(self.font_slot);
        if self.font.is_some() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("empty font slot."))
        }
    }
}

pub struct ChangeFontSlot {
    from: usize,
    to: usize,
}

impl ChangeFontSlot {
    pub fn new(from: usize, to: usize) -> Self {
        Self { from, to }
    }
}

impl UndoOperation for ChangeFontSlot {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-change_font_slot")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        let font = edit_state.buffer.remove_font(self.to);
        if let Some(font) = font {
            edit_state.buffer.set_font(self.from, font);
            Ok(())
        } else {
            Err(anyhow::anyhow!("empty font slot."))
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        let font = edit_state.buffer.remove_font(self.from);
        if let Some(font) = font {
            edit_state.buffer.set_font(self.to, font);
            Ok(())
        } else {
            Err(anyhow::anyhow!("empty font slot."))
        }
    }
}

pub struct UpdateLayerProperties {
    index: usize,
    old_properties: Properties,
    new_properties: Properties,
}

impl UpdateLayerProperties {
    pub fn new(index: usize, old_properties: Properties, new_properties: Properties) -> Self {
        Self {
            index,
            old_properties,
            new_properties,
        }
    }
}

impl UndoOperation for UpdateLayerProperties {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-update_layer_properties")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            layer.properties = self.old_properties.clone();
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(layer) = edit_state.buffer.layers.get_mut(self.index) {
            layer.properties = self.new_properties.clone();
            Ok(())
        } else {
            Err(EditorError::InvalidLayer(self.index).into())
        }
    }
}

pub struct SetUseLetterSpacing {
    new_ls: bool,
}

impl SetUseLetterSpacing {
    pub fn new(new_ls: bool) -> Self {
        Self { new_ls }
    }
}

impl UndoOperation for SetUseLetterSpacing {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_use_letter_spacing")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.set_use_letter_spacing(!self.new_ls);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.set_use_letter_spacing(self.new_ls);
        Ok(())
    }
}

pub struct SetUseAspectRatio {
    new_ar: bool,
}

impl SetUseAspectRatio {
    pub fn new(new_ar: bool) -> Self {
        Self { new_ar }
    }
}

impl UndoOperation for SetUseAspectRatio {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_use_aspect_ratio")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.set_use_aspect_ratio(!self.new_ar);
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.set_use_aspect_ratio(self.new_ar);
        Ok(())
    }
}

pub struct AddTag {
    new_tag: Tag,
}

impl AddTag {
    pub fn new(new_ar: Tag) -> Self {
        Self { new_tag: new_ar }
    }
}

impl UndoOperation for AddTag {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_use_aspect_ratio")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.tags.pop();
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.tags.push(self.new_tag.clone());
        Ok(())
    }
}

pub struct EditTag {
    tag_index: usize,
    old_tag: Tag,
    new_tag: Tag,
}

impl EditTag {
    pub fn new(tag_index: usize, old_tag: Tag, new_tag: Tag) -> Self {
        Self { tag_index, old_tag, new_tag }
    }
}

impl UndoOperation for EditTag {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-edit-tag")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(tag) = edit_state.buffer.tags.get_mut(self.tag_index) {
            *tag = self.old_tag.clone();
            Ok(())
        } else {
            Err(anyhow::anyhow!("tag not found"))
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(tag) = edit_state.buffer.tags.get_mut(self.tag_index) {
            *tag = self.new_tag.clone();
            Ok(())
        } else {
            Err(anyhow::anyhow!("tag not found"))
        }
    }
}

pub struct MoveTag {
    tag: usize,
    new_pos: Position,
    old_pos: Position,
}

impl MoveTag {
    pub fn new(tag: usize, old_pos: Position, new_pos: Position) -> Self {
        Self { tag, new_pos, old_pos }
    }
}

impl UndoOperation for MoveTag {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-set_use_aspect_ratio")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(tag) = edit_state.buffer.tags.get_mut(self.tag) {
            tag.position = self.old_pos;
            Ok(())
        } else {
            Err(anyhow::anyhow!("tag not found"))
        }
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        if let Some(tag) = edit_state.buffer.tags.get_mut(self.tag) {
            tag.position = self.new_pos;
            Ok(())
        } else {
            Err(anyhow::anyhow!("tag not found"))
        }
    }
}

pub struct RemoveTag {
    tag_index: usize,
    tag: Tag,
}

impl RemoveTag {
    pub fn new(tag_index: usize, tag: Tag) -> Self {
        Self { tag_index, tag }
    }
}

impl UndoOperation for RemoveTag {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-remove-tag")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.tags.insert(self.tag_index, self.tag.clone());
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.tags.remove(self.tag_index);
        Ok(())
    }
}

pub struct ShowTags {
    show: bool,
}

impl ShowTags {
    pub fn new(show: bool) -> Self {
        Self { show }
    }
}

impl UndoOperation for ShowTags {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-show-tags")
    }

    fn undo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.show_tags = !self.show;
        Ok(())
    }

    fn redo(&mut self, edit_state: &mut EditState) -> EngineResult<()> {
        edit_state.buffer.show_tags = self.show;
        Ok(())
    }
}
