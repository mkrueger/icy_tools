use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use icy_parser_core::{CommandParser, CommandSink, RipParser, TerminalCommand};
use std::fs;
use std::path::Path;

struct NullSink;
impl CommandSink for NullSink {
    #[inline]
    fn print(&mut self, _text: &[u8]) { /* discard */
    }

    #[inline]
    fn emit(&mut self, _cmd: TerminalCommand) { /* discard */
    }
}

fn load_rip_files() -> Vec<u8> {
    let rip_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/rip_data");
    let mut combined = Vec::new();

    let files = [
        "bg-svhnd.rip",
        "dragon01.rip",
        "fel-pro1.rip",
        "garfield.rip",
        "jdraw.rip",
        "lthouse.rip",
        "msg5.rip",
        "ns-scrlz.rip",
        "paleo.rip",
        "pk!knght.rip",
        "shadow.rip",
        "sm-prod3.rip",
        "to-rip.rip",
        "win1.rip",
    ];

    for filename in &files {
        let path = rip_dir.join(filename);
        if let Ok(data) = fs::read(&path) {
            combined.extend_from_slice(&data);
        } else {
            eprintln!("Warning: Could not read {}", filename);
        }
    }

    if combined.is_empty() {
        panic!("No RIP files loaded from benches/rip_data/");
    }

    combined
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Text heavy with minimal RIP commands (realistic BBS output)
    let mut text_heavy = Vec::new();
    for i in 0..500 {
        text_heavy.extend_from_slice(format!("Message {}: Welcome to the BBS!\r\n", i).as_bytes());
    }
    // Add a simple RIP command
    text_heavy.extend_from_slice(b"!|c0F|L000A0A14");

    // 2. RIP command heavy (lots of drawing commands)
    let mut command_heavy = Vec::new();
    command_heavy.extend_from_slice(b"!|*"); // Reset windows
    for i in 0..100 {
        // Draw lines
        command_heavy.extend_from_slice(format!("!|L{:02X}{:02X}{:02X}{:02X}", i, i, i + 10, i + 10).as_bytes());
        // Draw rectangles
        command_heavy.extend_from_slice(format!("!|R{:02X}{:02X}{:02X}{:02X}", i, i, i + 20, i + 20).as_bytes());
        // Set colors
        command_heavy.extend_from_slice(format!("!|c{:02X}", i % 16).as_bytes());
        // Draw circles
        command_heavy.extend_from_slice(format!("!|C{:02X}{:02X}{:02X}", i + 50, i + 50, 10).as_bytes());
    }

    // 3. Mixed content (typical RIP screen with text and graphics)
    let mut mixed = Vec::new();
    mixed.extend_from_slice(b"!|*"); // Reset
    mixed.extend_from_slice(b"!|w00002743011"); // Set text window
    mixed.extend_from_slice(b"Welcome to the graphics BBS!\r\n");
    for i in 0..50 {
        mixed.extend_from_slice(format!("!|L{:02X}{:02X}{:02X}{:02X}", i * 2, i * 2, i * 2 + 5, i * 2 + 5).as_bytes());
        mixed.extend_from_slice(b"Loading menu...\r\n");
        mixed.extend_from_slice(format!("!|c{:02X}", i % 16).as_bytes());
    }

    // 4. Complex scene with buttons and mouse regions
    let mut complex = Vec::new();
    complex.extend_from_slice(b"!|*");
    complex.extend_from_slice(b"!|v00000OJSA"); // Viewport
    // Draw background
    complex.extend_from_slice(b"!|c0F!|B000000OJSA");
    // Add buttons
    for i in 0..10 {
        let y = i * 30;
        complex.extend_from_slice(format!("!|1U{:02X}{:02X}{:02X}{:02X}0010Button {}", 10, y, 80, y + 20, i).as_bytes());
        // Mouse region
        complex.extend_from_slice(format!("!|1M00{:02X}{:02X}{:02X}{:02X}1100000CLICK{}", 10, y, 80, y + 20, i).as_bytes());
    }
    // Text overlay
    complex.extend_from_slice(b"Menu System\r\n");

    (text_heavy, command_heavy, mixed, complex)
}

fn bench_rip_parser_real_world(c: &mut Criterion) {
    let data = load_rip_files();
    let mut group = c.benchmark_group("rip_parser_real_world");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("parse_real_rip_files", |b| {
        b.iter(|| {
            let mut parser = RipParser::new();
            let mut sink = NullSink;
            parser.parse(black_box(&data), &mut sink);
        });
    });

    group.finish();
}

fn bench_rip_parser_text_heavy(c: &mut Criterion) {
    let (text_heavy, _, _, _) = make_synthetic_inputs();
    let mut group = c.benchmark_group("rip_parser_synthetic");
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));

    group.bench_function("text_heavy", |b| {
        b.iter(|| {
            let mut parser = RipParser::new();
            let mut sink = NullSink;
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    group.finish();
}

fn bench_rip_parser_command_heavy(c: &mut Criterion) {
    let (_, command_heavy, _, _) = make_synthetic_inputs();
    let mut group = c.benchmark_group("rip_parser_synthetic");
    group.throughput(Throughput::Bytes(command_heavy.len() as u64));

    group.bench_function("command_heavy", |b| {
        b.iter(|| {
            let mut parser = RipParser::new();
            let mut sink = NullSink;
            parser.parse(black_box(&command_heavy), &mut sink);
        });
    });

    group.finish();
}

fn bench_rip_parser_mixed(c: &mut Criterion) {
    let (_, _, mixed, _) = make_synthetic_inputs();
    let mut group = c.benchmark_group("rip_parser_synthetic");
    group.throughput(Throughput::Bytes(mixed.len() as u64));

    group.bench_function("mixed_content", |b| {
        b.iter(|| {
            let mut parser = RipParser::new();
            let mut sink = NullSink;
            parser.parse(black_box(&mixed), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_rip_parser_real_world,
    bench_rip_parser_text_heavy,
    bench_rip_parser_command_heavy,
    bench_rip_parser_mixed,
);

criterion_main!(benches);
