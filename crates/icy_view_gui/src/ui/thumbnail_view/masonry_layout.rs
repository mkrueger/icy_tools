//! Masonry layout algorithm for thumbnail grid
//!
//! This module implements a Pinterest-style masonry layout where items are
//! placed in the shortest column, creating a visually pleasing grid with
//! varying item heights.

/// Configuration for the masonry layout
#[derive(Debug, Clone)]
pub struct MasonryConfig {
    /// Width of each column (tile width)
    pub column_width: f32,
    /// Spacing between columns and rows
    pub spacing: f32,
    /// Left margin from container edge
    pub left_margin: f32,
    /// Number of columns
    pub num_columns: usize,
}

impl MasonryConfig {
    /// Create a new masonry configuration
    pub fn new(column_width: f32, spacing: f32, left_margin: f32, num_columns: usize) -> Self {
        Self {
            column_width,
            spacing,
            left_margin,
            num_columns: num_columns.max(1),
        }
    }

    /// Calculate the number of columns that fit in the given width
    pub fn columns_for_width(available_width: f32, column_width: f32, spacing: f32, margin: f32) -> usize {
        let usable_width = available_width - margin * 2.0;
        // N * column_width + (N-1) * spacing <= usable_width
        // N * (column_width + spacing) - spacing <= usable_width
        // N <= (usable_width + spacing) / (column_width + spacing)
        ((usable_width + spacing) / (column_width + spacing)).floor().max(1.0) as usize
    }
}

/// A positioned item in the masonry layout
#[derive(Debug, Clone)]
pub struct MasonryItem {
    /// Index in the source list
    pub index: usize,
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
    /// Column this item is in (0-based)
    #[allow(dead_code)]
    pub column: usize,
}

/// Result of a masonry layout calculation
#[derive(Debug, Clone)]
pub struct MasonryLayout {
    /// Positioned items
    pub items: Vec<MasonryItem>,
    /// Total content height
    pub content_height: f32,
    /// Column heights (for debugging/extension)
    #[allow(dead_code)]
    pub column_heights: Vec<f32>,
}

/// Input for an item to be laid out
pub struct ItemSize {
    /// Index in the source list
    pub index: usize,
    /// How many columns this item spans (1 = single, 2 = double width, etc.)
    pub column_span: usize,
    /// Height of the item in pixels
    pub height: f32,
}

/// Calculate masonry layout for items
///
/// Items are placed in the shortest column to create an even distribution.
/// Multi-column items are placed starting at the first column that can fit them.
pub fn calculate_masonry_layout(config: &MasonryConfig, items: &[ItemSize]) -> MasonryLayout {
    let mut column_heights: Vec<f32> = vec![0.0; config.num_columns];
    let mut layout_items = Vec::with_capacity(items.len());

    for item in items {
        let span = item.column_span.min(config.num_columns).max(1);

        // Calculate item width
        let item_width = if span == 1 {
            config.column_width
        } else {
            config.column_width * span as f32 + config.spacing * (span - 1) as f32
        };

        // Find the best starting column
        let best_col = if span == 1 {
            // Single-column: find the shortest column
            column_heights
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0)
        } else {
            // Multi-column: find the position where max height of spanned columns is minimal
            let mut best_start = 0;
            let mut best_max_height = f32::MAX;

            for start in 0..=(config.num_columns - span) {
                let max_height = column_heights[start..start + span].iter().cloned().fold(0.0f32, f32::max);
                if max_height < best_max_height {
                    best_max_height = max_height;
                    best_start = start;
                }
            }
            best_start
        };

        // Calculate position
        let x = config.left_margin + best_col as f32 * (config.column_width + config.spacing);
        let y = if span == 1 {
            column_heights[best_col]
        } else {
            // For multi-column, start at the max height of all spanned columns
            column_heights[best_col..best_col + span].iter().cloned().fold(0.0f32, f32::max)
        };

        layout_items.push(MasonryItem {
            index: item.index,
            x,
            y,
            width: item_width,
            height: item.height,
            column: best_col,
        });

        // Update column heights
        let new_bottom = y + item.height + config.spacing;
        for col in best_col..best_col + span {
            column_heights[col] = new_bottom;
        }
    }

    // Content height is the maximum column height (minus trailing spacing)
    let content_height = column_heights.iter().cloned().fold(0.0f32, f32::max);

    MasonryLayout {
        items: layout_items,
        content_height,
        column_heights,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_column() {
        let config = MasonryConfig::new(100.0, 10.0, 5.0, 1);
        let items = vec![
            ItemSize {
                index: 0,
                column_span: 1,
                height: 50.0,
            },
            ItemSize {
                index: 1,
                column_span: 1,
                height: 75.0,
            },
        ];

        let layout = calculate_masonry_layout(&config, &items);

        assert_eq!(layout.items.len(), 2);
        assert_eq!(layout.items[0].y, 0.0);
        assert_eq!(layout.items[1].y, 60.0); // 50 + 10 spacing
    }

    #[test]
    fn test_two_columns_balanced() {
        let config = MasonryConfig::new(100.0, 10.0, 5.0, 2);
        let items = vec![
            ItemSize {
                index: 0,
                column_span: 1,
                height: 100.0,
            },
            ItemSize {
                index: 1,
                column_span: 1,
                height: 50.0,
            },
            ItemSize {
                index: 2,
                column_span: 1,
                height: 30.0,
            },
        ];

        let layout = calculate_masonry_layout(&config, &items);

        // First item goes to column 0
        assert_eq!(layout.items[0].column, 0);
        // Second item goes to column 1 (shorter)
        assert_eq!(layout.items[1].column, 1);
        // Third item goes to column 1 (still shorter: 60 vs 110)
        assert_eq!(layout.items[2].column, 1);
    }

    #[test]
    fn test_multi_column_span() {
        let config = MasonryConfig::new(100.0, 10.0, 5.0, 3);
        let items = vec![
            ItemSize {
                index: 0,
                column_span: 2,
                height: 100.0,
            },
            ItemSize {
                index: 1,
                column_span: 1,
                height: 50.0,
            },
        ];

        let layout = calculate_masonry_layout(&config, &items);

        // First item spans 2 columns, width = 100 + 10 + 100 = 210
        assert_eq!(layout.items[0].width, 210.0);
        // Second item goes to column 2 (columns 0-1 are blocked)
        assert_eq!(layout.items[1].column, 2);
    }
}
