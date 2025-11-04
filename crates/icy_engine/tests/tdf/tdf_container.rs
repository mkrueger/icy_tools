use icy_engine::font::TheDrawFont;

use super::FontType;
const TEST_FONT: &[u8] = include_bytes!("CODERX.TDF");

#[test]
fn test_load() {
    let result = TheDrawFont::from_bytes(TEST_FONT).unwrap();
    for r in &result {
        assert!(matches!(r.font_type, FontType::Color));
    }
    assert_eq!(6, result.len());
    assert_eq!("Coder Blue", result[0].name);
    assert_eq!("Coder Green", result[1].name);
    assert_eq!("Coder Margen", result[2].name);
    assert_eq!("Coder Purple", result[3].name);
    assert_eq!("Coder Red", result[4].name);
    assert_eq!("Coder Silver", result[5].name);
}

#[test]
fn test_load_save_multi() {
    let result = TheDrawFont::from_bytes(TEST_FONT).unwrap();
    let bundle = TheDrawFont::create_font_bundle(&result).unwrap();
    let result = TheDrawFont::from_bytes(&bundle).unwrap();
    for r in &result {
        assert!(matches!(r.font_type, FontType::Color));
    }
    assert_eq!(6, result.len());
    assert_eq!("Coder Blue", result[0].name);
    assert_eq!("Coder Green", result[1].name);
    assert_eq!("Coder Margen", result[2].name);
    assert_eq!("Coder Purple", result[3].name);
    assert_eq!("Coder Red", result[4].name);
    assert_eq!("Coder Silver", result[5].name);
}
