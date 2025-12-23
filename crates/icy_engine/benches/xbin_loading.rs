//! Benchmarks for XBin file loading
//!
//! Tests loading performance for compressed and uncompressed XBin files.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use icy_engine::FileFormat;
use std::fs;
use std::hint::black_box;
use std::path::Path;

// ============================================================================
// Helper functions
// ============================================================================

fn load_test_files(dir: &str) -> Vec<(String, Vec<u8>)> {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches").join("data").join(dir);

    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("xb") || ext.eq_ignore_ascii_case("xbin"))
            {
                if let Ok(data) = fs::read(&path) {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                    files.push((name, data));
                }
            }
        }
    }
    files
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_xbin_compressed(c: &mut Criterion) {
    let files = load_test_files("xb_compressed");

    if files.is_empty() {
        eprintln!("Warning: No compressed XBin files found in benches/data/xb_compressed/");
        return;
    }

    let mut group = c.benchmark_group("xbin_compressed");

    for (name, data) in &files {
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("load", name), data, |b, data| {
            b.iter(|| {
                let result = FileFormat::XBin.from_bytes(black_box(data), black_box(None));
                black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_xbin_uncompressed(c: &mut Criterion) {
    let files = load_test_files("xb_uncompressed");

    if files.is_empty() {
        eprintln!("Warning: No uncompressed XBin files found in benches/data/xb_uncompressed/");
        return;
    }

    let mut group = c.benchmark_group("xbin_uncompressed");

    for (name, data) in &files {
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("load", name), data, |b, data| {
            b.iter(|| {
                let result = FileFormat::XBin.from_bytes(black_box(data), black_box(None));
                black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_xbin_all_files(c: &mut Criterion) {
    let compressed = load_test_files("xb_compressed");
    let uncompressed = load_test_files("xb_uncompressed");

    let total_compressed_size: usize = compressed.iter().map(|(_, d)| d.len()).sum();
    let total_uncompressed_size: usize = uncompressed.iter().map(|(_, d)| d.len()).sum();

    let mut group = c.benchmark_group("xbin_batch");

    if !compressed.is_empty() {
        group.throughput(Throughput::Bytes(total_compressed_size as u64));
        group.bench_function("all_compressed", |b| {
            b.iter(|| {
                for (_, data) in &compressed {
                    let result = FileFormat::XBin.from_bytes(black_box(data), black_box(None));
                    let _ = black_box(result);
                }
            });
        });
    }

    if !uncompressed.is_empty() {
        group.throughput(Throughput::Bytes(total_uncompressed_size as u64));
        group.bench_function("all_uncompressed", |b| {
            b.iter(|| {
                for (_, data) in &uncompressed {
                    let result = FileFormat::XBin.from_bytes(black_box(data), black_box(None));
                    let _ = black_box(result);
                }
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_xbin_compressed, bench_xbin_uncompressed, bench_xbin_all_files,);
criterion_main!(benches);
