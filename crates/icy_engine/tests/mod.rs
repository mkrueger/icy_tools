use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use icy_engine::{
    Color, RenderOptions, Screen, Size, TextBuffer,
    editor::{EditState, UndoState},
};

mod output;

#[test]
fn test_set_aspect_ratio() {
    let mut buffer = TextBuffer::new((80, 25));
    buffer.set_use_aspect_ratio(false);
    let mut edit_state = EditState::from_buffer(buffer);

    edit_state.set_use_aspect_ratio(true).unwrap();
    assert!(edit_state.get_buffer().use_aspect_ratio());
    edit_state.set_use_aspect_ratio(false).unwrap();
    assert!(!edit_state.get_buffer().use_aspect_ratio());
    edit_state.undo().unwrap();
    assert!(edit_state.get_buffer().use_aspect_ratio());
}

#[test]
fn test_set_letter_spacing() {
    let mut buffer = TextBuffer::new((80, 25));
    buffer.set_use_letter_spacing(false);
    let mut edit_state = EditState::from_buffer(buffer);

    edit_state.set_use_letter_spacing(true).unwrap();
    assert!(edit_state.get_buffer().use_letter_spacing());
    edit_state.set_use_letter_spacing(false).unwrap();
    assert!(!edit_state.get_buffer().use_letter_spacing());
    edit_state.undo().unwrap();
    assert!(edit_state.get_buffer().use_letter_spacing());
}

pub fn compare_output(screen: &dyn Screen, cur_entry: &Path) {
    let (rendered_size, rendered_data) = screen.render_to_rgba(&RenderOptions::default());

    let filename = cur_entry.file_name().unwrap().to_string_lossy().to_string();
    let png_file = cur_entry.with_extension("png");
    let output_path = cur_entry.with_extension("output.png");

    // Load expected PNG
    let file = File::open(&png_file).unwrap_or_else(|e| panic!("Error opening PNG file {:?}: {}", png_file, e));
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().unwrap();

    // Get expected dimensions and color type
    let (width, height, color_type) = {
        let info = reader.info();
        (info.width as usize, info.height as usize, info.color_type)
    };

    // Check resolution
    if width != rendered_size.width as usize || height != rendered_size.height as usize {
        panic!(
            "Test failed for: {}\nResolution mismatch!\nExpected: {}x{}\nGot: {}x{}",
            filename, width, height, rendered_size.width, rendered_size.height
        );
    }

    // Allocate buffer based on the actual color type
    let output_buffer_size = reader.output_buffer_size().unwrap();
    let mut img_buf = vec![0; output_buffer_size];
    reader.next_frame(&mut img_buf).unwrap();

    // Convert to RGBA if needed
    let img_buf = if color_type == png::ColorType::Rgb {
        let mut rgba_buf = Vec::with_capacity(width * height * 4);
        for chunk in img_buf.chunks_exact(3) {
            rgba_buf.push(chunk[0]); // R
            rgba_buf.push(chunk[1]); // G
            rgba_buf.push(chunk[2]); // B
            rgba_buf.push(255); // A
        }
        rgba_buf
    } else {
        img_buf
    };

    // Compare
    let mut mismatch: Option<(usize, usize, Color, Color)> = None;
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            if img_buf[idx] != rendered_data[idx] || img_buf[idx + 1] != rendered_data[idx + 1] || img_buf[idx + 2] != rendered_data[idx + 2] {
                let expected = Color::new(img_buf[idx], img_buf[idx + 1], img_buf[idx + 2]);
                let got = Color::new(rendered_data[idx], rendered_data[idx + 1], rendered_data[idx + 2]);
                mismatch = Some((x, y, expected, got));
                break;
            }
        }
        if mismatch.is_some() {
            break;
        }
    }

    if mismatch.is_some() {
        // Save the rendered output as PNG for comparison
        let file = File::create(&output_path).unwrap();
        let w = BufWriter::new(file);

        let mut encoder = png::Encoder::new(w, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&rendered_data).unwrap();

        // Get absolute paths for easier opening
        let absolute_output_path = output_path.canonicalize().unwrap_or_else(|_| output_path.to_path_buf());
        let absolute_png_path = png_file.canonicalize().unwrap_or_else(|_| png_file.to_path_buf());

        let (x, y, expected, got) = mismatch.unwrap();
        panic!(
            "Test failed for: {}\nMismatch pixel at x: {}, y: {}.\nExpected: {:?}\nGot: {:?}\nOutput saved to: file://{}\nShould look like: file://{}",
            filename,
            x,
            y,
            expected,
            got,
            absolute_output_path.display(),
            absolute_png_path.display()
        );
    }
    println!("Test passed for: {}", filename);
}

fn compare_images(rendered_data: &[u8], img_buf: &[u8], width: usize, height: usize) -> Option<(usize, usize, Color, Color)> {
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let col1 = Color::new(rendered_data[idx], rendered_data[idx + 1], rendered_data[idx + 2]);
            let col2 = Color::new(img_buf[idx], img_buf[idx + 1], img_buf[idx + 2]);

            if col1 != col2 {
                return Some((x, y, col2, col1));
            }
        }
    }
    None
}
