//! Convert YAFF font files to PSF2 format
//!
//! Usage: cargo run -p icy_engine --bin convert_yaff_to_psf [input.yaff] [output.psf]
//! Or run without arguments to convert all YAFF files in data/fonts/Rip/

use libyaff::YaffFont;
use std::fs;
use std::path::Path;

fn convert_yaff_to_psf(input_path: &Path, output_path: &Path) {
    println!("Converting: {} -> {}", input_path.display(), output_path.display());

    let data = fs::read(input_path).unwrap();
    let yaff = YaffFont::from_bytes(&data).unwrap();

    // Get dimensions
    let width = yaff.bounding_box.map(|(w, _)| w as u8).or(yaff.cell_size.map(|(w, _)| w as u8)).unwrap_or(8);

    let height = yaff
        .pixel_size
        .or(yaff.size)
        .or(yaff.line_height)
        .or(yaff.bounding_box.map(|(_, h)| h as i32))
        .or(yaff.cell_size.map(|(_, h)| h as i32))
        .unwrap_or(16) as u8;

    println!("  Dimensions: {}x{}", width, height);

    // Create PSF2 output
    let mut output = Vec::new();

    // PSF2 header
    output.extend_from_slice(&[0x72, 0xb5, 0x4a, 0x86]); // magic
    output.extend_from_slice(&0u32.to_le_bytes()); // version
    output.extend_from_slice(&32u32.to_le_bytes()); // header size
    output.extend_from_slice(&0u32.to_le_bytes()); // flags (no unicode table)
    output.extend_from_slice(&256u32.to_le_bytes()); // num glyphs
    let bytes_per_glyph = height as u32;
    output.extend_from_slice(&bytes_per_glyph.to_le_bytes());
    output.extend_from_slice(&(height as u32).to_le_bytes()); // height
    output.extend_from_slice(&(width as u32).to_le_bytes()); // width

    // Build glyph data
    let mut glyphs = vec![vec![0u8; height as usize]; 256];

    for glyph_def in &yaff.glyphs {
        for label in &glyph_def.labels {
            if let libyaff::Label::Codepoint(codes) = label {
                for code in codes {
                    let code_val = *code as usize;
                    if code_val < 256 {
                        for (y, row) in glyph_def.bitmap.pixels.iter().enumerate() {
                            if y >= height as usize {
                                break;
                            }
                            let mut packed: u8 = 0;
                            for (x, pixel) in row.iter().enumerate() {
                                if x >= 8 {
                                    break;
                                }
                                if *pixel {
                                    packed |= 1 << (7 - x);
                                }
                            }
                            glyphs[code_val][y] = packed;
                        }
                    }
                }
            }
        }
    }

    // Write glyph data
    for glyph in &glyphs {
        output.extend_from_slice(glyph);
    }

    // Write output file
    fs::write(output_path, &output).unwrap();
    println!("  Wrote: {} ({} bytes)", output_path.display(), output.len());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 3 {
        // Convert single file
        let input = Path::new(&args[1]);
        let output = Path::new(&args[2]);
        convert_yaff_to_psf(input, output);
    } else {
        // Convert all YAFF files in Rip directory
        let rip_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/fonts/Rip");

        println!("Looking in: {}", rip_dir.display());

        for entry in fs::read_dir(&rip_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "yaff") {
                let out_path = path.with_extension("psf");
                convert_yaff_to_psf(&path, &out_path);
            }
        }
    }

    println!("\nDone!");
}
