use icy_parser_core::{
    IgsCommand, IgsCommandItem, IgsItem, IgsParameter, IgsText, encode_igs_command, encode_igs_stream, encode_igs_stream_checked, parse_igs_commands,
    parse_igs_stream,
};
use std::fs;

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir("benches/igs_data")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "IG"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| (path.file_name().unwrap().to_string_lossy().to_string(), fs::read(&path).unwrap()))
        .collect()
}

fn commands(items: &[IgsItem]) -> Vec<IgsCommand> {
    items.iter().filter_map(IgsItem::command).cloned().collect()
}

fn line(x1: i32, y1: i32, x2: i32, y2: i32) -> IgsCommand {
    IgsCommand::Line {
        x1: x1.into(),
        y1: y1.into(),
        x2: x2.into(),
        y2: y2.into(),
    }
}

#[test]
fn fixtures_round_trip_byte_exact() {
    for (name, bytes) in fixtures() {
        let items = parse_igs_stream(&bytes);
        assert_eq!(commands(&items), parse_igs_commands(&bytes), "{name}: segmented commands differ");
        assert!(
            items.iter().any(|item| matches!(item, IgsItem::Command(_))),
            "{name}: stream fell back to plain text"
        );
        assert_eq!(encode_igs_stream_checked(&items).unwrap(), bytes, "{name}: bytes changed");
    }
}

#[test]
fn line_breaks_are_attached_to_commands() {
    let items = parse_igs_stream(b"G#L>1,2,3,4:\r\nG#O>5,6,7:\r\n");
    assert_eq!(items.len(), 2);
    let IgsItem::Command(first) = &items[0] else { panic!("expected a command") };
    assert_eq!(first.source(), Some(&b"G#L>1,2,3,4:"[..]));
    assert_eq!(first.trailing(), b"\r\n");
}

#[test]
fn edited_fixture_commands_are_reencoded() {
    for (name, bytes) in fixtures() {
        let mut items = parse_igs_stream(&bytes);
        let mut edited = 0;
        for item in &mut items {
            if let IgsItem::Command(command) = item
                && let IgsCommand::Circle { x, y, radius } = command.command().clone()
            {
                command.set_command(IgsCommand::Circle {
                    x,
                    y,
                    radius: IgsParameter::Value(radius.evaluate(&Default::default(), 0, 0) + 1),
                });
                edited += 1;
            }
        }
        let encoded = encode_igs_stream_checked(&items).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(commands(&parse_igs_stream(&encoded)), commands(&items), "{name}");
        if edited == 0 {
            assert_eq!(encoded, bytes, "{name}");
        }
    }
}

#[test]
fn chained_commands_get_a_prefix_when_moved() {
    let mut items = parse_igs_stream(b"G#C>1,2:L>1,2,3,4:");
    assert_eq!(items.len(), 2);
    items.swap(0, 1);
    let encoded = encode_igs_stream_checked(&items).unwrap();
    assert_eq!(encoded, b"G#L>1,2,3,4:C>1,2:");

    items.insert(1, IgsItem::Text(IgsText::new(b"hello".to_vec())));
    let encoded = encode_igs_stream_checked(&items).unwrap();
    assert_eq!(encoded, b"G#L>1,2,3,4:\nhelloG#C>1,2:");
}

#[test]
fn new_commands_are_written_one_per_line() {
    let items: Vec<IgsItem> = vec![line(0, 0, 10, 10).into(), line(10, 10, 20, 0).into()];
    let encoded = encode_igs_stream_checked(&items).unwrap();
    assert_eq!(encoded, b"G#L>0,0,10,10:\nG#L>10,10,20,0:\n");
    assert_eq!(commands(&parse_igs_stream(&encoded)), commands(&items));
}

#[test]
fn text_bytes_are_written_exactly() {
    let command = IgsCommand::WriteText {
        x: 1.into(),
        y: 2.into(),
        text: vec![b'A', 0x9e, 0xe1],
    };
    let bytes = encode_igs_command(&command).unwrap();
    assert_eq!(bytes, b"G#W>1,2,A\x9e\xe1@");
    assert_eq!(parse_igs_commands(&bytes), vec![command]);
}

#[test]
fn text_after_a_chained_command_is_separated() {
    let mut items = parse_igs_stream(b"G#C>1,2:L>1,2,3,4:");
    items.insert(1, IgsItem::Text(IgsText::new(b"Hello".to_vec())));
    let encoded = encode_igs_stream_checked(&items).unwrap();
    assert_eq!(encoded, b"G#C>1,2:\nHelloG#L>1,2,3,4:");
    let reparsed = parse_igs_stream(&encoded);
    assert!(matches!(&reparsed[1], IgsItem::Text(text) if text.bytes == b"\nHello" && !text.invalid));

    // Control bytes are kept too; parsed text that starts after a command stays as it was.
    let mut items = parse_igs_stream(b"G#C>1,2:");
    items.push(IgsItem::Text(IgsText::new(b"\x1bE".to_vec())));
    assert_eq!(encode_igs_stream_checked(&items).unwrap(), b"G#C>1,2:\n\x1bE");
    let parsed = b"G#C>1,2:\rHi";
    let items = parse_igs_stream(parsed);
    assert!(matches!(&items[1], IgsItem::Text(text) if text.chained));
    assert_eq!(encode_igs_stream_checked(&items).unwrap(), parsed);
}

#[test]
fn fill_patterns_are_written_without_a_terminator() {
    let command = IgsCommand::LoadFillPattern {
        pattern: 2,
        data: (0..16).map(|row| 0x8001u16.rotate_left(row)).collect(),
    };
    let bytes = encode_igs_command(&command).unwrap();
    assert!(bytes.ends_with(b"@"));
    let items: Vec<IgsItem> = vec![command.clone().into(), line(0, 0, 1, 1).into()];
    let encoded = encode_igs_stream_checked(&items).unwrap();
    assert_eq!(commands(&parse_igs_stream(&encoded)), commands(&items));
}

#[test]
fn unrepresentable_commands_are_rejected() {
    let command = IgsCommand::WriteText {
        x: 1.into(),
        y: 2.into(),
        text: b"a@b".to_vec(),
    };
    assert!(encode_igs_command(&command).is_none());
    assert!(encode_igs_stream(&[IgsItem::Command(IgsCommandItem::new(command))]).is_err());
}

#[test]
fn unparsable_input_is_kept_as_text() {
    let bytes = b"Hello\x1bY  G#L>1,2,3,4:G#~:";
    let items = parse_igs_stream(bytes);
    assert!(items.iter().any(|item| matches!(item, IgsItem::Text(_))));
    assert_eq!(encode_igs_stream(&items).unwrap(), bytes);
}

#[test]
fn fixture_commands_have_canonical_encodings() {
    for (name, bytes) in fixtures() {
        for command in commands(&parse_igs_stream(&bytes)) {
            assert!(encode_igs_command(&command).is_some(), "{name}: {command:?}");
        }
    }
}
