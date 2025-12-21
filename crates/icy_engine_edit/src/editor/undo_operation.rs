//! Editor undo operations as serializable enum
//!
//! This module provides a single enum containing all editor undo operations,
//! making them serializable for session persistence.

use serde::{Deserialize, Serialize};

use crate::{
    AttributedChar, BitFont, EngineError, IceMode, Layer, Line, Palette, Position, Properties, Result, Role, SauceMetaData, Selection, SelectionMask, Size,
    Tag, TextPane, stamp_layer,
};

use super::EditState;
use super::undo_stack::OperationType;

/// Serializable editor undo operation enum
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EditorUndoOp {
    /// Atomic group of operations
    Atomic {
        description: String,
        operations: Vec<EditorUndoOp>,
        operation_type: OperationType,
    },

    /// Set a single character
    SetChar {
        pos: Position,
        layer: usize,
        old: AttributedChar,
        new: AttributedChar,
        /// Caret position to restore on undo (None = don't move caret)
        #[serde(default)]
        undo_caret: Option<Position>,
        /// Caret position to restore on redo (None = don't move caret)
        #[serde(default)]
        redo_caret: Option<Position>,
    },

    /// Swap two characters
    SwapChar { layer: usize, pos1: Position, pos2: Position },

    /// Add a new layer
    AddLayer { index: usize, layer: Option<Layer> },

    /// Remove a layer
    RemoveLayer { layer_index: usize, layer: Option<Layer> },

    /// Raise layer in stack
    RaiseLayer { layer_index: usize },

    /// Lower layer in stack
    LowerLayer { layer_index: usize },

    /// Merge layer down
    MergeLayerDown {
        index: usize,
        merged_layer: Option<Layer>,
        orig_layers: Option<Vec<Layer>>,
    },

    /// Toggle layer visibility
    ToggleLayerVisibility { index: usize },

    /// Move layer position
    MoveLayer { index: usize, from: Position, to: Position },

    /// Set layer size
    SetLayerSize { index: usize, from: Size, to: Size },

    /// Paste operation
    Paste { layer: Option<Layer>, current_layer: usize },

    /// Add floating layer
    AddFloatingLayer { current_layer: usize },

    /// Resize buffer
    ResizeBuffer { orig_size: Size, size: Size },

    /// Layer change (bulk character change)
    LayerChange {
        layer: usize,
        pos: Position,
        old_chars: Layer,
        new_chars: Layer,
    },

    /// Crop buffer
    Crop { orig_size: Size, size: Size, layers: Vec<Layer> },

    /// Delete row
    DeleteRow { layer: usize, line: i32, deleted_row: Line },

    /// Insert row
    InsertRow { layer: usize, line: i32, inserted_row: Line },

    /// Delete column
    DeleteColumn {
        layer: usize,
        column: i32,
        deleted_chars: Vec<Option<AttributedChar>>,
    },

    /// Insert column
    InsertColumn { layer: usize, column: i32 },

    /// Scroll whole layer up
    ScrollWholeLayerUp { layer: usize },

    /// Scroll whole layer down
    ScrollWholeLayerDown { layer: usize },

    /// Rotate floating paste layer (for collaboration: Moebius ROTATE=18)
    PasteRotate {
        layer: usize,
        old_lines: Vec<Line>,
        new_lines: Vec<Line>,
        old_size: Size,
        new_size: Size,
    },

    /// Flip floating paste layer horizontally (for collaboration: Moebius FLIP_X=19)
    PasteFlipX { layer: usize, old_lines: Vec<Line>, new_lines: Vec<Line> },

    /// Flip floating paste layer vertically (for collaboration: Moebius FLIP_Y=20)
    PasteFlipY { layer: usize, old_lines: Vec<Line>, new_lines: Vec<Line> },

    /// Anchor floating paste layer - generates DRAW commands for all blocks
    PasteAnchor { x: i32, y: i32, blocks: crate::collaboration::Blocks },

    /// Set background color
    SetBackground { old_value: u32, new_value: u32 },

    /// Reversed undo (wraps another operation)
    Reversed {
        description: String,
        op: Box<EditorUndoOp>,
        operation_type: OperationType,
    },

    /// Reverse caret position
    ReverseCaretPosition { pos: Position, old_pos: Position },

    /// Clear layer
    ClearLayer { layer_index: usize, layer: Vec<Line> },

    /// Deselect
    Deselect { sel: Selection },

    /// Select nothing (clear selection and mask)
    SelectNothing { sel: Option<Selection>, mask: SelectionMask },

    /// Set selection
    SetSelection { old: Option<Selection>, new: Option<Selection> },

    /// Set selection mask
    SetSelectionMask {
        description: String,
        old: SelectionMask,
        new: SelectionMask,
    },

    /// Add selection to mask
    AddSelectionToMask { old: SelectionMask, selection: Selection },

    /// Inverse selection
    InverseSelection {
        sel: Option<Selection>,
        old: SelectionMask,
        new: SelectionMask,
    },

    /// Switch palette (legacy)
    SwitchPalettte { pal: Palette },

    /// Set SAUCE metadata
    SetSauceData { new: SauceMetaData, old: SauceMetaData },

    /// Switch to font page
    SwitchToFontPage { old: u8, new: u8 },

    /// Set font
    SetFont { font_page: u8, old: BitFont, new: BitFont },

    /// Add font
    AddFont { old_font_page: u8, new_font_page: u8, font: BitFont },

    /// Switch palette mode
    SwitchPalette {
        old_palette: Palette,
        old_layers: Vec<Layer>,
        new_palette: Palette,
        new_layers: Vec<Layer>,
    },

    /// Set ICE mode
    SetIceMode {
        old_mode: IceMode,
        old_layers: Vec<Layer>,
        new_mode: IceMode,
        new_layers: Vec<Layer>,
    },

    /// Replace font usage
    ReplaceFontUsage {
        old_caret_page: u8,
        old_layers: Vec<Layer>,
        new_caret_page: u8,
        new_layers: Vec<Layer>,
    },

    /// Remove font
    RemoveFont { font_slot: u8, font: Option<BitFont> },

    /// Change font slot
    ChangeFontSlot { from: u8, to: u8 },

    /// Update layer properties
    UpdateLayerProperties {
        index: usize,
        old_properties: Properties,
        new_properties: Properties,
    },

    /// Set use letter spacing
    SetUseLetterSpacing { new_ls: bool },

    /// Set use aspect ratio
    SetUseAspectRatio { new_ar: bool },

    /// Set font dimensions
    SetFontDimensions { old_size: Size, new_size: Size },

    /// Add tag
    AddTag { new_tag: Tag, clone: bool },

    /// Edit tag
    EditTag { tag_index: usize, old_tag: Tag, new_tag: Tag },

    /// Move tag
    MoveTag { tag: usize, new_pos: Position, old_pos: Position },

    /// Remove tag
    RemoveTag { tag_index: usize, tag: Tag },

    /// Show/hide tags
    ShowTags { show: bool },
}

