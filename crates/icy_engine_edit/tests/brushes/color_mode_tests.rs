//! Tests for ColorMode

use icy_engine_edit::brushes::ColorMode;

#[test]
fn test_affects_foreground() {
    assert!(!ColorMode::None.affects_foreground());
    assert!(ColorMode::Foreground.affects_foreground());
    assert!(!ColorMode::Background.affects_foreground());
    assert!(ColorMode::Both.affects_foreground());
}

#[test]
fn test_affects_background() {
    assert!(!ColorMode::None.affects_background());
    assert!(!ColorMode::Foreground.affects_background());
    assert!(ColorMode::Background.affects_background());
    assert!(ColorMode::Both.affects_background());
}

#[test]
fn test_affects_any() {
    assert!(!ColorMode::None.affects_any());
    assert!(ColorMode::Foreground.affects_any());
    assert!(ColorMode::Background.affects_any());
    assert!(ColorMode::Both.affects_any());
}
