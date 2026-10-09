use std::collections::HashMap;

use i18n_embed_fl::fl;

use crate::{EngineError, Layer, Position, Result};

use super::{EditState, EditorUndoOp};

/// Where a dragged layer lands, relative to the layer `usize` in stack order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerDrop {
    /// Directly above the layer or group, in the same parent.
    Above(usize),
    /// Directly below the layer or group's whole subtree, in the same parent.
    Below(usize),
    /// At the top inside the group.
    Into(usize),
}

impl LayerDrop {
    pub fn index(self) -> usize {
        match self {
            Self::Above(index) | Self::Below(index) | Self::Into(index) => index,
        }
    }
}

impl EditState {
    pub(crate) fn replace_layer_stack(&mut self, layers: Vec<Layer>, current_layer: usize, description: String) -> Result<()> {
        if layers.is_empty() || current_layer >= layers.len() {
            return Err(EngineError::Generic("A document must retain a layer".into()));
        }
        let mut buffer = self.get_buffer().clone();
        buffer.layers = layers;
        buffer.validate_layer_groups()?;
        self.push_undo_action(EditorUndoOp::ReplaceLayers {
            layers: buffer.layers,
            current_layer,
            description,
        })
    }

    /// Wraps a layer or an existing group without changing compositing order.
    pub fn group_layer(&mut self, index: usize) -> Result<()> {
        let buffer = self.get_buffer();
        let layer = buffer.layers.get(index).ok_or_else(|| EngineError::Generic("Invalid layer index".into()))?;
        let id = buffer.next_group_id()?;
        let mut group = Layer::new_group(fl!(crate::LANGUAGE_LOADER, "layer-new-group"), id);
        group.parent_group = layer.parent_group;
        let mut layers = buffer.layers.clone();
        layers[index].parent_group = Some(id);
        layers.insert(index + 1, group);
        self.replace_layer_stack(layers, index + 1, fl!(crate::LANGUAGE_LOADER, "undo-group-layers"))
    }

    pub fn ungroup_layer(&mut self, index: usize) -> Result<()> {
        let buffer = self.get_buffer();
        let layer = buffer.layers.get(index).ok_or_else(|| EngineError::Generic("Invalid layer index".into()))?;
        let group = layer.group.as_ref().ok_or_else(|| EngineError::Generic("This layer is not a group".into()))?;
        let id = group.id;
        let parent = layer.parent_group;
        let visible = layer.is_visible();
        let mut layers = buffer.layers.clone();
        for child in &mut layers {
            if child.parent_group == Some(id) {
                child.parent_group = parent;
                child.properties.is_visible &= visible;
            }
        }
        layers.remove(index);
        let current = index.saturating_sub(1).min(layers.len().saturating_sub(1));
        self.replace_layer_stack(layers, current, fl!(crate::LANGUAGE_LOADER, "undo-ungroup-layers"))
    }

    /// Moves the complete subtree to the top of group `parent`, or with `None` out of its
    /// group to just above that group.
    pub fn move_layer_to_group(&mut self, index: usize, parent: Option<u64>) -> Result<()> {
        let buffer = self.get_buffer();
        let layer = buffer.layers.get(index).ok_or_else(|| EngineError::Generic("Invalid layer index".into()))?;
        let target = match parent {
            Some(id) => LayerDrop::Into(buffer.group_index(id).ok_or_else(|| EngineError::Generic("Invalid destination group".into()))?),
            None => match layer.parent_group.and_then(|id| buffer.group_index(id)) {
                Some(group) => LayerDrop::Above(group),
                None => return Ok(()),
            },
        };
        self.drop_layer(index, target)
    }

    /// Moves the layer or group `index` with its subtree to `target` in one undo step.
    pub fn drop_layer(&mut self, index: usize, target: LayerDrop) -> Result<()> {
        let buffer = self.get_buffer();
        if index >= buffer.layers.len() || target.index() >= buffer.layers.len() {
            return Err(EngineError::Generic("Invalid layer index".into()));
        }
        let range = buffer.layer_subtree(index);
        if range.contains(&target.index()) {
            if matches!(target, LayerDrop::Into(_)) {
                return Err(EngineError::Generic("A group cannot contain itself".into()));
            }
            return Ok(());
        }
        let (parent, insert) = match target {
            LayerDrop::Into(group) => {
                let id = buffer.layers[group]
                    .group
                    .as_ref()
                    .ok_or_else(|| EngineError::Generic("The destination is not a group".into()))?
                    .id;
                (Some(id), group)
            }
            LayerDrop::Above(other) => (buffer.layers[other].parent_group, other + 1),
            LayerDrop::Below(other) => (buffer.layers[other].parent_group, buffer.layer_subtree(other).start),
        };
        let insert = if insert > range.start { insert - range.len() } else { insert };
        let mut layers = buffer.layers.clone();
        let mut moved: Vec<_> = layers.drain(range).collect();
        moved.last_mut().unwrap().parent_group = parent;
        let current = insert + moved.len() - 1;
        layers.splice(insert..insert, moved);
        if layers == buffer.layers {
            return Ok(());
        }
        self.replace_layer_stack(layers, current, fl!(crate::LANGUAGE_LOADER, "undo-move-layer-group"))
    }

