use serde::{Deserialize, Serialize};

use crate::{AddType, Position, Rectangle, Selection, Size, overlay_mask::OverlayMask};

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelectionMask {
    overlay_mask: OverlayMask,
}

impl SelectionMask {
    pub fn clear(&mut self) {
        self.overlay_mask.clear();
    }

    pub fn add_rectangle(&mut self, rect: Rectangle) {
        self.overlay_mask.add_rectangle(rect);
    }

    pub fn remove_rectangle(&mut self, rect: Rectangle) {
        self.overlay_mask.remove_rectangle(rect);
    }

    pub fn is_selected(&self, pos: impl Into<Position>) -> bool {
        self.overlay_mask.is_selected(pos)
    }

    pub fn set_is_selected(&mut self, pos: impl Into<Position>, selected: bool) {
        self.overlay_mask.set_is_selected(pos, selected);
    }

    pub fn set_size(&mut self, size: Size) {
        self.overlay_mask.set_size(size);
    }

    pub fn is_empty(&self) -> bool {
        self.overlay_mask.is_empty()
    }

    pub fn rectangle(&self) -> Rectangle {
        self.overlay_mask.rectangle()
    }

    pub fn add_selection(&mut self, selection: Selection) {
        match selection.shape {
            crate::Shape::Rectangle => {
                let rect: Rectangle = selection.as_rectangle();
                #[cfg(debug_assertions)]
                eprintln!(
                    "[DEBUG] AddSelectionToMask - Adding selection rect: ({}, {}, w={}, h={})",
                    rect.left(),
                    rect.top(),
                    rect.width(),
                    rect.height()
                );

                self.add_rectangle(rect);
                #[cfg(debug_assertions)]
                eprintln!("[DEBUG] AddSelectionToMask Mask after adding rect: {:?}", self.is_empty());
            }
            crate::Shape::Lines => {
                let mut pos = selection.anchor;
                let mut max = selection.lead;
                if pos > max {
                    std::mem::swap(&mut pos, &mut max);
                }
                while pos < max {
                    self.set_is_selected(pos, true);
                    pos.x += 1;
                    if pos.x >= self.overlay_mask.width() {
                        pos.x = 0;
                        pos.y += 1;
                    }
                }
            }
        }
    }

    pub fn remove_selection(&mut self, selection: Selection) {
        match selection.shape {
            crate::Shape::Rectangle => {
                let rect = selection.as_rectangle();

                #[cfg(debug_assertions)]
                eprintln!(
                    "[DEBUG] AddSelectionToMask::redo - Removing selection rect: ({}, {}, w={}, h={})",
                    rect.left(),
                    rect.top(),
                    rect.width(),
                    rect.height()
                );

                self.remove_rectangle(rect);
            }
            crate::Shape::Lines => {
                let mut pos = selection.anchor;
                let mut max = selection.lead;
                if pos > max {
                    std::mem::swap(&mut pos, &mut max);
                }
                while pos < max {
                    self.set_is_selected(pos, false);
                    pos.x += 1;
                    if pos.x >= self.overlay_mask.width() {
                        pos.x = 0;
                        pos.y += 1;
                    }
                }
            }
        }
    }

    pub fn selected_rectangle(&self, selection_opt: &Option<Selection>) -> Rectangle {
        let mut rect = self.rectangle();
        if let Some(sel) = selection_opt {
            if rect.is_empty() {
                return sel.as_rectangle();
            }
            rect = rect.union(&sel.as_rectangle());
        }
        rect
    }

    pub fn selected_in_selection(&self, pos: impl Into<Position>, selection_opt: &Option<Selection>) -> bool {
        let pos = pos.into();
        if let Some(sel) = selection_opt {
            if sel.is_inside(pos) {
                return !matches!(sel.add_type, AddType::Subtract);
            }
        }

        self.is_selected(pos)
    }
}
