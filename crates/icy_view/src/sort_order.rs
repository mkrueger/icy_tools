use serde::{Deserialize, Serialize};

/// Sort order for file listing
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum SortOrder {
    /// Sort by name (A-Z)
    #[default]
    NameAsc,
    /// Sort by name (Z-A)
    NameDesc,
    /// Sort by size (smallest first)
    SizeAsc,
    /// Sort by size (largest first)
    SizeDesc,
    /// Sort by date (oldest first)
    DateAsc,
    /// Sort by date (newest first)
    DateDesc,
}

impl SortOrder {
    /// Cycle to the next sort order
    pub fn next(&self) -> SortOrder {
        match self {
            SortOrder::NameAsc => SortOrder::NameDesc,
            SortOrder::NameDesc => SortOrder::SizeAsc,
            SortOrder::SizeAsc => SortOrder::SizeDesc,
            SortOrder::SizeDesc => SortOrder::DateAsc,
            SortOrder::DateAsc => SortOrder::DateDesc,
            SortOrder::DateDesc => SortOrder::NameAsc,
        }
    }

    /// Get the icon for this sort order
    pub fn icon(&self) -> &'static str {
        match self {
            SortOrder::NameAsc => "A↓",
            SortOrder::NameDesc => "A↑",
            SortOrder::SizeAsc => "S↓",
            SortOrder::SizeDesc => "S↑",
            SortOrder::DateAsc => "D↓",
            SortOrder::DateDesc => "D↑",
        }
    }
}
