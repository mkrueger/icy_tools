use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_parser_core::{CommandParser, CommandSink, SkypixCommand, SkypixParser, TerminalCommand};
use std::fs;
use std::hint::black_box;
use std::path::Path;

struct NullSink;
impl CommandSink for NullSink {
    #[inline]
    fn print(&mut self, _text: &[u8]) { /* discard */
    }

    #[inline]
    fn emit(&mut self, _cmd: TerminalCommand) { /* discard */
    }

    #[inline]
    fn emit_skypix(&mut self, _cmd: SkypixCommand) { /* discard */
    }
}

fn load_skypix_files() -> Vec<u8> {
    let skypix_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("icy_engine/tests/output/skypix/files");
    let mut combined = Vec::new();

    // Load all .ans files from the directory
    if let Ok(entries) = fs::read_dir(&skypix_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ans") {
                if let Ok(data) = fs::read(&path) {
                    combined.extend_from_slice(&data);
                } else {
                    eprintln!("Warning: Could not read {:?}", path);
                }
            }
        }
    } else {
        eprintln!("Warning: Could not read skypix directory");
    }

    if combined.is_empty() {
        eprintln!("Warning: No SkyPix files found, using synthetic data");
        // Generate some synthetic SkyPix data for benchmarking
        let mut data = Vec::new();

        // Add some common SkyPix commands
        data.extend_from_slice(b"\x1B[1;100;100!"); // SetPixel
        data.extend_from_slice(b"\x1B[2;200;200!"); // DrawLine
        data.extend_from_slice(b"\x1B[4;10;10;100;100!"); // RectangleFill
        data.extend_from_slice(b"\x1B[5;50;50;30;20!"); // Ellipse
        data.extend_from_slice(b"\x1B[11;0;0;170;170;170;85;85;85;255;85;85;85;255;85;255;255;85;85;85;255;255;85;255;85;255;255;255;255;255!"); // NewPalette
        data.extend_from_slice(b"\x1B[17;2!"); // SetDisplayMode
        data.extend_from_slice(b"\x1B[19;100;50!"); // PositionCursor
        data.extend_from_slice(b"Hello, SkyPix!"); // Text
        data.extend_from_slice(b"\x1B[15;14!"); // SetPenA
        data.extend_from_slice(b"\x1B[8;150;150!"); // MovePen

        // Repeat to get more data
        combined = data.repeat(100);
    }

    combined
}

fn bench_skypix_parser(c: &mut Criterion) {
    let data = load_skypix_files();

    let mut group = c.benchmark_group("skypix_parser");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("parse_skypix", |b| {
        b.iter(|| {
            let mut parser = SkypixParser::new();
            let mut sink = NullSink;
            parser.parse(black_box(&data), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_skypix_parser);
criterion_main!(benches);
