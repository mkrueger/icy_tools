use std::fs;

use icy_engine::{TextBuffer, formats::FileFormat};

const TEST_FILE: &str = "tests/output/ar9px/files/aeleus-usta1.ans";

fn load_test_buffer() -> TextBuffer {
    let path: &std::path::Path = std::path::Path::new(TEST_FILE);
    let data = fs::read(path).expect("Failed to read test file");
    FileFormat::Ansi.from_bytes(&data, None).expect("Failed to parse test file").screen.buffer
}

/// Test 9px font rendering (letter spacing)
#[test]
pub fn test_9px_rendering() {
    crate::init_logging();

    let mut buffer = load_test_buffer();

    // Set letter spacing to trigger 9px font creation
    buffer.set_use_letter_spacing(true);

    crate::compare_buffer_output_with_options(
        &mut buffer,
        std::path::Path::new("tests/output/ar9px/files/aeleus-usta1_9px"),
        true,  // use_letter_spacing
        false, // use_aspect_ratio
    );
}

/*
/// Test aspect ratio rendering
#[test]
pub fn test_aspect_ratio_rendering() {
    crate::init_logging();

    let mut buffer = load_test_buffer();

    crate::compare_buffer_output_with_options(
        &mut buffer,
        std::path::Path::new("tests/output/ar9px/files/aeleus-usta1_ar"),
        false, // use_letter_spacing
        true,  // use_aspect_ratio
    );
}

/// Test combined 9px + aspect ratio rendering
#[test]
pub fn test_9px_and_aspect_ratio_rendering() {
    crate::init_logging();

    let mut buffer = load_test_buffer();

    crate::compare_buffer_output_with_options(
        &mut buffer,
        std::path::Path::new("tests/output/ar9px/files/aeleus-usta1_9pxar"),
        true, // use_letter_spacing
        true, // use_aspect_ratio
    );
}
*/
