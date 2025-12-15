//! Benchmarks for IcyDraw file format loading and saving
//!
//! Tests:
//! - Loading performance
//! - Saving performance (with thumbnail)
//! - Saving performance (skip_thumbnail)

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_engine::{FileFormat, SaveOptions, TextPane};
use std::fs;
use std::hint::black_box;
use std::path::Path;

// ============================================================================
// Helper functions
// ============================================================================

fn load_large_icy_bytes() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("data")
        .join("icy_draw")
        .join("large_dZ-taos1_x2_layers5.icy");

    fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "Missing benchmark input: {}. Run: cargo run -p icy_engine --bin generate_icy_draw_testdata",
            path.display()
        )
    })
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_icy_draw_loading(c: &mut Criterion) {
    let data = load_large_icy_bytes();
    let mut group = c.benchmark_group("icy_draw_large (dZ-taos1.xb ×2, layers=5)");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_function("load", |b| {
        b.iter(|| {
            let result = FileFormat::IcyDraw.from_bytes(black_box(&data), black_box(None));
            black_box(result)
        })
    });
    group.finish();
}

fn bench_icy_draw_saving(c: &mut Criterion) {
    let data = load_large_icy_bytes();
    let loaded = FileFormat::IcyDraw.from_bytes(&data, None).expect("load icydraw");
    let buf = loaded.buffer;

    let mut group = c.benchmark_group("icy_draw_large (dZ-taos1.xb ×2, layers=5)");
    let size = (buf.width() * buf.height()) as u64;
    group.throughput(Throughput::Elements(size));
    group.bench_function("save (with thumbnail)", |b| {
        b.iter(|| {
            let mut buf_clone = buf.clone();
            let opts = SaveOptions::default();
            let result = FileFormat::IcyDraw.to_bytes(black_box(&mut buf_clone), black_box(&opts));
            black_box(result)
        })
    });
    group.finish();
}

fn bench_icy_draw_saving_skip_thumbnail(c: &mut Criterion) {
    let data = load_large_icy_bytes();
    let loaded = FileFormat::IcyDraw.from_bytes(&data, None).expect("load icydraw");
    let buf = loaded.buffer;

    let mut group = c.benchmark_group("icy_draw_large (dZ-taos1.xb ×2, layers=5)");
    let size = (buf.width() * buf.height()) as u64;
    group.throughput(Throughput::Elements(size));
    group.bench_function("save (fast_save/skip_thumbnail)", |b| {
        b.iter(|| {
            let mut buf_clone = buf.clone();
            let mut opts = SaveOptions::default();
            opts.skip_thumbnail = true;
            let result = FileFormat::IcyDraw.to_bytes(black_box(&mut buf_clone), black_box(&opts));
            black_box(result)
        })
    });
    group.finish();
}

// ============================================================================
// Criterion setup
// ============================================================================

criterion_group!(benches, bench_icy_draw_loading, bench_icy_draw_saving, bench_icy_draw_saving_skip_thumbnail,);

criterion_main!(benches);
