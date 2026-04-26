//! Pure scroll calculation helpers (testable without the full editor).
//!
//! Extracted from `ansi_editor.rs` so the math stays decoupled from the
//! editor state and is easy to unit-test.

/// Computes the scroll offset needed to keep a caret cell visible.
///
/// Returns `Some((new_scroll_x, new_scroll_y))` if scrolling is needed,
/// or `None` if the caret is already fully visible.
///
/// # Arguments
/// * `caret_col`, `caret_row` - Caret position in document cell coordinates.
/// * `font_width`, `font_height` - Size of one character cell in content pixels.
/// * `scroll_x`, `scroll_y` - Current scroll offset in content pixels.
/// * `visible_width`, `visible_height` - Size of the visible viewport in content pixels.
/// * `content_width`, `content_height` - Total content size in pixels.
#[allow(clippy::too_many_arguments)]
pub(super) fn compute_scroll_to_keep_caret_visible(
    caret_col: i32,
    caret_row: i32,
    font_width: f32,
    font_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    visible_width: f32,
    visible_height: f32,
    content_width: f32,
    content_height: f32,
) -> Option<(f32, f32)> {
    // Caret cell bounds in content pixels.
    let caret_left = caret_col as f32 * font_width;
    let caret_right = caret_left + font_width;
    let caret_top = caret_row as f32 * font_height;
    let caret_bottom = caret_top + font_height;

    // Visible region in content coordinates.
    let view_left = scroll_x;
    let view_right = scroll_x + visible_width;
    let view_top = scroll_y;
    let view_bottom = scroll_y + visible_height;

    // If the caret is fully inside the visible region, no scrolling needed.
    if caret_left >= view_left && caret_right <= view_right && caret_top >= view_top && caret_bottom <= view_bottom {
        return None;
    }

    // Calculate minimal adjustments to bring caret into view.
    let mut new_scroll_x = scroll_x;
    let mut new_scroll_y = scroll_y;

    // Horizontal adjustment.
    if caret_left < view_left {
        new_scroll_x = caret_left;
    } else if caret_right > view_right {
        new_scroll_x = caret_right - visible_width;
    }

    // Vertical adjustment.
    if caret_top < view_top {
        new_scroll_y = caret_top;
    } else if caret_bottom > view_bottom {
        new_scroll_y = caret_bottom - visible_height;
    }

    // Clamp to valid scroll range.
    let max_scroll_x = (content_width - visible_width).max(0.0);
    let max_scroll_y = (content_height - visible_height).max(0.0);
    new_scroll_x = new_scroll_x.clamp(0.0, max_scroll_x);
    new_scroll_y = new_scroll_y.clamp(0.0, max_scroll_y);

    // Only return if something changed.
    if (new_scroll_x - scroll_x).abs() > 0.5 || (new_scroll_y - scroll_y).abs() > 0.5 {
        Some((new_scroll_x, new_scroll_y))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::compute_scroll_to_keep_caret_visible;

    const FONT_W: f32 = 8.0;
    const FONT_H: f32 = 16.0;

    /// Caret at (0, 0), viewport scrolled down - should scroll up to show row 0.
    #[test]
    fn caret_at_top_scrolled_down_scrolls_up() {
        let result = compute_scroll_to_keep_caret_visible(
            0, 0, // caret at (0, 0)
            FONT_W, FONT_H, 0.0, 16.0, // scroll_y = 1 row down
            640.0, 400.0, // visible area
            640.0, 4432.0, // content (80x277)
        );

        assert!(result.is_some(), "Expected scrolling to be triggered");
        let (_, new_scroll_y) = result.unwrap();
        assert!(new_scroll_y < 0.5, "Expected scroll_y near 0, got {new_scroll_y}");
    }

    /// Caret at (0, 1), viewport at scroll_y = 0 - caret is visible, no scroll.
    #[test]
    fn caret_visible_no_scroll() {
        let result = compute_scroll_to_keep_caret_visible(0, 1, FONT_W, FONT_H, 0.0, 0.0, 640.0, 400.0, 640.0, 4432.0);

        assert!(result.is_none(), "Caret is visible, should not scroll");
    }

    /// Caret at bottom row (276), viewport at top - should scroll down.
    #[test]
    fn caret_at_bottom_scrolled_to_top_scrolls_down() {
        let content_height = 277.0 * FONT_H; // 277 rows
        let visible_height = 400.0;

        let result = compute_scroll_to_keep_caret_visible(79, 276, FONT_W, FONT_H, 0.0, 0.0, 640.0, visible_height, 640.0, content_height);

        assert!(result.is_some(), "Expected scrolling to be triggered");
        let (_, new_scroll_y) = result.unwrap();
        let expected = 276.0 * FONT_H + FONT_H - visible_height;
        assert!((new_scroll_y - expected).abs() < 1.0, "Expected scroll_y near {expected}, got {new_scroll_y}");
    }

    /// Caret at right edge when scrolled left - should scroll right.
    #[test]
    fn caret_at_right_edge_scrolls_horizontally() {
        let content_width = 80.0 * FONT_W;
        let visible_width = 400.0;

        let result = compute_scroll_to_keep_caret_visible(79, 0, FONT_W, FONT_H, 0.0, 0.0, visible_width, 400.0, content_width, 4432.0);

        assert!(result.is_some(), "Expected horizontal scrolling");
        let (new_scroll_x, _) = result.unwrap();
        let expected = 80.0 * FONT_W - visible_width;
        assert!((new_scroll_x - expected).abs() < 1.0, "Expected scroll_x near {expected}, got {new_scroll_x}");
    }

    /// Caret at left edge when scrolled right - should scroll left.
    #[test]
    fn caret_at_left_edge_scrolls_left() {
        let result = compute_scroll_to_keep_caret_visible(0, 10, FONT_W, FONT_H, 100.0, 0.0, 400.0, 400.0, 640.0, 4432.0);

        assert!(result.is_some(), "Expected horizontal scrolling");
        let (new_scroll_x, _) = result.unwrap();
        assert!(new_scroll_x < 0.5, "Expected scroll_x near 0, got {new_scroll_x}");
    }

    /// Max scroll clamp: don't scroll past content bounds.
    #[test]
    fn scroll_clamped_to_max() {
        let content_height = 100.0;
        let visible_height = 400.0;

        let result = compute_scroll_to_keep_caret_visible(0, 0, FONT_W, FONT_H, 0.0, 50.0, 640.0, visible_height, 640.0, content_height);

        if let Some((_, new_scroll_y)) = result {
            assert!(new_scroll_y < 0.5, "Expected scroll_y clamped to 0, got {new_scroll_y}");
        }
    }
}
