//! `REXPaint` `.xp` images: gzip-compressed, column-major layers of CP437 glyphs with 24-bit
//! foreground and background colors. Background 255,0,255 marks a transparent cell.
//!
//! Layout as documented by `REXPaint` and implemented in kaleidotron (MIT, Copyright (c) 2026 Rick Christy).

use std::io::Read;

use super::super::{apply_sauce_to_buffer, LoadData};
use crate::{AttributedChar, Layer, LoadingError, Position, Result, TextAttribute, TextPane, TextScreen};

const MAX_LAYERS: usize = 16;
const MAX_SIZE: i32 = 4096;
const MAX_UNPACKED: u64 = 256 * 1024 * 1024;
const CELL_SIZE: usize = 10;
const TRANSPARENT: [u8; 3] = [255, 0, 255];

pub(crate) fn load_rexpaint(data: &[u8], load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(data)
        .take(MAX_UNPACKED)
        .read_to_end(&mut raw)
        .map_err(|error| LoadingError::Error(format!("REXPaint: invalid gzip stream: {error}")))?;

    let i32_at = |offset: usize| raw.get(offset..offset + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let invalid = |message: &str| LoadingError::Error(format!("REXPaint: {message}"));
    let _version = i32_at(0).ok_or(LoadingError::FileTooShort)?;
    let layer_count = i32_at(4).ok_or(LoadingError::FileTooShort)?;
    if !(1..=MAX_LAYERS as i32).contains(&layer_count) {
        return Err(invalid("implausible layer count").into());
    }

    let mut offset = 8;
    let mut layers = Vec::new();
    for _ in 0..layer_count {
        let (Some(width), Some(height)) = (i32_at(offset), i32_at(offset + 4)) else {
            return Err(LoadingError::FileTooShort.into());
        };
        if !(1..=MAX_SIZE).contains(&width) || !(1..=MAX_SIZE).contains(&height) {
            return Err(invalid("implausible layer size").into());
        }
        offset += 8;
        let cells = raw
            .get(offset..offset + width as usize * height as usize * CELL_SIZE)
            .ok_or(LoadingError::FileTooShort)?;
        offset += cells.len();
        layers.push((width, height, cells));
    }

    let width = layers.iter().map(|l| l.0).max().unwrap_or(1);
    let height = layers.iter().map(|l| l.1).max().unwrap_or(1);
    let mut screen = TextScreen::new((width, height));
    screen.buffer.terminal_state.is_terminal_buffer = false;
    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer(&mut screen.buffer, sauce);
    }
    screen.buffer.layers.clear();
    for (index, (layer_width, layer_height, cells)) in layers.into_iter().enumerate() {
        let mut layer = Layer::new(
            if index == 0 {
                "Background".to_string()
            } else {
                format!("Layer {}", index + 1)
            },
            (width, height),
        );
        for (cell_index, cell) in cells.chunks_exact(CELL_SIZE).enumerate() {
            let x = cell_index as i32 / layer_height;
            let y = cell_index as i32 % layer_height;
            if x >= layer_width {
                break;
            }
            let background = [cell[7], cell[8], cell[9]];
            if background == TRANSPARENT && index > 0 {
                continue;
            }
            let glyph = u32::from_le_bytes([cell[0], cell[1], cell[2], cell[3]]).min(255) as u8;
            let mut attribute = TextAttribute::default();
            attribute.set_foreground_rgb(cell[4], cell[5], cell[6]);
            if background == TRANSPARENT {
                attribute.set_background_rgb(0, 0, 0);
            } else {
                attribute.set_background_rgb(background[0], background[1], background[2]);
            }
            layer.set_char(Position::new(x, y), AttributedChar::new(glyph as char, attribute));
        }
        screen.buffer.layers.push(layer);
    }

    if let Some(max_height) = load_data_opt.and_then(LoadData::max_height) {
        if screen.buffer.height() > max_height {
            screen.buffer.set_height(max_height);
        }
    }
    Ok(screen)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::AttributeColor;

    fn cell(glyph: u8, fg: [u8; 3], bg: [u8; 3]) -> Vec<u8> {
        let mut out = u32::from(glyph).to_le_bytes().to_vec();
        out.extend_from_slice(&fg);
        out.extend_from_slice(&bg);
        out
    }

    fn xp(layers: &[Vec<Vec<u8>>], width: i32, height: i32) -> Vec<u8> {
        let mut raw = (-1i32).to_le_bytes().to_vec();
        raw.extend_from_slice(&(layers.len() as i32).to_le_bytes());
        for cells in layers {
            raw.extend_from_slice(&width.to_le_bytes());
            raw.extend_from_slice(&height.to_le_bytes());
            for c in cells {
                raw.extend_from_slice(c);
            }
        }
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&raw).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn loads_column_major_layers_with_transparency() {
        let magenta = TRANSPARENT;
        // 2×1 layers: cells are listed column by column.
        let background = vec![cell(b'A', [255, 255, 255], [0, 0, 128]), cell(0xDB, [10, 20, 30], magenta)];
        let overlay = vec![cell(b'x', [1, 2, 3], magenta), cell(b'O', [200, 0, 0], [0, 50, 0])];
        let screen = load_rexpaint(&xp(&[background, overlay], 2, 1), None, None).unwrap();
        assert_eq!((screen.buffer.width(), screen.buffer.height()), (2, 1));
        assert_eq!(screen.buffer.layers.len(), 2);

        let a = screen.buffer.layers[0].char_at(Position::new(0, 0));
        assert_eq!(a.ch, 'A');
        assert_eq!(a.attribute.foreground_color(), AttributeColor::Rgb(255, 255, 255));
        assert_eq!(a.attribute.background_color(), AttributeColor::Rgb(0, 0, 128));
        let block = screen.buffer.layers[0].char_at(Position::new(1, 0));
        assert_eq!(block.ch, '\u{DB}');
        assert_eq!(block.attribute.background_color(), AttributeColor::Rgb(0, 0, 0));

        assert!(screen.buffer.layers[1].char_at(Position::new(0, 0)).is_transparent());
        assert_eq!(screen.buffer.layers[1].char_at(Position::new(1, 0)).ch, 'O');
    }

    #[test]
    fn rejects_bad_input() {
        assert!(load_rexpaint(b"not gzip", None, None).is_err());
        assert!(load_rexpaint(&xp(&[], 1, 1), None, None).is_err());
        let truncated = xp(&[vec![cell(b'A', [0; 3], [0; 3])]], 2, 2);
        assert!(load_rexpaint(&truncated, None, None).is_err());
    }
}
