//! Selection drag handling for the ANSI editor.
//!
//! This module encapsulates all logic related to dragging/resizing selections,
//! used by both the Click tool and the Select tool.

use icy_engine::{Position, Rectangle, Selection};

/// Selection drag mode - determines what part of selection is being dragged
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum SelectionDrag {
    #[default]
    None,
    /// Create new selection
    Create,
    /// Move existing selection
    Move,
    /// Resize from edges/corners
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl SelectionDrag {
    /// Convert to mouse cursor interaction for resize handles
    pub fn to_cursor_interaction(self) -> Option<icy_ui::mouse::Interaction> {
        use icy_ui::mouse::Interaction;
        match self {
            SelectionDrag::None | SelectionDrag::Create => None,
            SelectionDrag::Move => Some(Interaction::Grab),
            SelectionDrag::Left | SelectionDrag::Right => Some(Interaction::ResizingHorizontally),
            SelectionDrag::Top | SelectionDrag::Bottom => Some(Interaction::ResizingVertically),
            SelectionDrag::TopLeft | SelectionDrag::BottomRight => Some(Interaction::ResizingDiagonallyDown),
            SelectionDrag::TopRight | SelectionDrag::BottomLeft => Some(Interaction::ResizingDiagonallyUp),
        }
    }
}

/// Hit-test a position against a selection to determine which drag handle (if any) is under the cursor.
///
/// Returns `SelectionDrag::None` if the position is outside the selection.
pub fn hit_test_selection(selection: Option<Selection>, pos: Position) -> SelectionDrag {
    let Some(selection) = selection else {
        return SelectionDrag::None;
    };

    let rect = selection.as_rectangle();

    if !rect.contains_pt(pos) {
        return SelectionDrag::None;
    }

    // Check edges/corners (within 2 chars)
    let left = pos.x - rect.left() < 2;
    let top = pos.y - rect.top() < 2;
    let right = rect.right() - pos.x < 2;
    let bottom = rect.bottom() - pos.y < 2;

    // Corners first
    if left && top {
        return SelectionDrag::TopLeft;
    }
    if right && top {
        return SelectionDrag::TopRight;
    }
    if left && bottom {
        return SelectionDrag::BottomLeft;
    }
    if right && bottom {
        return SelectionDrag::BottomRight;
    }

    // Edges
    if left {
        return SelectionDrag::Left;
    }
    if right {
        return SelectionDrag::Right;
    }
    if top {
        return SelectionDrag::Top;
    }
    if bottom {
        return SelectionDrag::Bottom;
    }

    // Inside - move
    SelectionDrag::Move
}

/// Parameters for computing a resized/moved selection.
#[derive(Clone, Copy, Debug)]
pub struct DragParameters {
    /// The selection rectangle at the start of the drag
    pub start_rect: Rectangle,
    /// Drag start position (absolute buffer coords)
    pub start_pos: Position,
    /// Current drag position (absolute buffer coords)
    pub cur_pos: Position,
}

/// Compute a new selection rectangle based on the drag mode and parameters.
///
/// For `SelectionDrag::Create`, `start_pos` and `cur_pos` define the new rectangle directly.
/// For move/resize modes, the delta between `start_pos` and `cur_pos` is applied to `start_rect`.
///
/// Returns `None` if the drag mode is `None`.
pub fn compute_dragged_selection(mode: SelectionDrag, params: DragParameters) -> Option<Rectangle> {
    match mode {
        SelectionDrag::None => None,
        SelectionDrag::Create => {
            // Create new selection spanning from start to current
            let min_x = params.start_pos.x.min(params.cur_pos.x);
            let max_x = params.start_pos.x.max(params.cur_pos.x);
            let min_y = params.start_pos.y.min(params.cur_pos.y);
            let max_y = params.start_pos.y.max(params.cur_pos.y);
            Some(Rectangle::from(min_x, min_y, max_x - min_x, max_y - min_y))
        }
        SelectionDrag::Move => {
            let delta_x = params.cur_pos.x - params.start_pos.x;
            let delta_y = params.cur_pos.y - params.start_pos.y;
            Some(Rectangle::from(
                params.start_rect.left() + delta_x,
                params.start_rect.top() + delta_y,
                params.start_rect.width(),
                params.start_rect.height(),
            ))
        }
        SelectionDrag::Left => Some(resize_left(params)),
        SelectionDrag::Right => Some(resize_right(params)),
        SelectionDrag::Top => Some(resize_top(params)),
        SelectionDrag::Bottom => Some(resize_bottom(params)),
        SelectionDrag::TopLeft => Some(resize_corner(params, true, true)),
        SelectionDrag::TopRight => Some(resize_corner(params, false, true)),
        SelectionDrag::BottomLeft => Some(resize_corner(params, true, false)),
        SelectionDrag::BottomRight => Some(resize_corner(params, false, false)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal resize helpers
// ─────────────────────────────────────────────────────────────────────────────

fn resize_left(p: DragParameters) -> Rectangle {
    let delta = p.start_pos.x - p.cur_pos.x;
    let mut new_left = p.start_rect.left() - delta;
    let mut new_width = p.start_rect.width() + delta;

    if new_width < 0 {
        new_width = new_left - p.start_rect.right();
        new_left = p.start_rect.right();
    }

    Rectangle::from(new_left, p.start_rect.top(), new_width, p.start_rect.height())
}

fn resize_right(p: DragParameters) -> Rectangle {
    let mut new_width = p.start_rect.width() - p.start_pos.x + p.cur_pos.x;
    let mut new_left = p.start_rect.left();

    if new_width < 0 {
        new_left = p.start_rect.left() + new_width;
        new_width = p.start_rect.left() - new_left;
    }

    Rectangle::from(new_left, p.start_rect.top(), new_width, p.start_rect.height())
}

fn resize_top(p: DragParameters) -> Rectangle {
    let delta = p.start_pos.y - p.cur_pos.y;
    let mut new_top = p.start_rect.top() - delta;
    let mut new_height = p.start_rect.height() + delta;

    if new_height < 0 {
        new_height = new_top - p.start_rect.bottom();
        new_top = p.start_rect.bottom();
    }

    Rectangle::from(p.start_rect.left(), new_top, p.start_rect.width(), new_height)
}

fn resize_bottom(p: DragParameters) -> Rectangle {
    let mut new_height = p.start_rect.height() - p.start_pos.y + p.cur_pos.y;
    let mut new_top = p.start_rect.top();

    if new_height < 0 {
        new_top = p.start_rect.top() + new_height;
        new_height = p.start_rect.top() - new_top;
    }

    Rectangle::from(p.start_rect.left(), new_top, p.start_rect.width(), new_height)
}

fn resize_corner(p: DragParameters, resize_left_edge: bool, resize_top_edge: bool) -> Rectangle {
    // X dimension
    let (new_left, new_width) = if resize_left_edge {
        let delta = p.start_pos.x - p.cur_pos.x;
        let mut left = p.start_rect.left() - delta;
        let mut width = p.start_rect.width() + delta;
        if width < 0 {
            width = left - p.start_rect.right();
            left = p.start_rect.right();
        }
        (left, width)
    } else {
        let mut width = p.start_rect.width() - p.start_pos.x + p.cur_pos.x;
        let mut left = p.start_rect.left();
        if width < 0 {
            left = p.start_rect.left() + width;
            width = p.start_rect.left() - left;
        }
        (left, width)
    };

    // Y dimension
    let (new_top, new_height) = if resize_top_edge {
        let delta = p.start_pos.y - p.cur_pos.y;
        let mut top = p.start_rect.top() - delta;
        let mut height = p.start_rect.height() + delta;
        if height < 0 {
            height = top - p.start_rect.bottom();
            top = p.start_rect.bottom();
        }
        (top, height)
    } else {
        let mut height = p.start_rect.height() - p.start_pos.y + p.cur_pos.y;
        let mut top = p.start_rect.top();
        if height < 0 {
            top = p.start_rect.top() + height;
            height = p.start_rect.top() - top;
        }
        (top, height)
    };

    Rectangle::from(new_left, new_top, new_width, new_height)
}