    pub fn toggle_group_collapsed(&mut self, index: usize) -> Result<()> {
        let mut layer = self
            .get_buffer()
            .layers
            .get(index)
            .ok_or_else(|| EngineError::Generic("Invalid layer index".into()))?
            .clone();
        let group = layer.group.as_mut().ok_or_else(|| EngineError::Generic("This layer is not a group".into()))?;
        group.collapsed = !group.collapsed;
        self.push_undo_action(EditorUndoOp::ReplaceLayer {
            index,
            layer: Box::new(layer),
            description: fl!(crate::LANGUAGE_LOADER, "undo-collapse-group"),
        })
    }

    pub(crate) fn duplicate_group(&mut self, index: usize) -> Result<()> {
        let buffer = self.get_buffer();
        let range = buffer.layer_subtree(index);
        let mut copies = buffer.layers[range].to_vec();
        let mut next = buffer.next_group_id()?;
        let mut ids = HashMap::new();
        for layer in &mut copies {
            if let Some(group) = &mut layer.group {
                ids.insert(group.id, next);
                group.id = next;
                next = next
                    .checked_add(1)
                    .ok_or_else(|| EngineError::Generic("Layer group identifiers exhausted".into()))?;
            }
        }
        for layer in &mut copies {
            if let Some(parent) = layer.parent_group.and_then(|id| ids.get(&id)) {
                layer.parent_group = Some(*parent);
            }
        }
        let root = copies.last_mut().unwrap();
        root.properties.title = fl!(crate::LANGUAGE_LOADER, "layer-duplicate-name", name = root.properties.title.clone());
        let current = index + copies.len();
        let mut layers = buffer.layers.clone();
        layers.splice(index + 1..index + 1, copies);
        self.replace_layer_stack(layers, current, fl!(crate::LANGUAGE_LOADER, "undo-group-layers"))
    }

    pub(crate) fn reorder_grouped_layer(&mut self, index: usize, up: bool) -> Result<()> {
        let buffer = self.get_buffer();
        buffer.layers.get(index).ok_or_else(|| EngineError::Generic("Invalid layer index".into()))?;
        let siblings: Vec<_> = buffer
            .layers
            .iter()
            .enumerate()
            .filter_map(|(i, layer)| (layer.parent_group == buffer.layers[index].parent_group).then_some(i))
            .collect();
        let neighbor = if up {
            siblings.into_iter().find(|i| *i > index)
        } else {
            siblings.into_iter().rev().find(|i| *i < index)
        };
        let Some(neighbor) = neighbor else { return Ok(()) };
        let a = buffer.layer_subtree(index.min(neighbor));
        let b = buffer.layer_subtree(index.max(neighbor));
        let mut layers = buffer.layers.clone();
        let replacement: Vec<_> = layers[b.clone()].iter().chain(&layers[a.clone()]).cloned().collect();
        let current = if up { b.end - 1 } else { a.start + b.len() - 1 };
        layers.splice(a.start..b.end, replacement);
        self.replace_layer_stack(layers, current, fl!(crate::LANGUAGE_LOADER, "undo-move-layer-group"))
    }

    pub fn can_move_layer(&self, index: usize) -> bool {
        index < self.get_buffer().layers.len()
            && self.get_buffer().layers[self.get_buffer().layer_subtree(index)]
                .iter()
                .all(|layer| !layer.properties.is_position_locked)
    }

    pub(crate) fn move_group(&mut self, index: usize, to: Position) -> Result<()> {
        if !self.can_move_layer(index) {
            return Err(EngineError::Generic("The group contains a position-locked layer".into()));
        }
        let mut layers = self.get_buffer().layers.clone();
        let delta = to - layers[index].base_offset();
        for layer in &mut layers[self.get_buffer().layer_subtree(index)] {
            layer.set_offset(layer.base_offset() + delta);
        }
        self.replace_layer_stack(layers, index, fl!(crate::LANGUAGE_LOADER, "undo-move-layer-group"))
    }
}
