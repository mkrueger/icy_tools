// XBin roundtrip tests: Load .icy -> Save as XBin -> Load XBin -> Compare with original
// Run with: cargo test --package icy_engine --test mod -- format::xbin::roundtrip --nocapture

use super::super::ansi2::{compare_buffers, CompareOptions};
use icy_engine::{FileFormat, SaveOptions, TextPane};
use std::path::Path;

/// Test roundtrip for AK-TORCH.icy
#[test]
fn roundtrip_ak_torch() {
    test_icy_xbin_roundtrip("AK-TORCH.icy", 25104);
}

/// Test roundtrip for dZ-taos1.icy  
#[test]
fn roundtrip_dz_taos1() {
    test_icy_xbin_roundtrip("dZ-taos1.icy", 155746);
}

/// Test roundtrip for om-nouchka2.icy
#[test]
fn roundtrip_om_nouchka2() {
    test_icy_xbin_roundtrip("om-nouchka2.icy", 14199);
}

/// Test roundtrip for r-tribut.icy
#[test]
fn roundtrip_r_tribut() {
    test_icy_xbin_roundtrip("r-tribut.icy", 88474);
}

fn test_icy_xbin_roundtrip(filename: &str, expected_compressed_size: usize) {
    let test_dir: &Path = Path::new("tests/format/xbin/test_data");
    let icy_path = test_dir.join(filename);

    println!("Testing roundtrip for: {}", icy_path.display());

    // Step 1: Load the .icy file
    let icy_format = FileFormat::IcyDraw;
    let original = icy_format
        .load(&icy_path, None)
        .unwrap_or_else(|e| panic!("Failed to load {}: {:?}", filename, e));

    println!(
        "  Loaded .icy: {}x{}, {} fonts, ice_mode={:?}",
        original.buffer.width(),
        original.buffer.height(),
        original.buffer.font_count(),
        original.buffer.ice_mode
    );

    // Step 2: Save as XBin (uncompressed)
    let xbin_format = FileFormat::XBin;
    let mut save_options = SaveOptions::default();
    save_options.compress = false;
    save_options.lossles_output = true;

    let xbin_bytes = xbin_format
        .to_bytes(&original.buffer, &save_options)
        .unwrap_or_else(|e| panic!("Failed to save {} as XBin: {:?}", filename, e));

    println!("  Saved as XBin (uncompressed): {} bytes", xbin_bytes.len());

    // Step 3: Load the XBin back
    let reloaded = xbin_format
        .from_bytes(&xbin_bytes, None)
        .unwrap_or_else(|e| panic!("Failed to reload XBin for {}: {:?}", filename, e));

    println!(
        "  Reloaded XBin: {}x{}, {} fonts",
        reloaded.buffer.width(),
        reloaded.buffer.height(),
        reloaded.buffer.font_count()
    );

    // Step 4: Compare original with reloaded
    compare_buffers(&original.buffer, &reloaded.buffer, CompareOptions::ALL);
    println!("  ✓ Uncompressed roundtrip passed");

    // Step 5: Also test with compression
    save_options.compress = true;
    let xbin_compressed = xbin_format
        .to_bytes(&original.buffer, &save_options)
        .unwrap_or_else(|e| panic!("Failed to save {} as compressed XBin: {:?}", filename, e));

    println!("  Saved as XBin (compressed): {} bytes", xbin_compressed.len());

    // Verify compression size hasn't regressed
    assert_eq!(
        xbin_compressed.len(),
        expected_compressed_size,
        "Compressed size mismatch for {}! Expected {} bytes, got {} bytes. Compression may have regressed.",
        filename,
        expected_compressed_size,
        xbin_compressed.len()
    );

    let reloaded_compressed = xbin_format
        .from_bytes(&xbin_compressed, None)
        .unwrap_or_else(|e| panic!("Failed to reload compressed XBin for {}: {:?}", filename, e));

    compare_buffers(&original.buffer, &reloaded_compressed.buffer, CompareOptions::ALL);
    println!("  ✓ Compressed roundtrip passed");
}
