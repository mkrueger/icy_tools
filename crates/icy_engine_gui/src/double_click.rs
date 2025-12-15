//! Double-click detection utility
//!
//! Provides a simple abstraction for detecting double-clicks in Iced,
//! which doesn't have built-in double-click support.

use std::time::Instant;

/// Default time window for double-click detection (in milliseconds)
pub const DEFAULT_DOUBLE_CLICK_MS: u128 = 400;

/// Detects double-clicks by tracking the time and item of the last click.
///
/// # Usage
/// ```ignore
/// struct MyComponent {
///     double_click: DoubleClickDetector<usize>,
/// }
///
/// fn on_click(&mut self, index: usize) {
///     if self.double_click.is_double_click(index) {
///         // Handle double-click
///     } else {
///         // Handle single click
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct DoubleClickDetector<T: PartialEq + Clone> {
    /// Last click time and item
    last_click: Option<(Instant, T)>,
    /// Time window for double-click detection (in milliseconds)
    threshold_ms: u128,
}

impl<T: PartialEq + Clone> Default for DoubleClickDetector<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PartialEq + Clone> DoubleClickDetector<T> {
    /// Create a new detector with the default threshold (400ms)
    pub fn new() -> Self {
        Self {
            last_click: None,
            threshold_ms: DEFAULT_DOUBLE_CLICK_MS,
        }
    }

    /// Create a new detector with a custom threshold
    pub fn with_threshold_ms(threshold_ms: u128) -> Self {
        Self {
            last_click: None,
            threshold_ms,
        }
    }

    /// Check if the current click is a double-click on the same item.
    ///
    /// Returns `true` if:
    /// 1. There was a previous click
    /// 2. The previous click was on the same item
    /// 3. The time since the previous click is less than the threshold
    ///
    /// After returning `true`, the detector is reset so subsequent clicks
    /// start fresh (prevents triple-click triggering double-click twice).
    pub fn is_double_click(&mut self, item: T) -> bool {
        let now = Instant::now();

        let is_double = self.last_click.as_ref().map_or(false, |(last_time, last_item)| {
            *last_item == item && now.duration_since(*last_time).as_millis() < self.threshold_ms
        });

        if is_double {
            // Reset after double-click detected
            self.last_click = None;
        } else {
            // Update last click
            self.last_click = Some((now, item));
        }

        is_double
    }

    /// Reset the detector state
    pub fn reset(&mut self) {
        self.last_click = None;
    }

    /// Check if a click was recently registered (useful for hover effects)
    pub fn has_pending_click(&self) -> bool {
        self.last_click.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_single_click() {
        let mut detector = DoubleClickDetector::new();
        assert!(!detector.is_double_click(1));
    }

    #[test]
    fn test_double_click() {
        let mut detector = DoubleClickDetector::new();
        assert!(!detector.is_double_click(1));
        assert!(detector.is_double_click(1));
    }

    #[test]
    fn test_different_items() {
        let mut detector = DoubleClickDetector::new();
        assert!(!detector.is_double_click(1));
        assert!(!detector.is_double_click(2)); // Different item, not double-click
    }

    #[test]
    fn test_timeout() {
        let mut detector = DoubleClickDetector::with_threshold_ms(50);
        assert!(!detector.is_double_click(1));
        sleep(Duration::from_millis(100));
        assert!(!detector.is_double_click(1)); // Too slow
    }

    #[test]
    fn test_triple_click_resets() {
        let mut detector = DoubleClickDetector::new();
        assert!(!detector.is_double_click(1)); // First click
        assert!(detector.is_double_click(1)); // Double-click (resets)
        assert!(!detector.is_double_click(1)); // Third click starts fresh
    }
}
