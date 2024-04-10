use std::fs;

use bstr::BString;
use chrono::NaiveDate;
use icy_sauce::{
    char_caps::{CharCaps, ContentType},
    SauceInformation, SauceInformationBuilder,
};

#[test]
fn test_simple_file() {
    let file = fs::read("tests/files/test1.pcb").unwrap();
    let info = SauceInformation::read(&file).unwrap().unwrap();
    assert!(info.comments().is_empty());
    assert_eq!(info.info_len(), 129);
    assert_eq!(info.title(), &BString::from("Title"));
    assert_eq!(info.group(), &BString::from("Group"));
    assert_eq!(info.author(), &BString::from("Author"));

    let caps = info.get_character_capabilities().unwrap();
    assert_eq!(caps.content_type, ContentType::PCBoard);
    assert_eq!(caps.width, 40);
    assert_eq!(caps.height, 42);
    assert!(!caps.use_aspect_ratio);
    assert!(!caps.use_ice);
    assert!(!caps.use_letter_spacing);
}

#[test]
fn test_comments() {
    let file = fs::read("tests/files/test2.ans").unwrap();
    let info = SauceInformation::read(&file).unwrap().unwrap();
    assert_eq!(info.comments().len(), 2);
    assert_eq!(info.info_len(), 129 + 2 * 64 + 5);
    assert_eq!(info.title(), &BString::from("Title"));
    assert_eq!(info.group(), &BString::from("Group"));
    assert_eq!(info.author(), &BString::from("Author"));

    assert_eq!(info.comments()[0], BString::from("+9px & AR"));
    assert_eq!(info.comments()[1], BString::from("and 2 Comments!!!!"));

    let caps = info.get_character_capabilities().unwrap();
    assert_eq!(caps.content_type, ContentType::Ansi);
    assert_eq!(caps.width, 80);
    assert_eq!(caps.height, 25);
    assert!(caps.use_aspect_ratio);
    assert!(!caps.use_ice); // Not set in the file
    assert!(caps.use_letter_spacing);
}

#[test]
fn test_write1() {
    let file = fs::read("tests/files/test1.pcb").unwrap();
    let info = SauceInformation::read(&file).unwrap().unwrap();

    let mut write_to = Vec::new();
    info.write(&mut write_to, 0).unwrap();
    let info2 = SauceInformation::read(&write_to).unwrap().unwrap();

    assert_eq!(info.title(), info2.title());
    assert_eq!(info.group(), info2.group());
    assert_eq!(info.author(), info2.author());
}

#[test]
fn test_write2() {
    let file = fs::read("tests/files/test2.ans").unwrap();
    let info = SauceInformation::read(&file).unwrap().unwrap();

    let mut write_to = Vec::new();
    info.write(&mut write_to, 0).unwrap();
    let info2 = SauceInformation::read(&write_to).unwrap().unwrap();

    assert_eq!(info.title(), info2.title());
    assert_eq!(info.group(), info2.group());
    assert_eq!(info.author(), info2.author());
}

#[test]
fn test_builder() {
    let builder = SauceInformationBuilder::default()
        .with_title("Title".into())
        .unwrap()
        .with_author("Author".into())
        .unwrap()
        .with_group("Group".into())
        .unwrap()
        .with_date(NaiveDate::from_ymd_opt(1976, 12, 30).unwrap())
        .with_data_type(icy_sauce::SauceDataType::XBin)
        .with_char_caps(CharCaps {
            content_type: ContentType::Ansi,
            width: 112,
            height: 90,
            use_ice: false,
            use_letter_spacing: false,
            use_aspect_ratio: false,
            font_opt: None,
        })
        .unwrap();

    let mut write_to = Vec::new();
    builder.build().write(&mut write_to, 0).unwrap();
    let info2 = SauceInformation::read(&write_to).unwrap().unwrap();

    assert_eq!(info2.title(), &BString::from("Title"));
    assert_eq!(info2.group(), &BString::from("Group"));
    assert_eq!(info2.author(), &BString::from("Author"));
    assert_eq!(info2.get_data_type(), icy_sauce::SauceDataType::XBin);
    assert_eq!(info2.get_date().unwrap(), NaiveDate::from_ymd_opt(1976, 12, 30).unwrap());
    let caps = info2.get_character_capabilities().unwrap();
    assert_eq!(caps.width, 112);
    assert_eq!(caps.height, 90);
}

#[test]
fn test_build_comments() {
    let builder = SauceInformationBuilder::default()
        .with_title("Title".into())
        .unwrap()
        .with_author("Author".into())
        .unwrap()
        .with_group("Group".into())
        .unwrap()
        .with_comment(BString::new("This is a comment".into()))
        .unwrap()
        .with_comment(BString::new("This is another comment".into()))
        .unwrap();

    let mut write_to = Vec::new();
    builder.build().write(&mut write_to, 0).unwrap();
    let info2 = SauceInformation::read(&write_to).unwrap().unwrap();

    assert_eq!(info2.title(), &BString::from("Title"));
    assert_eq!(info2.group(), &BString::from("Group"));
    assert_eq!(info2.author(), &BString::from("Author"));
    assert_eq!(info2.comments().len(), 2);
    assert_eq!(info2.comments()[0], BString::from("This is a comment"));
    assert_eq!(info2.comments()[1], BString::from("This is another comment"));
}
