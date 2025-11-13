use icy_engine::{FORMATS, SaveOptions, TextBuffer, TextPane};

#[test]
fn test_clear() {
    let buf = TextBuffer::from_bytes(&std::path::PathBuf::from("test.avt"), false, &[b'X', 12, b'X'], None, None).unwrap();
    assert_eq!(1, buf.get_line_count());
    assert_eq!(1, buf.get_real_buffer_width());
}

#[test]
fn test_repeat() {
    let buf = TextBuffer::from_bytes(&std::path::PathBuf::from("test.avt"), false, &[b'X', 25, b'b', 3, b'X'], None, None).unwrap();
    assert_eq!(1, buf.get_line_count());
    assert_eq!(5, buf.get_real_buffer_width());
    assert_eq!(b'X', buf.get_char((0, 0).into()).ch as u8);
    assert_eq!(b'b', buf.get_char((1, 0).into()).ch as u8);
    assert_eq!(b'b', buf.get_char((2, 0).into()).ch as u8);
    assert_eq!(b'b', buf.get_char((3, 0).into()).ch as u8);
    assert_eq!(b'X', buf.get_char((4, 0).into()).ch as u8);
}

#[test]
fn test_zero_repeat() {
    let buf = TextBuffer::from_bytes(&std::path::PathBuf::from("test.avt"), false, &[25, b'b', 0], None, None).unwrap();
    assert_eq!(0, buf.get_line_count());
    assert_eq!(0, buf.get_real_buffer_width());
}

#[test]
fn test_linebreak_bug() {
    let buf = TextBuffer::from_bytes(
        &std::path::PathBuf::from("test.avt"),
        false,
        &[
            12, 22, 1, 8, 32, 88, 22, 1, 15, 88, 25, 32, 4, 88, 22, 1, 8, 88, 32, 32, 32, 22, 1, 3, 88, 88, 22, 1, 57, 88, 88, 88, 25, 88, 7, 22, 1, 9, 25, 88,
            4, 22, 1, 25, 88, 88, 88, 88, 88, 88, 22, 1, 1, 25, 88, 13,
        ],
        None,
        None,
    )
    .unwrap();
    assert_eq!(1, buf.get_line_count());
    assert_eq!(47, buf.get_real_buffer_width());
}

fn output_avt(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut prev = 0;

    for d in data {
        match d {
            12 => result.extend_from_slice(b"^L"),
            25 => result.extend_from_slice(b"^Y"),
            22 => result.extend_from_slice(b"^V"),
            _ => {
                if prev == 22 {
                    match d {
                        1 => result.extend_from_slice(b"<SET_COLOR>"),
                        2 => result.extend_from_slice(b"<BLINK_ON>"),
                        3 => result.extend_from_slice(b"<MOVE_UP>"),
                        4 => result.extend_from_slice(b"<MOVE_DOWN>"),
                        5 => result.extend_from_slice(b"<MOVE_RIGHT"),
                        6 => result.extend_from_slice(b"<MOVE_LEFT>"),
                        7 => result.extend_from_slice(b"<CLR_EOL>"),
                        8 => result.extend_from_slice(b"<GOTO_XY>"),
                        _ => result.extend_from_slice(b"<UNKNOWN_CMD>"),
                    }
                    prev = *d;
                    continue;
                }

                result.push(*d);
            }
        }
        prev = *d;
    }
    result
}

fn test_avt(data: &[u8]) {
    let mut buf = TextBuffer::from_bytes(&std::path::PathBuf::from("test.avt"), false, data, None, None).unwrap();
    let converted = FORMATS[7].to_bytes(&mut buf, &SaveOptions::new()).unwrap();

    // more gentle output.
    let b: Vec<u8> = output_avt(&converted);
    let converted = String::from_utf8_lossy(b.as_slice());

    let b: Vec<u8> = output_avt(data);
    let expected = String::from_utf8_lossy(b.as_slice());

    assert_eq!(expected, converted);
}

#[test]
fn test_char_compression() {
    let data = b"\x16\x01\x07A-A--A---A\x19-\x04A\x19-\x05A\x19-\x06A\x19-\x07A";
    test_avt(data);
}
