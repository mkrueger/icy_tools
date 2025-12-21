use std::{env, fs, path::PathBuf};

use codepages::tables::CP437_TO_UNICODE;

use icy_engine::formats::{FileFormat, ansi_v2::AnsiCompatibilityLevel, ansi_v2::SaveOptions, ansi_v2::save_ansi_v2};

fn cp437_ansi_bytes_to_utf8_with_bom(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 3);
    // UTF-8 BOM
    out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);

    for &b in bytes {
        if b < 0x80 {
            out.push(b);
        } else {
            let ch = CP437_TO_UNICODE.get(b as usize).copied().unwrap_or(char::from(b));
            let mut buf = [0u8; 4];
            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        }
    }

    out
}

fn main() -> icy_engine::Result<()> {
    // Defaults: use workspace root samples.
    let mut input = PathBuf::from("color_test.ans");
    let mut output = PathBuf::from("utf8_terminal_color_test.ans");

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--in" => {
                if let Some(p) = args.next() {
                    input = PathBuf::from(p);
                }
            }
            "--out" => {
                if let Some(p) = args.next() {
                    output = PathBuf::from(p);
                }
            }
            _ => {
                eprintln!("Unknown arg: {arg}");
                eprintln!("Usage: export_utf8_terminal_sample [--in PATH] [--out PATH]");
                return Ok(());
            }
        }
    }

    let screen = FileFormat::Ansi.load(&input, None)?;

    let mut opt = SaveOptions::default();
    opt.level = Some(AnsiCompatibilityLevel::Utf8Terminal);
    opt.compress = true;
    opt.preserve_line_length = false;
    opt.output_line_length = None;
    opt.longer_terminal_output = false;

    let bytes_cp437 = save_ansi_v2(&screen.screen.buffer, &opt)?;
    let bytes_utf8 = cp437_ansi_bytes_to_utf8_with_bom(&bytes_cp437);

    fs::write(&output, &bytes_utf8)?;
    eprintln!("Wrote {} bytes to {}", bytes_utf8.len(), output.display());

    Ok(())
}
