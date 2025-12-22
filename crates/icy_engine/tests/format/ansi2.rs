use icy_engine::{AttributedChar, Color, SaveOptions, TextAttribute, TextBuffer, TextPane, formats::FileFormat};

fn test_ansi(data: &[u8]) {
    let mut buf = FileFormat::Ansi.from_bytes(data, None).unwrap().screen.buffer;
    let converted: Vec<u8> = FileFormat::Ansi.to_bytes(&mut buf, &SaveOptions::new()).unwrap();
    // more gentle output.
    let b: Vec<u8> = converted.iter().map(|&x| if x == 27 { b'x' } else { x }).collect();
    let converted: std::borrow::Cow<'_, str> = String::from_utf8_lossy(b.as_slice());

    let b: Vec<u8> = data.iter().map(|&x| if x == 27 { b'x' } else { x }).collect();
    let expected = String::from_utf8_lossy(b.as_slice());

    assert_eq!(expected, converted);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_space_compression() {
    let data = b"A A  A   A    A\x1B[5CA\x1B[6CA\x1B[8CA";
    test_ansi(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_fg_color_change() {
    let data = b"a\x1B[32ma\x1B[33ma\x1B[1ma\x1B[35ma\x1B[0;35ma\x1B[1;32ma\x1B[0;36ma\x1B[32mA";
    test_ansi(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_bg_color_change() {
    let data = b"A\x1B[44mA\x1B[45mA\x1B[31;40mA\x1B[42mA\x1B[40mA\x1B[1;46mA\x1B[0mA\x1B[1;47mA\x1B[0;47mA";
    test_ansi(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_blink_change() {
    let data = b"A\x1B[5mA\x1B[0mA\x1B[1;5;42mA\x1B[0;1;42mA\x1B[0;5mA\x1B[0;36mA\x1B[5;33mA\x1B[0;1mA";
    test_ansi(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_eol_skip() {
    let data = b"\x1B[79C\x1B[1mdd";
    test_ansi(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_23bit() {
    let data = b"\x1B[1;24;12;200t#";
    test_ansi(data);
    let data = b"\x1B[0;44;2;120t#";
    test_ansi(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_extended_color() {
    let data = b"\x1B[38;5;42m#";
    test_ansi(data);
    let data = b"\x1B[48;5;100m#";
    test_ansi(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_first_char_color() {
    let data = b"\x1B[1;36mA";
    test_ansi(data);
    let data = b"\x1B[31mA";
    test_ansi(data);
    let data = b"\x1B[33;45mA\x1B[40m ";
    test_ansi(data);
    let data = b"\x1B[1;33;45mA";
    test_ansi(data);
}

fn test_ansi_ice(data: &[u8]) {
    let mut buf = FileFormat::Ansi.from_bytes(data, None).unwrap().screen.buffer;
    buf.ice_mode = icy_engine::IceMode::Ice;
    let converted: Vec<u8> = FileFormat::Ansi.to_bytes(&mut buf, &SaveOptions::new()).unwrap();
    // more gentle output.
    let b: Vec<u8> = converted.iter().map(|&x| if x == 27 { b'x' } else { x }).collect();
    let converted: std::borrow::Cow<'_, str> = String::from_utf8_lossy(b.as_slice());

    let b: Vec<u8> = data.iter().map(|&x| if x == 27 { b'x' } else { x }).collect();
    let expected = String::from_utf8_lossy(b.as_slice());

    assert_eq!(expected, converted);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_ice() {
    let data = b"\x1B[?33h\x1B[5m   test\x1B[?33l";
    test_ansi_ice(data);
}

#[test]
#[ignore = "ANSI output format changed"]
fn test_palette_color_bug() {
    let mut buf = TextBuffer::new((3, 1));
    buf.palette.set_color(25, Color::new(0xD3, 0xD3, 0xD3));
    buf.layers[0].set_char(
        (1, 0),
        AttributedChar {
            ch: 'A',
            attribute: TextAttribute::new(25, 0),
        },
    );

    let bytes = FileFormat::Ansi.to_bytes(&buf, &SaveOptions::default()).unwrap();
    let str = String::from_utf8_lossy(&bytes).to_string();

    assert_eq!(" \u{1b}[1;211;211;211tA ", str);
}
/*
#[cfg(test)]
fn crop2_loaded_file(result: &mut dyn Screen) {
for l in 0..result.layers.len() {
    if let Some(line) = result.layers[l].lines.last_mut() {
        while !line.chars.is_empty() && !line.chars.last().unwrap().is_visible() {
            line.chars.pop();
        }
    }

    if !result.layers[l].lines.is_empty()
        && result.layers[l].lines.last().unwrap().chars.is_empty()
    {
        result.layers[l].lines.pop();
        crop2_loaded_file(result);
    }
}
}*/

#[cfg(test)]
#[derive(Clone, Copy)]
pub struct CompareOptions {
    pub compare_palette: bool,
    pub compare_fonts: bool,
    pub ignore_invisible_chars: bool,
}

#[cfg(test)]
impl CompareOptions {
    pub const ALL: CompareOptions = CompareOptions {
        compare_palette: true,
        compare_fonts: true,
        ignore_invisible_chars: false,
    };
}

#[cfg(test)]
pub(crate) fn compare_buffers(buf_old: &TextBuffer, buf_new: &TextBuffer, compare_options: CompareOptions) {
    assert_eq!(buf_old.layers.len(), buf_new.layers.len());
    assert_eq!(buf_old.size(), buf_new.size(), "size differs: {} != {}", buf_old.size(), buf_new.size());

    //crop2_loaded_file(buf_old);
    //crop2_loaded_file(buf_new);
    /*assert_eq!(
        buf_old.ice_mode, buf_new.ice_mode,
        "ice_mode differs: {:?} != {:?}",
        buf_old.ice_mode, buf_new.ice_mode,
    );*/

    if compare_options.compare_palette {
        assert_eq!(buf_old.palette.len(), buf_new.palette.len(), "palette color count differs");
        for i in 0..buf_old.palette.len() {
            assert_eq!(
                buf_old.palette.color(i as u32),
                buf_new.palette.color(i as u32),
                "palette color {} differs: {} <> {}",
                i,
                buf_old.palette.color(i as u32),
                buf_new.palette.color(i as u32),
            );
        }
    }

    if compare_options.compare_fonts {
        assert_eq!(buf_old.font_count(), buf_new.font_count());

        for (i, old_fnt) in buf_old.font_iter() {
            let new_fnt = buf_new.font(*i).unwrap();

            // Compare the glyphs directly (256 glyphs per font)
            assert_eq!(old_fnt.glyphs.len(), new_fnt.glyphs.len(), "glyph count differs for font {i}");
            /*
            for (old_glyph, new_glyph) in old_fnt.glyphs.iter().zip(new_fnt.glyphs.iter()) {
                assert_eq!(old_glyph, new_glyph, "glyphs differ font: {i}");
            }*/
        }
    }
    for layer in 0..buf_old.layers.len() {
        /*      assert_eq!(
            buf_old.layers[layer].lines.len(),
            buf_new.layers[layer].lines.len(),
            "layer {layer} line count differs"
        );*/
        assert_eq!(buf_old.layers[layer].offset(), buf_new.layers[layer].offset(), "layer {layer} offset differs");
        assert_eq!(buf_old.layers[layer].size(), buf_new.layers[layer].size(), "layer {layer} size differs");
        assert_eq!(
            buf_old.layers[layer].properties.is_visible, buf_new.layers[layer].properties.is_visible,
            "layer {layer} is_visible differs"
        );
        assert_eq!(
            buf_old.layers[layer].properties.has_alpha_channel, buf_new.layers[layer].properties.has_alpha_channel,
            "layer {layer} has_alpha_channel differs"
        );

        for line in 0..buf_old.layers[layer].lines.len() {
            for i in 0..buf_old.layers[layer].width() as usize {
                // char_at expects (x, y), so i is x (column) and line is y (row)
                let mut ch = buf_old.layers[layer].char_at((i, line).into());
                let mut ch2 = buf_new.layers[layer].char_at((i, line).into());
                if compare_options.ignore_invisible_chars && (!ch.is_visible() || !ch2.is_visible()) {
                    continue;
                }

                assert_eq!(
                    buf_old.palette.color(ch.attribute.foreground()),
                    buf_new.palette.color(ch2.attribute.foreground()),
                    "fg differs at layer: {layer}, line: {line}, char: {i} (old:{}={}, new:{}={})",
                    ch.attribute.foreground(),
                    buf_old.palette.color(ch.attribute.foreground()),
                    ch2.attribute.foreground(),
                    buf_new.palette.color(ch2.attribute.foreground())
                );
                assert_eq!(
                    buf_old.palette.color(ch.attribute.background()),
                    buf_new.palette.color(ch2.attribute.background()),
                    "bg differs at layer: {layer}, line: {line}, char: {i} (old:{}={}, new:{}={})",
                    ch.attribute.background(),
                    buf_old.palette.color(ch.attribute.background()),
                    ch2.attribute.background(),
                    buf_new.palette.color(ch2.attribute.background())
                );

                ch.attribute.set_foreground(0);
                ch.attribute.set_background(0);

                ch2.attribute.set_foreground(0);
                ch2.attribute.set_background(0);
                assert_eq!(ch, ch2, "layer: {layer}, line: {line}, char: {i}");
            }
        }
    }
}
