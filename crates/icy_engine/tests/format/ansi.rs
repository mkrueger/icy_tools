/*

use std::{fs, path::Path};

use icy_engine::{AnsiState, Color, ControlCharHandling, FORMATS, SaveOptions, StringGenerator, TextBuffer, TextPane};

use super::ansi2::{CompareOptions, compare_buffers};

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_str().map_or(false, |s| s.starts_with('.'))
}

#[test]
fn test_roundtrip() {
    let walker = walkdir::WalkDir::new("../sixteencolors-archive").into_iter();
    let mut num = 0;

    for entry in walker.filter_entry(|e| !is_hidden(e)) {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            continue;
        }
        let extension = path.extension();
        if extension.is_none() {
            continue;
        }
        let extension = extension.unwrap().to_str();
        if extension.is_none() {
            continue;
        }
        let extension = extension.unwrap().to_lowercase();

        let mut found = false;
        for format in &*FORMATS {
            if format.get_file_extension() == extension || format.get_alt_extensions().contains(&extension) {
                found = true;
            }
        }
        if !found {
            continue;
        }
        num += 1;
        if num < 0 {
            continue;
        }

        let orig_bytes = fs::read(path).unwrap();

        if let Ok(buf) = TextBuffer::from_bytes(path, true, &orig_bytes, None, None) {
            if buf.get_width() != 80 {
                continue;
            }
            if buf.palette.len() > 16 {
                continue;
            }
            let mut opt = SaveOptions::default();
            opt.control_char_handling = ControlCharHandling::IcyTerm;
            opt.compress = true;
            opt.save_sauce = true; // buf.has_sauce() is private
            let mut draw = StringGenerator::new(opt);
            draw.screen_prep(&buf);
            draw.generate(&buf, &buf);
            let state = AnsiState {
                is_bold: false,
                is_blink: false,
                is_faint: false,
                is_italic: false,
                is_underlined: false,
                is_double_underlined: false,
                is_crossed_out: false,
                is_concealed: false,
                fg_idx: 7,
                fg: Color::new(170, 170, 170),
                bg_idx: 0,
                bg: Color::new(0, 0, 0),
            };
            draw.screen_end(&buf, state);
            let bytes = draw.get_data().to_vec();
            let buf2 = TextBuffer::from_bytes(Path::new("test.ans"), true, &bytes, None, None).unwrap();
            if buf.get_height() != buf2.get_height() {
                continue;
            }

            /*
            for x in 23..30 {
                let ch = buf2.layers[0].get_char((x, 0).into());
                "{:?} {:?}", ch, buf2.palette.get_color(ch.attribute.get_foreground()));
            }
            */

            compare_buffers(
                &buf,
                &buf2,
                CompareOptions {
                    compare_palette: false,
                    compare_fonts: false,
                    ignore_invisible_chars: true,
                },
            );
        }
    }
}

*/
