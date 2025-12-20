use icy_engine::TextPane;
use icy_engine::formats::{AnsiSaveOptionsV2, FileFormat};
use std::fs;

fn main() {
    // Load the original about.icy (V0 format with zTXt chunks)
    let original_data = fs::read("crates/icy_draw/data/about.icy").expect("Failed to read file");
    println!("Original file size: {} bytes", original_data.len());

    // Load it using the IcyDraw format parser
    let loaded = FileFormat::IcyDraw.from_bytes(&original_data, None).expect("Failed to load original .icy file");
    println!(
        "Loaded buffer: {}x{}, {} layers",
        loaded.screen.buffer.width(),
        loaded.screen.buffer.height(),
        loaded.screen.buffer.layers.len()
    );

    // Save it back using the new format (V1 with icYD chunks)
    let options = AnsiSaveOptionsV2::default();
    let saved_data = FileFormat::IcyDraw.to_bytes(&loaded.screen.buffer, &options).expect("Failed to save .icy file");
    println!("Saved file size: {} bytes", saved_data.len());

    // Save to temp file for inspection
    fs::write("/tmp/about_roundtrip.icy", &saved_data).expect("Failed to write temp file");
    println!("Saved to /tmp/about_roundtrip.icy");

    // Now try to load the saved file
    match FileFormat::IcyDraw.from_bytes(&saved_data, None) {
        Ok(reloaded) => {
            println!(
                "Reloaded buffer: {}x{}, {} layers",
                reloaded.screen.buffer.width(),
                reloaded.screen.buffer.height(),
                reloaded.screen.buffer.layers.len()
            );
            println!("SUCCESS: Roundtrip works!");
        }
        Err(e) => {
            println!("FAILED to reload: {}", e);
            // Analyze the saved file
            analyze_png_chunks(&saved_data);
        }
    }
}

fn analyze_png_chunks(data: &[u8]) {
    use std::io::Cursor;
    use zstd::stream::decode_all as zstd_decode_all;

    println!("\n=== PNG Chunk Analysis ===");
    let mut off = 8usize; // skip PNG signature
    while off + 8 <= data.len() {
        let length = u32::from_be_bytes(data[off..off + 4].try_into().unwrap()) as usize;
        let chunk_type = String::from_utf8_lossy(&data[off + 4..off + 8]);
        println!("Chunk: {} (length: {})", chunk_type, length);

        if chunk_type == "icYD" {
            let chunk_data = &data[off + 8..off + 8 + length];
            let _version = chunk_data[0];
            let keyword_len = u16::from_le_bytes(chunk_data[1..3].try_into().unwrap()) as usize;
            let keyword = String::from_utf8_lossy(&chunk_data[3..3 + keyword_len]);
            let data_len = u32::from_le_bytes(chunk_data[3 + keyword_len..7 + keyword_len].try_into().unwrap()) as usize;
            let payload = &chunk_data[7 + keyword_len..7 + keyword_len + data_len];
            println!("  keyword={}, payload_len={}", keyword, data_len);

            if keyword == "ICED" {
                let iced_version = u16::from_le_bytes(payload[0..2].try_into().unwrap());
                let compression = payload[2];
                let width = u32::from_le_bytes(payload[11..15].try_into().unwrap());
                let height = u32::from_le_bytes(payload[15..19].try_into().unwrap());
                println!("  version={}, compression={}, size={}x{}", iced_version, compression, width, height);
            } else if keyword.starts_with("LAYER_") {
                // Try to decompress
                match zstd_decode_all(Cursor::new(payload)) {
                    Ok(decompressed) => {
                        println!("  compressed={}, decompressed={}", payload.len(), decompressed.len());
                        analyze_layer_data(&decompressed);
                    }
                    Err(e) => println!("  Decompress error: {}", e),
                }
            }
        }

        if &data[off + 4..off + 8] == b"IEND" {
            break;
        }
        off += 8 + length + 4;
    }
}

fn analyze_layer_data(data: &[u8]) {
    let mut o = 0usize;

    // Name (with length prefix - u32!)
    if data.len() < o + 4 {
        println!("    ERROR: too short for name_len at offset {}", o);
        return;
    }
    let name_len = u32::from_le_bytes(data[o..o + 4].try_into().unwrap()) as usize;
    o += 4;
    println!("    name_len: {}", name_len);
    if data.len() < o + name_len {
        println!("    ERROR: too short for name (need {} bytes)", name_len);
        return;
    }
    let name = String::from_utf8_lossy(&data[o..o + name_len]);
    o += name_len;
    println!("    name: \"{}\"", name);

    // Role (1 byte)
    if data.len() < o + 1 {
        println!("    ERROR: too short for role");
        return;
    }
    let role = data[o];
    o += 1;
    println!("    role: {} (0=Normal, 1=Image)", role);

    // Unused (4 bytes)
    if data.len() < o + 4 {
        println!("    ERROR: too short for unused");
        return;
    }
    o += 4;

    // Mode (1 byte)
    if data.len() < o + 1 {
        println!("    ERROR: too short for mode");
        return;
    }
    let mode = data[o];
    o += 1;
    println!("    mode: {} (0=Normal, 1=Chars, 2=Attrs)", mode);

    // Color (4 bytes rgba)
    if data.len() < o + 4 {
        println!("    ERROR: too short for color");
        return;
    }
    let r = data[o];
    let g = data[o + 1];
    let b = data[o + 2];
    let a = data[o + 3];
    o += 4;
    println!("    color: rgba({},{},{},{})", r, g, b, a);

    // Flags (4 bytes)
    if data.len() < o + 4 {
        println!("    ERROR: too short for flags");
        return;
    }
    let flags = u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    o += 4;
    println!("    flags: {:#x}", flags);

    // Transparency (1 byte)
    if data.len() < o + 1 {
        println!("    ERROR: too short for transparency");
        return;
    }
    let transparency = data[o];
    o += 1;
    println!("    transparency: {}", transparency);

    // Offset (x, y) - 8 bytes
    if data.len() < o + 8 {
        println!("    ERROR: too short for offset");
        return;
    }
    let offset_x = i32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    o += 4;
    let offset_y = i32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    o += 4;
    println!("    offset: ({}, {})", offset_x, offset_y);

    // Size (width, height) - 8 bytes
    if data.len() < o + 8 {
        println!("    ERROR: too short for size");
        return;
    }
    let width = i32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    o += 4;
    let height = i32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    o += 4;
    println!("    size: {}x{}", width, height);

    // Default font page (2 bytes) - deprecated, kept for format compatibility
    if data.len() < o + 2 {
        println!("    ERROR: too short for default_font_page (deprecated)");
        return;
    }
    let _default_font_page = u16::from_le_bytes(data[o..o + 2].try_into().unwrap());
    o += 2;
    println!("    default_font_page (deprecated): {}", _default_font_page);

    // Data length (8 bytes)
    if data.len() < o + 8 {
        println!("    ERROR: too short for data_length");
        return;
    }
    let data_length = u64::from_le_bytes(data[o..o + 8].try_into().unwrap()) as usize;
    o += 8;
    let remaining = data.len() - o;
    println!("    data_length: {}, remaining: {}", data_length, remaining);

    if data_length > remaining {
        println!("    !!! BUG: data_length {} > remaining {} !!!", data_length, remaining);
    }

    println!("    header consumed: {} bytes, total layer data: {} bytes", o, data.len());
}
