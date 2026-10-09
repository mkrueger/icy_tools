use std::{collections::HashSet, ops::Range};

use serde::{Deserialize, Serialize};

use crate::{EngineError, Result, TextBuffer};

/// A pass-through group marker, stored immediately above its contiguous subtree.
/// Members keep document-relative offsets and their own visibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerGroup {
    pub id: u64,
    pub collapsed: bool,
}

impl TextBuffer {
    pub fn validate_layer_groups(&self) -> Result<()> {
        let mut open = Vec::new();
        let mut ids = HashSet::new();
        for layer in self.layers.iter().rev() {
            layer.validate_role()?;
            while open.last().copied() != layer.parent_group {
                if open.pop().is_none() {
                    return Err(EngineError::Generic("Invalid or non-contiguous layer group membership".into()));
                }
            }
            if let Some(group) = &layer.group {
                if group.id == 0 || !ids.insert(group.id) || open.len() >= 64 {
                    return Err(EngineError::Generic("Invalid, duplicate or excessively nested layer group".into()));
                }
                open.push(group.id);
            }
        }
        Ok(())
    }

    pub fn group_index(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|layer| layer.group.as_ref().is_some_and(|group| group.id == id))
    }

    pub fn layer_ancestors(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        let parent = self.layers.get(index).and_then(|layer| layer.parent_group);
        std::iter::successors(parent.and_then(|id| self.group_index(id)), |index| {
            self.layers[*index].parent_group.and_then(|id| self.group_index(id))
        })
        .take(64)
    }

    pub fn layer_is_visible(&self, index: usize) -> bool {
        self.layers.get(index).is_some_and(|layer| layer.is_visible()) && self.layer_ancestors(index).all(|parent| self.layers[parent].is_visible())
    }

    pub fn layer_subtree(&self, index: usize) -> Range<usize> {
        let mut start = index;
        if self.layers.get(index).is_some_and(|layer| layer.is_group()) {
            while start > 0 && self.layer_ancestors(start - 1).any(|parent| parent == index) {
                start -= 1;
            }
        }
        start..index + 1
    }

    pub fn visible_layer_rows(&self) -> Vec<(usize, usize)> {
        self.layers
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(index, _)| {
                let ancestors: Vec<_> = self.layer_ancestors(index).collect();
                (!ancestors
                    .iter()
                    .any(|parent| self.layers[*parent].group.as_ref().is_some_and(|group| group.collapsed)))
                .then_some((index, ancestors.len()))
            })
            .collect()
    }

    pub fn next_group_id(&self) -> Result<u64> {
        self.layers
            .iter()
            .filter_map(|layer| layer.group.as_ref().map(|group| group.id))
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| EngineError::Generic("Layer group identifiers exhausted".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttributedChar, Layer, Position, TextPane};

    #[test]
    fn nested_groups_preserve_visibility_and_stack_order() {
        let mut buffer = TextBuffer::new((4, 2));
        buffer.layers[0].set_char((0, 0), AttributedChar::from_char('A'));
        buffer.layers[0].parent_group = Some(1);
        let mut inner = Layer::new_group("Inner", 1);
        inner.parent_group = Some(2);
        buffer.layers.push(inner);
        buffer.layers.push(Layer::new_group("Outer", 2));
        buffer.validate_layer_groups().unwrap();
        assert_eq!(buffer.layer_subtree(2), 0..3);
        assert_eq!(buffer.visible_layer_rows(), [(2, 0), (1, 1), (0, 2)]);
        assert_eq!(buffer.char_at(Position::default()).ch, 'A');
        buffer.layers[2].properties.is_visible = false;
        assert!(!buffer.char_at(Position::default()).is_visible());
        assert!(buffer.layers[0].is_visible());
        buffer.layers[1].properties.is_visible = false;
        buffer.layers[2].properties.is_visible = true;
        assert!(!buffer.layer_is_visible(0));
        buffer.layers[2].group.as_mut().unwrap().collapsed = true;
        assert_eq!(buffer.visible_layer_rows(), [(2, 0)]);
        buffer.layers[0].parent_group = Some(99);
        assert!(buffer.validate_layer_groups().is_err());
    }
}
