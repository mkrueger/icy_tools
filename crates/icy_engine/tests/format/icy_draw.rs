
#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{AttributedChar, Color, Layer, OutputFormat, SaveOptions, TextAttribute, TextBuffer, TextPane, compare_buffers};

    use super::IcyDraw;
    /*
        fn is_hidden(entry: &walkdir::DirEntry) -> bool {
            entry
                .file_name()
                .to_str()
                .map_or(false, |s| s.starts_with('.'))
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
                            for format in &*crate::FORMATS {
                                if format.get_file_extension() == extension
                                    || format.get_alt_extensions().contains(&extension)
                                {
                                    found = true;
                                }
                            }
                            if !found {
                                continue;
                            }
                            num += 1;/*
                            if num < 53430 {
                                continue;
                            }*/
                            if let Ok(mut buf) = Buffer::load_buffer(path, true) {
                                let draw = IcyDraw::default();
                                let bytes = draw.to_bytes(&buf, &SaveOptions::default()).unwrap();
                                let buf2 = draw
                                    .load_buffer(Path::new("test.icy"), &bytes, None)
                                    .unwrap();
                                compare_buffers(&buf, &buf2);
                            }
                        }
                    }
    */
    /*
        #[test]
        fn test_single() {
            // .into()
            let mut buf = Buffer::load_buffer(
                Path::new("../sixteencolors-archive/1996/moz9604a/SHD-SOFT.ANS"),
                true,
            )
            .unwrap();
            let draw = IcyDraw::default();
            let bytes = draw.to_bytes(&buf, &SaveOptions::default()).unwrap();
            let buf2 = draw
                .load_buffer(Path::new("test.icy"), &bytes, None)
                .unwrap();
            compare_buffers(&buf, &buf2);
        }
    */

    #[test]
    fn test_default_font_page() {
        let mut buf = TextBuffer::default();
        buf.layers[0].default_font_page = 12;
        buf.layers.push(Layer::new("test", (80, 25)));
        buf.layers[1].default_font_page = 1;

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_empty_buffer() {
        let mut buf = TextBuffer::default();
        buf.set_width(12);
        buf.set_height(23);

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_rgb_serialization_bug() {
        let mut buf = TextBuffer::new((2, 2));
        let fg = buf.palette.insert_color(Color::new(82, 85, 82));
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: '²',
                attribute: TextAttribute::new(fg, 0),
            },
        );
        let bg = buf.palette.insert_color(Color::new(182, 185, 82));
        buf.layers[0].set_char(
            (1, 0),
            AttributedChar {
                ch: '²',
                attribute: TextAttribute::new(fg, bg),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_rgb_serialization_bug_2() {
        // was a bug in compare_buffers, but having more test doesn't hurt.
        let mut buf = TextBuffer::new((2, 2));

        let _ = buf.palette.insert_color(Color::new(1, 2, 3));
        let fg = buf.palette.insert_color(Color::new(4, 5, 6)); // 17
        let bg = buf.palette.insert_color(Color::new(7, 8, 9)); // 18
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: 'A',
                attribute: TextAttribute::new(fg, bg),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_nonstandard_palettes() {
        // was a bug in compare_buffers, but having more test doesn't hurt.
        let mut buf = TextBuffer::new((2, 2));
        buf.palette.set_color(9, Color::new(4, 5, 6));
        buf.palette.set_color(10, Color::new(7, 8, 9));

        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: 'A',
                attribute: TextAttribute::new(9, 10),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_fg_switch() {
        // was a bug in compare_buffers, but having more test doesn't hurt.
        let mut buf = TextBuffer::new((2, 1));
        let mut attribute = TextAttribute::new(1, 1);
        attribute.set_is_bold(true);
        buf.layers[0].set_char((0, 0), AttributedChar { ch: 'A', attribute });
        buf.layers[0].set_char(
            (1, 0),
            AttributedChar {
                ch: 'A',
                attribute: TextAttribute::new(2, 1),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_escape_char() {
        let mut buf = TextBuffer::new((2, 2));
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: '\x1b',
                attribute: TextAttribute::default(),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_0_255_chars() {
        let mut buf = TextBuffer::new((2, 2));
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: '\0',
                attribute: TextAttribute::default(),
            },
        );
        buf.layers[0].set_char(
            (0, 1),
            AttributedChar {
                ch: '\u{FF}',
                attribute: TextAttribute::default(),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_too_long_lines() {
        let mut buf = TextBuffer::new((2, 2));
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: '1',
                attribute: TextAttribute::default(),
            },
        );
        buf.layers[0].set_char(
            (0, 1),
            AttributedChar {
                ch: '2',
                attribute: TextAttribute::default(),
            },
        );
        buf.layers[0].lines[0].chars.resize(
            80,
            AttributedChar {
                ch: ' ',
                attribute: TextAttribute::default(),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_space_persistance_buffer() {
        let mut buf = TextBuffer::default();
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: ' ',
                attribute: TextAttribute::default(),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_invisible_layer_bug() {
        let mut buf = TextBuffer::new((1, 1));
        buf.layers.push(Layer::new("test", (1, 1)));
        buf.layers[1].set_char((0, 0), AttributedChar::new('a', TextAttribute::default()));
        buf.layers[0].properties.is_visible = false;
        buf.layers[1].properties.is_visible = false;

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let mut buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
        buf2.layers[0].properties.is_visible = true;
        buf2.layers[1].properties.is_visible = true;
    }

    #[test]
    fn test_invisisible_persistance_bug() {
        let mut buf = TextBuffer::new((3, 1));
        buf.layers.push(Layer::new("test", (3, 1)));
        buf.layers[1].set_char((0, 0), AttributedChar::new('a', TextAttribute::default()));
        buf.layers[1].set_char((2, 0), AttributedChar::new('b', TextAttribute::default()));
        buf.layers[0].properties.is_visible = false;
        buf.layers[1].properties.is_visible = false;
        buf.layers[1].properties.has_alpha_channel = true;

        assert_eq!(AttributedChar::invisible(), buf.layers[1].get_char((1, 0).into()).into());

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let mut buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
        buf2.layers[0].properties.is_visible = true;
        buf2.layers[1].properties.is_visible = true;
    }
}
