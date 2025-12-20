use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
    sync::Once,
};

use icy_engine::{Color, Rectangle, RenderOptions, Screen, TextBuffer, TextPane};

static INIT: Once = Once::new();

/// Initialize logging for tests
pub fn init_logging() {
    INIT.call_once(|| {
        let _ = env_logger::builder().is_test(true).filter_level(log::LevelFilter::Info).try_init();
    });
}

mod output;

mod format;

mod buffer;
/*
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
*/

pub fn compare_output(screen: &dyn Screen, src_file: &Path) {
    let rect: Rectangle = screen.size().into();
    let (rendered_size, rendered_data) = screen.render_to_rgba(&rect.into());
    let dims = screen.font_dimensions();

    compare_rendered_output(&rendered_size, dims.width as usize, dims.height as usize, &rendered_data, src_file);
}

pub fn compare_buffer_output(buffer: &TextBuffer, src_file: &Path) {
    let rect: Rectangle = Rectangle::from(0, 0, buffer.width(), buffer.height());
    let opts = RenderOptions::from(rect);
    let (rendered_size, rendered_data) = buffer.render_to_rgba(&opts, false);
    let dims = buffer.font_dimensions();

    compare_rendered_output(&rendered_size, dims.width as usize, dims.height as usize, &rendered_data, src_file);
}

/// Compare buffer output with custom settings for testing aspect_ratio and use_letter_spacing flags
/// This modifies the buffer's settings temporarily for rendering
pub fn compare_buffer_output_with_options(buffer: &mut TextBuffer, src_file: &Path, use_letter_spacing: bool, use_aspect_ratio: bool) {
    // Set the buffer's rendering flags
    buffer.set_use_letter_spacing(use_letter_spacing);
    buffer.set_use_aspect_ratio(use_aspect_ratio);

    let rect: Rectangle = Rectangle::from(0, 0, buffer.width(), buffer.height());
    let opts = RenderOptions::from(rect);
    let (mut rendered_size, mut rendered_data) = buffer.render_to_rgba_raw(&opts, false);

    // Aspect ratio correction is now applied at display/shader level.
    // For legacy output comparisons, apply the same vertical stretch here.
    if use_aspect_ratio {
        let stretch_factor = buffer.get_aspect_ratio_stretch_factor();
        let (scaled_h, scaled_pixels) = scale_image_vertical(rendered_data, rendered_size.width, rendered_size.height, stretch_factor);
        rendered_size = icy_engine::Size::new(rendered_size.width, scaled_h);
        rendered_data = scaled_pixels;
    }
    let dims = buffer.font_dimensions();
    compare_rendered_output(&rendered_size, dims.width as usize, dims.height as usize, &rendered_data, src_file);
}

fn scale_image_vertical(pixels: Vec<u8>, width: i32, height: i32, scale: f32) -> (i32, Vec<u8>) {
    let new_height = (height as f32 * scale).round() as i32;
    if new_height <= 0 || width <= 0 || height <= 0 || scale <= 0.0 {
        return (height, pixels);
    }

    let stride = width as usize * 4;
    let mut scaled = vec![0u8; stride * new_height as usize];

    for new_y in 0..new_height {
        let src_y = new_y as f32 / scale;
        let src_y0 = (src_y.floor() as i32).clamp(0, height - 1) as usize;
        let src_y1 = (src_y0 + 1).min(height as usize - 1);
        let t = src_y.fract();

        let dst_row = new_y as usize * stride;
        let src_row0 = src_y0 * stride;
        let src_row1 = src_y1 * stride;

        for x in 0..width as usize {
            let px = x * 4;

            let a0 = pixels[src_row0 + px + 3] as f32;
            let a1 = pixels[src_row1 + px + 3] as f32;
            let a = a0 + (a1 - a0) * t;
            let out_a = if a >= 128.0 { 255u8 } else { 0u8 };

            if out_a == 0 {
                scaled[dst_row + px] = 0;
                scaled[dst_row + px + 1] = 0;
                scaled[dst_row + px + 2] = 0;
                scaled[dst_row + px + 3] = 0;
                continue;
            }

            let mut w0 = 1.0 - t;
            let mut w1 = t;
            if pixels[src_row0 + px + 3] == 0 {
                w0 = 0.0;
            }
            if pixels[src_row1 + px + 3] == 0 {
                w1 = 0.0;
            }

            let w_sum = w0 + w1;
            if w_sum <= f32::EPSILON {
                scaled[dst_row + px] = 0;
                scaled[dst_row + px + 1] = 0;
                scaled[dst_row + px + 2] = 0;
                scaled[dst_row + px + 3] = 0;
                continue;
            }
            w0 /= w_sum;
            w1 /= w_sum;

            for c in 0..3 {
                let v0 = pixels[src_row0 + px + c] as f32;
                let v1 = pixels[src_row1 + px + c] as f32;
                scaled[dst_row + px + c] = (v0 * w0 + v1 * w1).round() as u8;
            }
            scaled[dst_row + px + 3] = 255;
        }
    }

    (new_height, scaled)
}

fn compare_rendered_output(rendered_size: &icy_engine::Size, font_w: usize, font_h: usize, rendered_data: &[u8], src_file: &Path) {
    let filename = src_file.file_name().unwrap().to_string_lossy().to_string();
    let png_file = src_file.with_extension("png");
    let output_path = src_file.with_extension("output.png");

    // Load expected PNG
    let file = File::open(&png_file).unwrap_or_else(|e| panic!("Error opening PNG file {:?}: {}", png_file, e));
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().unwrap();

    // Get expected dimensions and color type
    let (width, height, color_type) = {
        let info = reader.info();
        (info.width as usize, info.height as usize, info.color_type)
    };

    // Get absolute paths for easier opening
    let absolute_output_path = output_path.canonicalize().unwrap_or_else(|_| output_path.to_path_buf());
    let absolute_png_path = png_file.canonicalize().unwrap_or_else(|_| png_file.to_path_buf());

    // Check resolution
    if width != rendered_size.width as usize || height != rendered_size.height as usize {
        // Save the rendered output as PNG for comparison
        let file = File::create(&output_path).unwrap();
        let w = BufWriter::new(file);

        let mut encoder = png::Encoder::new(w, rendered_size.width as u32, rendered_size.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&rendered_data).unwrap();

        panic!(
            "Test failed for: {}\nResolution mismatch!\nExpected: {}x{}\nGot: {}x{}\nOutput saved to: file://{}\nShould look like: file://{}",
            filename,
            width,
            height,
            rendered_size.width,
            rendered_size.height,
            absolute_output_path.display(),
            absolute_png_path.display()
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
    } else if color_type == png::ColorType::Rgba {
        img_buf
    } else {
        panic!("Unsupported PNG color type: {:?} in {}", color_type, absolute_png_path.display());
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

        let (x, y, expected, got) = mismatch.unwrap();
        panic!(
            "Test failed for: {}\nMismatch {}x{} pixel at x: {}, y: {}.\nExpected: {:?}\nGot: {:?}\nOutput saved to: file://{}\nShould look like: file://{}\n",
            filename,
            x / font_w,
            y / font_h,
            x,
            y,
            expected,
            got,
            absolute_output_path.display(),
            absolute_png_path.display()
        );
    }
}
