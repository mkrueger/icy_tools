//! Buffer size limits to prevent memory exhaustion during parsing/viewing
//!
//! These limits prevent arithmetic overflow and excessive memory allocation
//! when parsing malformed or malicious files.

/// Maximum width in characters (columns)
/// 1000 columns is more than enough for any standard terminal format
pub const MAX_BUFFER_WIDTH: i32 = 1000;

/// Maximum height in lines (rows)
/// 5000 lines allows ~250KB of typical ANSI content while preventing
/// memory exhaustion from malformed files
pub const MAX_BUFFER_HEIGHT: i32 = 20000;

/// Check if dimensions are within safe limits
#[inline]
pub fn is_within_limits(width: i32, height: i32) -> bool {
    width > 0 && width <= MAX_BUFFER_WIDTH && height > 0 && height <= MAX_BUFFER_HEIGHT
}

/// Clamp dimensions to safe limits
#[inline]
pub fn clamp_dimensions(width: i32, height: i32) -> (i32, i32) {
    (width.clamp(1, MAX_BUFFER_WIDTH), height.clamp(1, MAX_BUFFER_HEIGHT))
}

/// Clamp a single width value
#[inline]
pub fn clamp_width(width: i32) -> i32 {
    width.clamp(1, MAX_BUFFER_WIDTH)
}

/// Clamp a single height value
#[inline]
pub fn clamp_height(height: i32) -> i32 {
    height.clamp(1, MAX_BUFFER_HEIGHT)
}
