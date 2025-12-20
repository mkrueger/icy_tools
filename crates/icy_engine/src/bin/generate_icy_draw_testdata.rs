//! Helper binary to generate a large IcyDraw test file for benchmarking.
//!
//! Run with:
//!   cargo run -p icy_engine --bin generate_icy_draw_testdata

use icy_engine::{AnsiSaveOptionsV2, FileFormat, TextPane};
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches").join("data").join("icy_draw");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let xbin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("data")
        .join("xb_uncompressed")
        .join("dZ-taos1.xb");

    let out_path = out_dir.join("large_dZ-taos1_x2_layers5.icy");
    generate_from_xbin(&xbin_path, &out_path);

    println!("Generated test file in {:?}", out_dir);
}

fn generate_from_xbin(xbin_path: &Path, out_path: &Path) {
    let data = fs::read(xbin_path).expect("read xbin input");
    let mut buf = FileFormat::XBin.from_bytes(&data, None).expect("load xbin").screen.buffer;

    let width = buf.width();
    let height = buf.height();
    let new_height = height * 2;

    buf.set_size((width, new_height));
    buf.set_height(new_height);

    // Duplicate content: append the full buffer once below (2× vertical concat).
    for layer in &mut buf.layers {
        layer.set_size((width, new_height));
        layer.set_height(new_height);
        for y in 0..height {
            for x in 0..width {
                let ch = TextPane::char_at(layer, (x, y).into());
                layer.set_char((x, y + height), ch);
            }
        }
    }

    // Ensure we have ~5 layers (clone layer 0 if needed).
    while buf.layers.len() < 5 {
        let mut new_layer = buf.layers[0].clone();
        let idx = buf.layers.len() as i32;
        new_layer.set_title(format!("Layer {}", idx + 1));
        new_layer.set_offset((idx * 2, idx * 2));
        buf.layers.push(new_layer);
    }

    let bytes = FileFormat::IcyDraw.to_bytes(&mut buf, &AnsiSaveOptionsV2::default()).expect("save icydraw");

    fs::write(out_path, &bytes).expect("write icydraw output");
    println!(
        "Generated {} ({}x{}, layers={}, {} bytes)",
        out_path.display(),
        width,
        new_height,
        buf.layers.len(),
        bytes.len()
    );
}
