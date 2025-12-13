//! Benchmarks for XBin file encoding
//!
//! Tests encoding performance for compressed and uncompressed XBin output.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_engine::{FileFormat, SaveOptions, TextPane};
use std::fs;
use std::hint::black_box;
use std::path::Path;

// ============================================================================
// Helper functions
// ============================================================================

fn load_all_xbin_files() -> Vec<icy_engine::TextBuffer> {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("data")
        .join("xb_uncompressed");

    let mut buffers = Vec::new();
    if let Ok(entries) = fs::read_dir(&base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("xb") || ext.eq_ignore_ascii_case("xbin"))
            {
                if let Ok(data) = fs::read(&path) {
                    if let Ok(screen) = FileFormat::XBin.from_bytes(&data, None) {
                        buffers.push(screen.buffer);
                    }
                }
            }
        }
    }
    buffers
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_xbin_encode_uncompressed(c: &mut Criterion) {
    let buffers = load_all_xbin_files();

    if buffers.is_empty() {
        eprintln!("Warning: No XBin files found in benches/data/xb_uncompressed/");
        return;
    }

    let mut group = c.benchmark_group("xbin_encode_uncompressed");

    // Calculate total size for throughput
    let total_cells: usize = buffers.iter().map(|b| (b.width() * b.height()) as usize).sum();
    group.throughput(Throughput::Elements(total_cells as u64));

    let mut options = SaveOptions::new();
    options.compress = false;

    group.bench_function("all_files", |b| {
        b.iter(|| {
            for buffer in &buffers {
                black_box(FileFormat::XBin.to_bytes(buffer, &options).unwrap());
            }
        });
    });

    group.finish();
}

fn bench_xbin_encode_compressed(c: &mut Criterion) {
    let buffers = load_all_xbin_files();

    if buffers.is_empty() {
        eprintln!("Warning: No XBin files found in benches/data/xb_uncompressed/");
        return;
    }

    let mut group = c.benchmark_group("xbin_encode_compressed");

    // Calculate total size for throughput
    let total_cells: usize = buffers.iter().map(|b| (b.width() * b.height()) as usize).sum();
    group.throughput(Throughput::Elements(total_cells as u64));

    let mut options = SaveOptions::new();
    options.compress = true;

    group.bench_function("all_files", |b| {
        b.iter(|| {
            for buffer in &buffers {
                black_box(FileFormat::XBin.to_bytes(buffer, &options).unwrap());
            }
        });
    });

    group.finish();
}

criterion_group!(benches, bench_xbin_encode_uncompressed, bench_xbin_encode_compressed);

criterion_main!(benches);