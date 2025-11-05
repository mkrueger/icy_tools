use std::{
    fs::{self, File},
    io,
};

use icy_engine::{Buffer, BufferParser, Caret, Color};

//#[test]
pub fn test_igs() {
    let mut img_buf = [0; 320 * 200 * 4];
    for entry in fs::read_dir("tests/igs/lowres").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ig" {
            continue;
        }

        let png_file = cur_entry.with_extension("png");
        let reader = io::BufReader::new(File::open(&png_file).unwrap());
        png::Decoder::new(reader).read_info().unwrap().next_frame(&mut img_buf).unwrap();

        let data = fs::read_to_string(cur_entry).expect("Error reading file.");
        let mut buffer = Buffer::new((80, 24));
        let mut caret = Caret::default();
        let mut parser = icy_engine::parsers::igs::Parser::new(icy_engine::igs::TerminalResolution::Low);

        for c in data.chars() {
            parser.print_char(&mut buffer, 0, &mut caret, c).unwrap();
        }

        let (_, rendered_data) = parser.get_picture_data().unwrap();

        check_output(&rendered_data, &img_buf);
    }
}

fn check_output(rendered_data: &[u8], img_buf: &[u8]) {
    assert_eq!(rendered_data.len(), img_buf.len());
    for y in 0..200 {
        for x in 0..320 {
            let idx = (y * 320 + x) * 4;
            let col1 = Color::new(rendered_data[idx], rendered_data[idx + 1], rendered_data[idx + 2]);

            let idx = (y * 320 + x) * 3;
            let col2 = Color::new(img_buf[idx], img_buf[idx + 1], img_buf[idx + 2]);

            if col1 != col2 {
                panic!("Mismatch pixel at x: {}, y: {}. Expected: {:?}, got: {:?}", x, y, col2, col1);
            }
        }
    }
}