impl EditorUndoOp {
    /// Get a description of this operation for UI display
    pub fn get_description(&self) -> String {
        use i18n_embed_fl::fl;
        match self {
            EditorUndoOp::Atomic { description, .. } => description.clone(),
            EditorUndoOp::SetChar { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_char"),
            EditorUndoOp::SwapChar { .. } => String::new(),
            EditorUndoOp::AddLayer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-add_layer"),
            EditorUndoOp::RemoveLayer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-remove_layer"),
            EditorUndoOp::RaiseLayer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-raise_layer"),
            EditorUndoOp::LowerLayer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-lower_layer"),
            EditorUndoOp::MergeLayerDown { .. } => fl!(crate::LANGUAGE_LOADER, "undo-merge_down_layer"),
            EditorUndoOp::ToggleLayerVisibility { .. } => fl!(crate::LANGUAGE_LOADER, "undo-toggle_layer_visibility"),
            EditorUndoOp::MoveLayer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-move_layer"),
            EditorUndoOp::SetLayerSize { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_layer_size"),
            EditorUndoOp::Paste { .. } => fl!(crate::LANGUAGE_LOADER, "undo-paste"),
            EditorUndoOp::AddFloatingLayer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-add_floating_layer"),
            EditorUndoOp::ResizeBuffer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-resize_buffer"),
            EditorUndoOp::LayerChange { .. } => String::new(),
            EditorUndoOp::Crop { .. } => fl!(crate::LANGUAGE_LOADER, "undo-crop"),
            EditorUndoOp::DeleteRow { .. } => fl!(crate::LANGUAGE_LOADER, "undo-delete_row"),
            EditorUndoOp::InsertRow { .. } => fl!(crate::LANGUAGE_LOADER, "undo-insert_row"),
            EditorUndoOp::DeleteColumn { .. } => fl!(crate::LANGUAGE_LOADER, "undo-delete_column"),
            EditorUndoOp::InsertColumn { .. } => fl!(crate::LANGUAGE_LOADER, "undo-insert_column"),
            EditorUndoOp::ScrollWholeLayerUp { .. } => fl!(crate::LANGUAGE_LOADER, "undo-scroll_layer_up"),
            EditorUndoOp::ScrollWholeLayerDown { .. } => fl!(crate::LANGUAGE_LOADER, "undo-scroll_layer_down"),
            EditorUndoOp::PasteRotate { .. } => fl!(crate::LANGUAGE_LOADER, "undo-rotate_layer"),
            EditorUndoOp::PasteFlipX { .. } => fl!(crate::LANGUAGE_LOADER, "undo-flip_layer_x"),
            EditorUndoOp::PasteFlipY { .. } => fl!(crate::LANGUAGE_LOADER, "undo-flip_layer_y"),
            EditorUndoOp::PasteAnchor { .. } => fl!(crate::LANGUAGE_LOADER, "undo-anchor"),
            EditorUndoOp::SetBackground { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_background"),
            EditorUndoOp::Reversed { description, .. } => description.clone(),
            EditorUndoOp::ReverseCaretPosition { .. } => "Reverse caret position".into(),
            EditorUndoOp::ClearLayer { .. } => fl!(crate::LANGUAGE_LOADER, "undo-clear_layer"),
            EditorUndoOp::Deselect { .. } => fl!(crate::LANGUAGE_LOADER, "undo-deselect"),
            EditorUndoOp::SelectNothing { .. } => fl!(crate::LANGUAGE_LOADER, "undo-select-nothing"),
            EditorUndoOp::SetSelection { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_selection"),
            EditorUndoOp::SetSelectionMask { description, .. } => description.clone(),
            EditorUndoOp::AddSelectionToMask { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_selection"),
            EditorUndoOp::InverseSelection { .. } => fl!(crate::LANGUAGE_LOADER, "undo-inverse_selection"),
            EditorUndoOp::SwitchPalettte { .. } => fl!(crate::LANGUAGE_LOADER, "undo-switch_palette"),
            EditorUndoOp::SetSauceData { .. } => fl!(crate::LANGUAGE_LOADER, "undo-change_sauce"),
            EditorUndoOp::SwitchToFontPage { .. } => fl!(crate::LANGUAGE_LOADER, "undo-switch_font_page"),
            EditorUndoOp::SetFont { .. } => fl!(crate::LANGUAGE_LOADER, "undo-switch_font_page"),
            EditorUndoOp::AddFont { .. } => fl!(crate::LANGUAGE_LOADER, "undo-switch_font_page"),
            EditorUndoOp::SwitchPalette { .. } => fl!(crate::LANGUAGE_LOADER, "undo-switch_palette_mode"),
            EditorUndoOp::SetIceMode { .. } => fl!(crate::LANGUAGE_LOADER, "undo-switch_ice_mode"),
            EditorUndoOp::ReplaceFontUsage { .. } => fl!(crate::LANGUAGE_LOADER, "undo-replace_font"),
            EditorUndoOp::RemoveFont { .. } => fl!(crate::LANGUAGE_LOADER, "undo-remove_font"),
            EditorUndoOp::ChangeFontSlot { .. } => fl!(crate::LANGUAGE_LOADER, "undo-change_font_slot"),
            EditorUndoOp::UpdateLayerProperties { .. } => fl!(crate::LANGUAGE_LOADER, "undo-update_layer_properties"),
            EditorUndoOp::SetUseLetterSpacing { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_use_letter_spacing"),
            EditorUndoOp::SetUseAspectRatio { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_use_aspect_ratio"),
            EditorUndoOp::SetFontDimensions { .. } => fl!(crate::LANGUAGE_LOADER, "undo-set_font_dimensions"),
            EditorUndoOp::AddTag { clone, .. } => {
                if *clone {
                    fl!(crate::LANGUAGE_LOADER, "undo-clone-tag")
                } else {
                    fl!(crate::LANGUAGE_LOADER, "undo-add-tag")
                }
            }
            EditorUndoOp::EditTag { .. } => fl!(crate::LANGUAGE_LOADER, "undo-edit-tag"),
            EditorUndoOp::MoveTag { .. } => fl!(crate::LANGUAGE_LOADER, "undo-move-tag"),
            EditorUndoOp::RemoveTag { .. } => fl!(crate::LANGUAGE_LOADER, "undo-remove-tag"),
            EditorUndoOp::ShowTags { .. } => fl!(crate::LANGUAGE_LOADER, "undo-show-tags"),
        }
    }

    /// Get the operation type for grouping
    pub fn get_operation_type(&self) -> OperationType {
        match self {
            EditorUndoOp::Atomic { operation_type, .. } => *operation_type,
            EditorUndoOp::Reversed { operation_type, .. } => *operation_type,
            _ => OperationType::Unknown,
        }
    }

    /// Clone this operation (for compatibility with old API)
    pub fn try_clone(&self) -> Option<EditorUndoOp> {
        Some(self.clone())
    }

    /// Whether this operation changes data (affects dirty flag)
    pub fn changes_data(&self) -> bool {
        match self {
            EditorUndoOp::Atomic { operations, .. } => operations.iter().any(|op| op.changes_data()),
            EditorUndoOp::Deselect { .. }
            | EditorUndoOp::SelectNothing { .. }
            | EditorUndoOp::SetSelection { .. }
            | EditorUndoOp::SetSelectionMask { .. }
            | EditorUndoOp::AddSelectionToMask { .. }
            | EditorUndoOp::InverseSelection { .. } => false,
            _ => true,
        }
    }

    /// Perform the undo operation
    pub fn undo(&mut self, edit_state: &mut EditState) -> Result<()> {
        match self {
            EditorUndoOp::Atomic { operations, .. } => {
                for op in operations.iter_mut().rev() {
                    op.undo(edit_state)?;
                }
                Ok(())
            }
            EditorUndoOp::SetChar {
                pos, layer, old, undo_caret, ..
            } => {
                edit_state.get_buffer_mut().layers[*layer].set_char(*pos, *old);
                if let Some(caret_pos) = undo_caret {
                    edit_state.set_caret_position(*caret_pos);
                }
                Ok(())
            }
            EditorUndoOp::SwapChar { layer, pos1, pos2 } => {
                edit_state.get_buffer_mut().layers[*layer].swap_char(*pos1, *pos2);
                Ok(())
            }
            EditorUndoOp::AddLayer { index, layer } => {
                *layer = Some(edit_state.get_buffer_mut().layers.remove(*index));
                edit_state.clamp_current_layer();
                Ok(())
            }
            EditorUndoOp::RemoveLayer { layer_index, layer } => {
                if let Some(l) = layer.take() {
                    edit_state.get_buffer_mut().layers.insert(*layer_index, l);
                }
                Ok(())
            }
            EditorUndoOp::RaiseLayer { layer_index } => {
                edit_state.get_buffer_mut().layers.swap(*layer_index, *layer_index + 1);
                Ok(())
            }
            EditorUndoOp::LowerLayer { layer_index } => {
                edit_state.get_buffer_mut().layers.swap(*layer_index, *layer_index - 1);
                Ok(())
            }
            EditorUndoOp::MergeLayerDown {
                index,
                merged_layer,
                orig_layers,
            } => {
                if let Some(mut layers) = orig_layers.take() {
                    while let Some(layer) = layers.pop() {
                        edit_state.get_buffer_mut().layers.insert(*index - 1, layer);
                    }
                    *merged_layer = Some(edit_state.get_buffer_mut().layers.remove(*index + 1));
                    edit_state.set_current_layer(*index);
                    edit_state.clamp_current_layer();
                }
                Ok(())
            }
            EditorUndoOp::ToggleLayerVisibility { index } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*index) {
                    layer.properties.is_visible = !layer.properties.is_visible;
                }
                Ok(())
            }
            EditorUndoOp::MoveLayer { index, from, .. } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*index) {
                    layer.set_offset(*from);
                }
                Ok(())
            }
            EditorUndoOp::SetLayerSize { index, from, .. } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*index) {
                    layer.set_size(*from);
                }
                Ok(())
            }
            EditorUndoOp::Paste { layer, current_layer } => {
                *layer = Some(edit_state.get_buffer_mut().layers.remove(*current_layer + 1));
                edit_state.set_current_layer(*current_layer);
                Ok(())
            }
            EditorUndoOp::AddFloatingLayer { current_layer } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*current_layer) {
                    layer.properties.title = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "layer-pasted-name");
                }
                Ok(())
            }
            EditorUndoOp::ResizeBuffer { orig_size, .. } => {
                edit_state.get_buffer_mut().set_size(*orig_size);
                edit_state.set_mask_size();
                Ok(())
            }
            EditorUndoOp::LayerChange {
                layer,
                pos,
                old_chars,
                new_chars: _,
            } => {
                if let Some(target_layer) = edit_state.get_buffer_mut().layers.get_mut(*layer) {
                    if target_layer.size() == old_chars.size() {
                        target_layer.lines = old_chars.lines.clone();
                    } else {
                        stamp_layer(target_layer, *pos, old_chars);
                    }
                }
                Ok(())
            }
            EditorUndoOp::Crop { orig_size, layers, .. } => {
                edit_state.get_buffer_mut().set_size(*orig_size);
                std::mem::swap(layers, &mut edit_state.get_buffer_mut().layers);
                edit_state.set_mask_size();
                Ok(())
            }
            EditorUndoOp::DeleteRow { layer, line, deleted_row } => {
                edit_state.get_buffer_mut().layers[*layer].lines.insert(*line as usize, deleted_row.clone());
                Ok(())
            }
            EditorUndoOp::InsertRow { layer, line, .. } => {
                edit_state.get_buffer_mut().layers[*layer].lines.remove(*line as usize);
                Ok(())
            }
            EditorUndoOp::DeleteColumn { layer, column, deleted_chars } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                for (i, ch) in deleted_chars.iter().enumerate() {
                    if let Some(c) = ch {
                        if i < layer.lines.len() {
                            layer.lines[i].insert_char(*column, *c);
                        }
                    }
                }
                let new_width = layer.width() + 1;
                layer.set_size((new_width, layer.height()));
                Ok(())
            }
            EditorUndoOp::InsertColumn { layer, column } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                for line in &mut layer.lines {
                    if (*column as usize) < line.chars.len() {
                        line.chars.remove(*column as usize);
                    }
                }
                let new_width = layer.width() - 1;
                layer.set_size((new_width, layer.height()));
                Ok(())
            }
            EditorUndoOp::ScrollWholeLayerUp { layer } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                if let Some(line) = layer.lines.pop() {
                    layer.lines.insert(0, line);
                }
                Ok(())
            }
            EditorUndoOp::ScrollWholeLayerDown { layer } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                if !layer.lines.is_empty() {
                    let line = layer.lines.remove(0);
                    layer.lines.push(line);
                }
                Ok(())
            }
            EditorUndoOp::PasteRotate {
                layer,
                old_lines,
                new_lines,
                old_size,
                new_size,
            } => {
                std::mem::swap(old_lines, new_lines);
                std::mem::swap(old_size, new_size);
                let l = &mut edit_state.get_buffer_mut().layers[*layer];
                l.lines = new_lines.clone();
                l.set_size(*new_size);
                Ok(())
            }
            EditorUndoOp::PasteFlipX { layer, old_lines, new_lines } => {
                std::mem::swap(old_lines, new_lines);
                edit_state.get_buffer_mut().layers[*layer].lines = new_lines.clone();
                Ok(())
            }
            EditorUndoOp::PasteFlipY { layer, old_lines, new_lines } => {
                std::mem::swap(old_lines, new_lines);
                edit_state.get_buffer_mut().layers[*layer].lines = new_lines.clone();
                Ok(())
            }
            EditorUndoOp::PasteAnchor { .. } => {
                // PasteAnchor is only for collaboration sync, local undo is handled by atomic group
                Ok(())
            }
            EditorUndoOp::SetBackground { old_value, .. } => {
                // For now, just set ice_mode based on whether background is non-zero
                // This is a simplification - actual background color handling may need more work
                if *old_value > 0 {
                    edit_state.get_buffer_mut().ice_mode = icy_engine::IceMode::Ice;
                }
                Ok(())
            }
            EditorUndoOp::Reversed { op, .. } => op.redo(edit_state),
            EditorUndoOp::ReverseCaretPosition { old_pos, .. } => {
                edit_state.get_caret_mut().set_position(*old_pos);
                Ok(())
            }
            EditorUndoOp::ClearLayer { layer_index, layer } => {
                std::mem::swap(layer, &mut edit_state.get_buffer_mut().layers[*layer_index].lines);
                Ok(())
            }
            EditorUndoOp::Deselect { sel } => {
                edit_state.selection_opt = Some(sel.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SelectNothing { sel, mask } => {
                edit_state.selection_opt = sel.clone();
                edit_state.set_selection_mask(mask.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SetSelection { old, .. } => {
                edit_state.selection_opt = old.clone();
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SetSelectionMask { old, .. } => {
                edit_state.set_selection_mask(old.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::AddSelectionToMask { old, .. } => {
                edit_state.set_selection_mask(old.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::InverseSelection { sel, old, new } => {
                edit_state.selection_opt = sel.clone();
                std::mem::swap(old, new);
                edit_state.set_selection_mask(new.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SwitchPalettte { pal } => {
                std::mem::swap(pal, &mut edit_state.get_buffer_mut().palette);
                Ok(())
            }
            EditorUndoOp::SetSauceData { old, new } => {
                std::mem::swap(old, new);
                edit_state.set_sauce_meta(new.clone().into());
                Ok(())
            }
            EditorUndoOp::SwitchToFontPage { old, new } => {
                std::mem::swap(old, new);
                edit_state.get_caret_mut().set_font_page(*new);
                Ok(())
            }
            EditorUndoOp::SetFont { font_page, old, new } => {
                std::mem::swap(old, new);
                edit_state.get_buffer_mut().set_font(*font_page, new.clone());
                Ok(())
            }
            EditorUndoOp::AddFont {
                old_font_page, new_font_page, ..
            } => {
                edit_state.get_caret_mut().set_font_page(*old_font_page);
                edit_state.get_buffer_mut().remove_font(*new_font_page);
                Ok(())
            }
            EditorUndoOp::SwitchPalette {
                old_palette,
                old_layers,
                new_palette,
                new_layers,
            } => {
                std::mem::swap(old_palette, new_palette);
                std::mem::swap(old_layers, new_layers);
                let buf = edit_state.get_buffer_mut();
                buf.palette = new_palette.clone();
                buf.layers = new_layers.clone();
                buf.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SetIceMode {
                old_mode,
                old_layers,
                new_mode,
                new_layers,
            } => {
                std::mem::swap(old_mode, new_mode);
                std::mem::swap(old_layers, new_layers);
                edit_state.get_buffer_mut().ice_mode = *new_mode;
                edit_state.get_buffer_mut().layers = new_layers.clone();
                Ok(())
            }
            EditorUndoOp::ReplaceFontUsage {
                old_caret_page,
                old_layers,
                new_caret_page,
                new_layers,
            } => {
                std::mem::swap(old_caret_page, new_caret_page);
                std::mem::swap(old_layers, new_layers);
                edit_state.get_caret_mut().set_font_page(*new_caret_page);
                edit_state.get_buffer_mut().layers = new_layers.clone();
                Ok(())
            }
            EditorUndoOp::RemoveFont { font_slot, font } => {
                if let Some(f) = font.take() {
                    edit_state.get_buffer_mut().set_font(*font_slot, f);
                }
                Ok(())
            }
            EditorUndoOp::ChangeFontSlot { from, to } => {
                std::mem::swap(from, to);
                if let Some(font) = edit_state.get_buffer_mut().remove_font(*from) {
                    edit_state.get_buffer_mut().set_font(*to, font);
                }
                Ok(())
            }
            EditorUndoOp::UpdateLayerProperties {
                index,
                old_properties,
                new_properties,
            } => {
                std::mem::swap(old_properties, new_properties);
                edit_state.get_buffer_mut().layers[*index].properties = new_properties.clone();
                Ok(())
            }
            EditorUndoOp::SetUseLetterSpacing { new_ls } => {
                let old = edit_state.get_buffer().use_letter_spacing();
                edit_state.get_buffer_mut().set_use_letter_spacing(*new_ls);
                *new_ls = old;
                Ok(())
            }
            EditorUndoOp::SetUseAspectRatio { new_ar } => {
                let old = edit_state.get_buffer().use_aspect_ratio();
                edit_state.get_buffer_mut().set_use_aspect_ratio(*new_ar);
                *new_ar = old;
                Ok(())
            }
            EditorUndoOp::SetFontDimensions { old_size, new_size } => {
                std::mem::swap(old_size, new_size);
                edit_state.get_buffer_mut().set_font_dimensions(*new_size);
                Ok(())
            }
            EditorUndoOp::AddTag { new_tag, .. } => {
                edit_state.get_buffer_mut().tags.retain(|t| t != new_tag);
                Ok(())
            }
            EditorUndoOp::EditTag { tag_index, old_tag, new_tag } => {
                std::mem::swap(old_tag, new_tag);
                if let Some(tag) = edit_state.get_buffer_mut().tags.get_mut(*tag_index) {
                    *tag = new_tag.clone();
                } else {
                    log::warn!(
                        "EditTag undo: tag index {} out of bounds (len={})",
                        tag_index,
                        edit_state.get_buffer().tags.len()
                    );
                }
                Ok(())
            }
            EditorUndoOp::MoveTag { tag, old_pos, new_pos } => {
                std::mem::swap(old_pos, new_pos);
                if let Some(t) = edit_state.get_buffer_mut().tags.get_mut(*tag) {
                    t.position = *new_pos;
                } else {
                    log::warn!("MoveTag undo: tag index {} out of bounds (len={})", tag, edit_state.get_buffer().tags.len());
                }
                Ok(())
            }
            EditorUndoOp::RemoveTag { tag_index, tag } => {
                edit_state.get_buffer_mut().tags.insert(*tag_index, tag.clone());
                Ok(())
            }
            EditorUndoOp::ShowTags { show } => {
                *show = !*show;
                Ok(())
            }
        }
    }

    /// Perform the redo operation
    pub fn redo(&mut self, edit_state: &mut EditState) -> Result<()> {
        match self {
            EditorUndoOp::Atomic { operations, .. } => {
                for op in operations.iter_mut() {
                    op.redo(edit_state)?;
                }
                Ok(())
            }
            EditorUndoOp::SetChar {
                pos, layer, new, redo_caret, ..
            } => {
                edit_state.get_buffer_mut().layers[*layer].set_char(*pos, *new);
                if let Some(caret_pos) = redo_caret {
                    edit_state.set_caret_position(*caret_pos);
                }
                Ok(())
            }
            EditorUndoOp::SwapChar { layer, pos1, pos2 } => {
                edit_state.get_buffer_mut().layers[*layer].swap_char(*pos1, *pos2);
                Ok(())
            }
            EditorUndoOp::AddLayer { index, layer } => {
                if let Some(l) = layer.take() {
                    edit_state.get_buffer_mut().layers.insert(*index, l);
                }
                Ok(())
            }
            EditorUndoOp::RemoveLayer { layer_index, layer } => {
                if *layer_index < edit_state.get_buffer().layers.len() {
                    *layer = Some(edit_state.get_buffer_mut().layers.remove(*layer_index));
                    edit_state.clamp_current_layer();
                    Ok(())
                } else {
                    Err(EngineError::Generic(format!("Invalid layer: {}", layer_index)))
                }
            }
            EditorUndoOp::RaiseLayer { layer_index } => {
                edit_state.get_buffer_mut().layers.swap(*layer_index, *layer_index + 1);
                Ok(())
            }
            EditorUndoOp::LowerLayer { layer_index } => {
                edit_state.get_buffer_mut().layers.swap(*layer_index, *layer_index - 1);
                Ok(())
            }
            EditorUndoOp::MergeLayerDown {
                index,
                merged_layer,
                orig_layers,
            } => {
                if let Some(layer) = merged_layer.take() {
                    *orig_layers = Some(edit_state.get_buffer_mut().layers.drain((*index - 1)..=*index).collect());
                    edit_state.get_buffer_mut().layers.insert(*index - 1, layer);
                    edit_state.set_current_layer(*index - 1);
                }
                Ok(())
            }
            EditorUndoOp::ToggleLayerVisibility { index } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*index) {
                    layer.properties.is_visible = !layer.properties.is_visible;
                }
                Ok(())
            }
            EditorUndoOp::MoveLayer { index, to, .. } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*index) {
                    layer.set_offset(*to);
                }
                Ok(())
            }
            EditorUndoOp::SetLayerSize { index, from, to } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*index) {
                    *from = layer.size();
                    layer.set_size(*to);
                }
                Ok(())
            }
            EditorUndoOp::Paste { layer, current_layer } => {
                if let Some(l) = layer.take() {
                    edit_state.get_buffer_mut().layers.insert(*current_layer + 1, l);
                    edit_state.set_current_layer(*current_layer + 1);
                }
                Ok(())
            }
            EditorUndoOp::AddFloatingLayer { current_layer } => {
                if let Some(layer) = edit_state.get_buffer_mut().layers.get_mut(*current_layer) {
                    layer.properties.title = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "layer-new-name");
                }
                Ok(())
            }
            EditorUndoOp::ResizeBuffer { size, .. } => {
                edit_state.get_buffer_mut().set_size(*size);
                edit_state.set_mask_size();
                Ok(())
            }
            EditorUndoOp::LayerChange {
                layer,
                pos,
                old_chars: _,
                new_chars,
            } => {
                if let Some(target_layer) = edit_state.get_buffer_mut().layers.get_mut(*layer) {
                    if target_layer.size() == new_chars.size() {
                        target_layer.lines = new_chars.lines.clone();
                    } else {
                        stamp_layer(target_layer, *pos, new_chars);
                    }
                }
                Ok(())
            }
            EditorUndoOp::Crop { size, layers, .. } => {
                edit_state.get_buffer_mut().set_size(*size);
                std::mem::swap(layers, &mut edit_state.get_buffer_mut().layers);
                edit_state.set_mask_size();
                Ok(())
            }
            EditorUndoOp::DeleteRow { layer, line, deleted_row } => {
                let l = &mut edit_state.get_buffer_mut().layers[*layer];
                if (*line as usize) < l.lines.len() {
                    *deleted_row = l.lines.remove(*line as usize);
                }
                Ok(())
            }
            EditorUndoOp::InsertRow { layer, line, inserted_row } => {
                edit_state.get_buffer_mut().layers[*layer].lines.insert(*line as usize, inserted_row.clone());
                Ok(())
            }
            EditorUndoOp::DeleteColumn { layer, column, deleted_chars } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                deleted_chars.clear();
                for line in &mut layer.lines {
                    if (*column as usize) < line.chars.len() {
                        deleted_chars.push(Some(line.chars.remove(*column as usize)));
                    } else {
                        deleted_chars.push(None);
                    }
                }
                let new_width = layer.width() - 1;
                layer.set_size((new_width, layer.height()));
                Ok(())
            }
            EditorUndoOp::InsertColumn { layer, column } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                for line in &mut layer.lines {
                    line.insert_char(*column, AttributedChar::default());
                }
                let new_width = layer.width() + 1;
                layer.set_size((new_width, layer.height()));
                Ok(())
            }
            EditorUndoOp::ScrollWholeLayerUp { layer } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                if !layer.lines.is_empty() {
                    let line = layer.lines.remove(0);
                    layer.lines.push(line);
                }
                Ok(())
            }
            EditorUndoOp::ScrollWholeLayerDown { layer } => {
                let layer = &mut edit_state.get_buffer_mut().layers[*layer];
                if let Some(line) = layer.lines.pop() {
                    layer.lines.insert(0, line);
                }
                Ok(())
            }
            EditorUndoOp::PasteRotate {
                layer,
                old_lines,
                new_lines,
                old_size,
                new_size,
            } => {
                // Set lines and size first, then swap for undo symmetry
                let l = &mut edit_state.get_buffer_mut().layers[*layer];
                l.lines = new_lines.clone();
                l.set_size(*new_size);
                std::mem::swap(old_lines, new_lines);
                std::mem::swap(old_size, new_size);
                Ok(())
            }
            EditorUndoOp::PasteFlipX { layer, old_lines, new_lines } => {
                // Set lines first, then swap for undo symmetry
                edit_state.get_buffer_mut().layers[*layer].lines = new_lines.clone();
                std::mem::swap(old_lines, new_lines);
                Ok(())
            }
            EditorUndoOp::PasteFlipY { layer, old_lines, new_lines } => {
                // Set lines first, then swap for undo symmetry
                edit_state.get_buffer_mut().layers[*layer].lines = new_lines.clone();
                std::mem::swap(old_lines, new_lines);
                Ok(())
            }
            EditorUndoOp::PasteAnchor { .. } => {
                // PasteAnchor is only for collaboration sync, local redo is handled by atomic group
                Ok(())
            }
            EditorUndoOp::SetBackground { new_value, .. } => {
                // For now, just set ice_mode based on whether background is non-zero
                // This is a simplification - actual background color handling may need more work
                if *new_value > 0 {
                    edit_state.get_buffer_mut().ice_mode = icy_engine::IceMode::Ice;
                }
                Ok(())
            }
            EditorUndoOp::Reversed { op, .. } => op.undo(edit_state),
            EditorUndoOp::ReverseCaretPosition { pos, .. } => {
                edit_state.get_caret_mut().set_position(*pos);
                Ok(())
            }
            EditorUndoOp::ClearLayer { layer_index, layer } => {
                std::mem::swap(layer, &mut edit_state.get_buffer_mut().layers[*layer_index].lines);
                Ok(())
            }
            EditorUndoOp::Deselect { sel } => {
                *sel = edit_state.selection_opt.clone().unwrap_or_default();
                edit_state.selection_opt = None;
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SelectNothing { .. } => {
                // sel and mask are already set when the operation is created
                // redo just needs to clear selection and mask
                edit_state.selection_opt = None;
                edit_state.selection_mask.clear();
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SetSelection { new, .. } => {
                edit_state.selection_opt = new.clone();
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SetSelectionMask { new, .. } => {
                edit_state.set_selection_mask(new.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::AddSelectionToMask { old, selection } => {
                *old = edit_state.selection_mask.clone();
                edit_state.selection_mask.add_selection(selection.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::InverseSelection { sel, old, new } => {
                *sel = edit_state.selection_opt.clone();
                std::mem::swap(old, new);
                edit_state.selection_opt = None;
                edit_state.set_selection_mask(new.clone());
                edit_state.mark_dirty();
                Ok(())
            }
            EditorUndoOp::SwitchPalettte { pal } => {
                std::mem::swap(pal, &mut edit_state.get_buffer_mut().palette);
                Ok(())
            }
            EditorUndoOp::SetSauceData { old, new } => {
                // Set value first, then swap for undo symmetry
                edit_state.set_sauce_meta(new.clone().into());
                std::mem::swap(old, new);
                Ok(())
            }
            EditorUndoOp::SwitchToFontPage { old, new } => {
                // Set value first, then swap for undo symmetry
                edit_state.get_caret_mut().set_font_page(*new);
                std::mem::swap(old, new);
                Ok(())
            }
            EditorUndoOp::SetFont { font_page, old, new } => {
                // Set font first, then swap for undo symmetry
                edit_state.get_buffer_mut().set_font(*font_page, new.clone());
                std::mem::swap(old, new);
                Ok(())
            }
            EditorUndoOp::AddFont { new_font_page, font, .. } => {
                edit_state.get_caret_mut().set_font_page(*new_font_page);
                edit_state.get_buffer_mut().set_font(*new_font_page, font.clone());
                Ok(())
            }
            EditorUndoOp::SwitchPalette {
                old_palette,
                old_layers,
                new_palette,
                new_layers,
            } => {
                // Set values first, then swap for undo symmetry
                let buf = edit_state.get_buffer_mut();
                buf.palette = new_palette.clone();
                buf.layers = new_layers.clone();
                buf.mark_dirty();
                std::mem::swap(old_palette, new_palette);
                std::mem::swap(old_layers, new_layers);
                Ok(())
            }
            EditorUndoOp::SetIceMode {
                old_mode,
                old_layers,
                new_mode,
                new_layers,
            } => {
                // Set values first, then swap for undo symmetry
                edit_state.get_buffer_mut().ice_mode = *new_mode;
                edit_state.get_buffer_mut().layers = new_layers.clone();
                std::mem::swap(old_mode, new_mode);
                std::mem::swap(old_layers, new_layers);
                Ok(())
            }
            EditorUndoOp::ReplaceFontUsage {
                old_caret_page,
                old_layers,
                new_caret_page,
                new_layers,
            } => {
                // Set values first, then swap for undo symmetry
                edit_state.get_caret_mut().set_font_page(*new_caret_page);
                edit_state.get_buffer_mut().layers = new_layers.clone();
                std::mem::swap(old_caret_page, new_caret_page);
                std::mem::swap(old_layers, new_layers);
                Ok(())
            }
            EditorUndoOp::RemoveFont { font_slot, font } => {
                *font = edit_state.get_buffer_mut().remove_font(*font_slot);
                Ok(())
            }
            EditorUndoOp::ChangeFontSlot { from, to } => {
                // Move font first, then swap for undo symmetry
                if let Some(font) = edit_state.get_buffer_mut().remove_font(*from) {
                    edit_state.get_buffer_mut().set_font(*to, font);
                }
                std::mem::swap(from, to);
                Ok(())
            }
            EditorUndoOp::UpdateLayerProperties {
                index,
                old_properties,
                new_properties,
            } => {
                // Set properties first, then swap for undo symmetry
                edit_state.get_buffer_mut().layers[*index].properties = new_properties.clone();
                std::mem::swap(old_properties, new_properties);
                Ok(())
            }
            EditorUndoOp::SetUseLetterSpacing { new_ls } => {
                let old = edit_state.get_buffer().use_letter_spacing();
                edit_state.get_buffer_mut().set_use_letter_spacing(*new_ls);
                *new_ls = old;
                Ok(())
            }
            EditorUndoOp::SetUseAspectRatio { new_ar } => {
                let old = edit_state.get_buffer().use_aspect_ratio();
                edit_state.get_buffer_mut().set_use_aspect_ratio(*new_ar);
                *new_ar = old;
                Ok(())
            }
            EditorUndoOp::SetFontDimensions { old_size, new_size } => {
                // Set dimensions first, then swap for undo symmetry
                edit_state.get_buffer_mut().set_font_dimensions(*new_size);
                std::mem::swap(old_size, new_size);
                Ok(())
            }
            EditorUndoOp::AddTag { new_tag, .. } => {
                edit_state.get_buffer_mut().tags.push(new_tag.clone());
                Ok(())
            }
            EditorUndoOp::EditTag { tag_index, old_tag, new_tag } => {
                // Set tag first, then swap for undo symmetry
                if let Some(tag) = edit_state.get_buffer_mut().tags.get_mut(*tag_index) {
                    *tag = new_tag.clone();
                } else {
                    log::warn!(
                        "EditTag redo: tag index {} out of bounds (len={})",
                        tag_index,
                        edit_state.get_buffer().tags.len()
                    );
                }
                std::mem::swap(old_tag, new_tag);
                Ok(())
            }
            EditorUndoOp::MoveTag { tag, old_pos, new_pos } => {
                // Move tag first, then swap for undo symmetry
                if let Some(t) = edit_state.get_buffer_mut().tags.get_mut(*tag) {
                    t.position = *new_pos;
                } else {
                    log::warn!("MoveTag redo: tag index {} out of bounds (len={})", tag, edit_state.get_buffer().tags.len());
                }
                std::mem::swap(old_pos, new_pos);
                Ok(())
            }
            EditorUndoOp::RemoveTag { tag_index, .. } => {
                edit_state.get_buffer_mut().tags.remove(*tag_index);
                Ok(())
            }
            EditorUndoOp::ShowTags { show } => {
                *show = !*show;
                Ok(())
            }
        }
    }
}

// Collaboration support - mapping UndoOps to ClientCommands
#[cfg(feature = "collaboration")]
mod collab_mapping {
    use super::EditorUndoOp;
    use crate::collaboration::{Block, ClientCommand};

    impl EditorUndoOp {
        /// Get ClientCommands to send when this operation is redone (forward direction).
        /// Returns None for operations that don't need network sync.
        pub fn redo_client_commands(&self) -> Option<Vec<ClientCommand>> {
            match self {
                EditorUndoOp::Atomic { operations, .. } => {
                    let mut cmds = Vec::new();
                    for op in operations {
                        if let Some(sub_cmds) = op.redo_client_commands() {
                            cmds.extend(sub_cmds);
                        }
                    }
                    if cmds.is_empty() { None } else { Some(cmds) }
                }

                EditorUndoOp::SetChar { pos, layer: _, new, .. } => Some(vec![ClientCommand::Draw {
                    col: pos.x,
                    row: pos.y,
                    block: Block {
                        code: new.ch as u32,
                        fg: new.attribute.foreground() as u8,
                        bg: new.attribute.background() as u8,
                    },
                }]),

                EditorUndoOp::SwapChar { .. } => None, // Complex, skip for now

                EditorUndoOp::LayerChange { pos, new_chars, .. } => {
                    let mut cmds = Vec::new();
                    for (y, line) in new_chars.lines.iter().enumerate() {
                        for (x, ch) in line.chars.iter().enumerate() {
                            cmds.push(ClientCommand::Draw {
                                col: pos.x + x as i32,
                                row: pos.y + y as i32,
                                block: Block {
                                    code: ch.ch as u32,
                                    fg: ch.attribute.foreground() as u8,
                                    bg: ch.attribute.background() as u8,
                                },
                            });
                        }
                    }
                    if cmds.is_empty() { None } else { Some(cmds) }
                }

                EditorUndoOp::ResizeBuffer { size, .. } => Some(vec![ClientCommand::SetCanvasSize {
                    columns: size.width as u32,
                    rows: size.height as u32,
                }]),

                EditorUndoOp::SetSauceData { new, .. } => Some(vec![ClientCommand::SetSauce {
                    title: new.title.to_string(),
                    author: new.author.to_string(),
                    group: new.group.to_string(),
                    comments: new.comments.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n"),
                }]),

                EditorUndoOp::SetIceMode { new_mode, .. } => {
                    let value = match new_mode {
                        crate::IceMode::Unlimited | crate::IceMode::Ice => true,
                        crate::IceMode::Blink => false,
                    };
                    Some(vec![ClientCommand::SetIceColors { value }])
                }

                EditorUndoOp::SetUseLetterSpacing { new_ls } => Some(vec![ClientCommand::SetUse9px { value: *new_ls }]),

                EditorUndoOp::PasteRotate { .. } => Some(vec![ClientCommand::Rotate]),

                EditorUndoOp::PasteFlipX { .. } => Some(vec![ClientCommand::FlipX]),

                EditorUndoOp::PasteFlipY { .. } => Some(vec![ClientCommand::FlipY]),

                EditorUndoOp::PasteAnchor { x, y, blocks } => {
                    // Generate DRAW commands for all blocks in the anchored layer
                    let mut cmds = Vec::new();
                    for (idx, block) in blocks.data.iter().enumerate() {
                        let col = *x + (idx as i32 % blocks.columns as i32);
                        let row = *y + (idx as i32 / blocks.columns as i32);
                        cmds.push(ClientCommand::Draw {
                            col,
                            row,
                            block: block.clone(),
                        });
                    }
                    Some(cmds)
                }

                EditorUndoOp::SetBackground { new_value, .. } => Some(vec![ClientCommand::SetBackground { value: *new_value }]),

                // Operations that don't map to collaboration commands
                EditorUndoOp::AddLayer { .. }
                | EditorUndoOp::RemoveLayer { .. }
                | EditorUndoOp::RaiseLayer { .. }
                | EditorUndoOp::LowerLayer { .. }
                | EditorUndoOp::MergeLayerDown { .. }
                | EditorUndoOp::ToggleLayerVisibility { .. }
                | EditorUndoOp::MoveLayer { .. }
                | EditorUndoOp::SetLayerSize { .. }
                | EditorUndoOp::Paste { .. }
                | EditorUndoOp::AddFloatingLayer { .. }
                | EditorUndoOp::Crop { .. }
                | EditorUndoOp::DeleteRow { .. }
                | EditorUndoOp::InsertRow { .. }
                | EditorUndoOp::DeleteColumn { .. }
                | EditorUndoOp::InsertColumn { .. }
                | EditorUndoOp::ScrollWholeLayerUp { .. }
                | EditorUndoOp::ScrollWholeLayerDown { .. }
                | EditorUndoOp::Reversed { .. }
                | EditorUndoOp::ReverseCaretPosition { .. }
                | EditorUndoOp::ClearLayer { .. }
                | EditorUndoOp::Deselect { .. }
                | EditorUndoOp::SelectNothing { .. }
                | EditorUndoOp::SetSelection { .. }
                | EditorUndoOp::SetSelectionMask { .. }
                | EditorUndoOp::AddSelectionToMask { .. }
                | EditorUndoOp::InverseSelection { .. }
                | EditorUndoOp::SwitchPalettte { .. }
                | EditorUndoOp::SwitchToFontPage { .. }
                | EditorUndoOp::SetFont { .. }
                | EditorUndoOp::AddFont { .. }
                | EditorUndoOp::SwitchPalette { .. }
                | EditorUndoOp::ReplaceFontUsage { .. }
                | EditorUndoOp::RemoveFont { .. }
                | EditorUndoOp::ChangeFontSlot { .. }
                | EditorUndoOp::UpdateLayerProperties { .. }
                | EditorUndoOp::SetUseAspectRatio { .. }
                | EditorUndoOp::SetFontDimensions { .. }
                | EditorUndoOp::AddTag { .. }
                | EditorUndoOp::EditTag { .. }
                | EditorUndoOp::MoveTag { .. }
                | EditorUndoOp::RemoveTag { .. }
                | EditorUndoOp::ShowTags { .. } => None,
            }
        }

        /// Get ClientCommands to send when this operation is undone (backward direction).
        /// Returns None for operations that don't need network sync.
        pub fn undo_client_commands(&self) -> Option<Vec<ClientCommand>> {
            match self {
                EditorUndoOp::Atomic { operations, .. } => {
                    let mut cmds = Vec::new();
                    // Reverse order for undo
                    for op in operations.iter().rev() {
                        if let Some(sub_cmds) = op.undo_client_commands() {
                            cmds.extend(sub_cmds);
                        }
                    }
                    if cmds.is_empty() { None } else { Some(cmds) }
                }

                EditorUndoOp::SetChar { pos, layer: _, old, .. } => Some(vec![ClientCommand::Draw {
                    col: pos.x,
                    row: pos.y,
                    block: Block {
                        code: old.ch as u32,
                        fg: old.attribute.foreground() as u8,
                        bg: old.attribute.background() as u8,
                    },
                }]),

                EditorUndoOp::SwapChar { .. } => None,

                EditorUndoOp::LayerChange { pos, old_chars, .. } => {
                    let mut cmds = Vec::new();
                    for (y, line) in old_chars.lines.iter().enumerate() {
                        for (x, ch) in line.chars.iter().enumerate() {
                            cmds.push(ClientCommand::Draw {
                                col: pos.x + x as i32,
                                row: pos.y + y as i32,
                                block: Block {
                                    code: ch.ch as u32,
                                    fg: ch.attribute.foreground() as u8,
                                    bg: ch.attribute.background() as u8,
                                },
                            });
                        }
                    }
                    if cmds.is_empty() { None } else { Some(cmds) }
                }

                EditorUndoOp::ResizeBuffer { orig_size, .. } => Some(vec![ClientCommand::SetCanvasSize {
                    columns: orig_size.width as u32,
                    rows: orig_size.height as u32,
                }]),

                EditorUndoOp::SetSauceData { old, .. } => Some(vec![ClientCommand::SetSauce {
                    title: old.title.to_string(),
                    author: old.author.to_string(),
                    group: old.group.to_string(),
                    comments: old.comments.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n"),
                }]),

                EditorUndoOp::SetIceMode { old_mode, .. } => {
                    let value = match old_mode {
                        crate::IceMode::Unlimited | crate::IceMode::Ice => true,
                        crate::IceMode::Blink => false,
                    };
                    Some(vec![ClientCommand::SetIceColors { value }])
                }

                EditorUndoOp::SetUseLetterSpacing { new_ls } => {
                    // Undo means reverse the value
                    Some(vec![ClientCommand::SetUse9px { value: !*new_ls }])
                }

                // For rotate/flip, we'd need to send the opposite operation
                // but the protocol doesn't have "undo rotate", so we skip
                EditorUndoOp::PasteRotate { .. } => None,
                EditorUndoOp::PasteFlipX { .. } => None,
                EditorUndoOp::PasteFlipY { .. } => None,
                EditorUndoOp::PasteAnchor { .. } => None,

                EditorUndoOp::SetBackground { old_value, .. } => Some(vec![ClientCommand::SetBackground { value: *old_value }]),

                // Operations that don't map to collaboration commands
                _ => None,
            }
        }
    }
}
