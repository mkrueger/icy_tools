//! Benchmarks for flip table generation
//!
//! Tests the performance of generating character flip tables for horizontal and vertical flipping.
//! These tables are used to map characters to their mirrored equivalents when flipping ANSI art.

use criterion::{Criterion, criterion_group, criterion_main};
use icy_engine_edit::{BitFont, generate_flipx_table, generate_flipy_table};
use std::hint::black_box;

fn load_default_font() -> BitFont {
    BitFont::default()
}

fn bench_generate_flipx_table(c: &mut Criterion) {
    let font = load_default_font();

    c.bench_function("generate_flipx_table", |b| {
        b.iter(|| {
            let result = generate_flipx_table(black_box(&font));
            black_box(result)
        })
    });
}

fn bench_generate_flipy_table(c: &mut Criterion) {
    let font = load_default_font();

    c.bench_function("generate_flipy_table", |b| {
        b.iter(|| {
            let result = generate_flipy_table(black_box(&font));
            black_box(result)
        })
    });
}

fn bench_both_flip_tables(c: &mut Criterion) {
    let font = load_default_font();

    let mut group = c.benchmark_group("flip_tables");

    group.bench_function("flipx", |b| {
        b.iter(|| {
            let result = generate_flipx_table(black_box(&font));
            black_box(result)
        })
    });

    group.bench_function("flipy", |b| {
        b.iter(|| {
            let result = generate_flipy_table(black_box(&font));
            black_box(result)
        })
    });

    group.bench_function("both_sequential", |b| {
        b.iter(|| {
            let x = generate_flipx_table(black_box(&font));
            let y = generate_flipy_table(black_box(&font));
            black_box((x, y))
        })
    });

    group.finish();
}

criterion_group!(benches, bench_generate_flipx_table, bench_generate_flipy_table, bench_both_flip_tables);
criterion_main!(benches);
