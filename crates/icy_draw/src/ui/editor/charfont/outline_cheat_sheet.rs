//! Outline Cheat Sheet Widget
//!
//! Displays a reference table showing the mapping between:
//! - Keys: F1-F10, 1-8
//! - Codes: A-O, @, &, ÷
//! - Results: The actual outline characters (CP437 codes)

use iced::{
    Element, Font, Length,
    widget::{column, container, row, text},
};

/// CP437 Unicode mappings for outline characters
/// These are the Unicode equivalents of CP437 box drawing characters
const OUTLINE_RESULTS: &[&str] = &[
    "═",  // F1: A - Double horizontal (CP437: 0xCD)
    "─",  // F2: B - Single horizontal (CP437: 0xC4)  
    "│",  // F3: C - Single vertical (CP437: 0xB3)
    "║",  // F4: D - Double vertical (CP437: 0xBA)
    "╒",  // F5: E - Down-right double-single (CP437: 0xD5)
    "╗",  // F6: F - Down-left double (CP437: 0xBB)
    "╓",  // F7: G - Down-right single-double (CP437: 0xD6)
    "┐",  // F8: H - Down-left single (CP437: 0xBF)
    "╚",  // F9: I - Up-left double (CP437: 0xC8)
    "╜",  // F10: J - Up-left double-single (CP437: 0xBE)
    "└",  // 1: K - Up-left single (CP437: 0xC0)
    "╜",  // 2: L - Up-left variant (CP437: 0xBD)
    "╡",  // 3: M - Vertical-left double-single (CP437: 0xB5)
    "╟",  // 4: N - Vertical-right single-double (CP437: 0xC7)
    "SP", // 5: O - Space
    "@",  // 6: @ 
    "&",  // 7: &
    "÷",  // 8: Division sign (CP437: 0xF7)
];

const OUTLINE_KEYS: &[&str] = &[
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10",
    "1", "2", "3", "4", "5", "6", "7", "8",
];

const OUTLINE_CODES: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J",
    "K", "L", "M", "N", "O", "@", "&", "÷",
];

/// Create a cheat sheet element for outline font editing
pub fn view_cheat_sheet<'a, Message: 'a>() -> Element<'a, Message> {
    let key_label = crate::fl!("tdf-editor-cheat_sheet_key");
    let code_label = crate::fl!("tdf-editor-cheat_sheet_code");
    let res_label = crate::fl!("tdf-editor-cheat_sheet_res");

    // Use monospace font for consistent spacing
    let mono_font = Font::MONOSPACE;

    // Build key row
    let mut key_row_items: Vec<Element<'_, Message>> = vec![
        text(format!("{:>6}:", key_label)).size(11).font(mono_font).into()
    ];
    for key in OUTLINE_KEYS {
        key_row_items.push(text(format!(" {:>3}", key)).size(11).font(mono_font).into());
    }

    // Build code row  
    let mut code_row_items: Vec<Element<'_, Message>> = vec![
        text(format!("{:>6}:", code_label)).size(11).font(mono_font).into()
    ];
    for code in OUTLINE_CODES {
        code_row_items.push(text(format!(" {:>3}", code)).size(11).font(mono_font).into());
    }

    // Build result row
    let mut res_row_items: Vec<Element<'_, Message>> = vec![
        text(format!("{:>6}:", res_label)).size(11).font(mono_font).into()
    ];
    for result in OUTLINE_RESULTS {
        res_row_items.push(text(format!(" {:>3}", result)).size(11).font(mono_font).into());
    }

    let content = column![
        row(key_row_items).spacing(0),
        row(code_row_items).spacing(0),
        row(res_row_items).spacing(0),
    ]
    .spacing(2);

    container(content)
        .width(Length::Fill)
        .padding(4)
        .into()
}
