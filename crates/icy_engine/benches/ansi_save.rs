//! Benchmarks for ANSI save performance (legacy vs v2)

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_engine::formats::ansi_v2::{AnsiCompatibilityLevel, AnsiSaveOptionsV2, save_ansi_v2};
use icy_engine::{FileFormat, LoadData, SaveOptions, TextPane};

use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Duration;

use walkdir::WalkDir;

fn legacy_optimized_options() -> SaveOptions {
    let mut opt = SaveOptions::default();
    opt.compress = true;
    opt.use_cursor_forward = true;
    opt.use_repeat_sequences = true;
    opt.use_extended_colors = true;
    opt.preserve_line_length = false;
    opt.output_line_length = None;
    opt
}

fn v2_optimized_options() -> AnsiSaveOptionsV2 {
    let mut opt = AnsiSaveOptionsV2::default();
    opt.level = AnsiCompatibilityLevel::IcyTerm;
    opt.compress = true;
    opt.preserve_line_length = false;
    opt.output_line_length = None;
    opt
}

fn load_samples(max_samples: usize) -> Vec<(String, icy_engine::TextBuffer)> {
    // Some historical fixtures can trigger panics in parsers due to edge-case
    // sequences. For benchmarking we want robustness, so we skip those inputs and
    // suppress panic-hook noise during loading.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../..");

    // Optional: if the local sixteencolors archive exists, use it to reach the
    // requested sample count quickly. This keeps the benchmark representative
    // for real-world ANSI corpora.
    let external_archive_root = PathBuf::from("/home/mkrueger/work/sixteencolors-archive/all");

    // Candidate roots are ordered; we sort within each root for determinism.
    let ansi_roots: &[PathBuf] = &[
        // workspace samples
        workspace_root.clone(),
        // engine test fixtures
        manifest_dir.join("tests/output/ansi/files"),
        manifest_dir.join("tests/output/skypix/files"),
        // parser-core bench fixtures (lots of stable ANSI data)
        workspace_root.join("crates/icy_parser_core/benches/ansi_data"),
        workspace_root.join("crates/icy_parser_core/benches/skypix_data"),
        // a few more repo assets
        workspace_root.join("crates/icy_engine_scripting/data"),
        // external corpus (if available)
        external_archive_root,
    ];

    let xbin_roots: &[PathBuf] = &[
        manifest_dir.join("benches/data/xb_compressed"),
        manifest_dir.join("benches/data/xb_uncompressed"),
    ];

    let mut out: Vec<(String, icy_engine::TextBuffer)> = Vec::new();

    // --- Load XBin first (stable, and requested) ---
    for root in xbin_roots {
        if !root.exists() {
            continue;
        }

        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.into_path();
            if !path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("xb") || ext.eq_ignore_ascii_case("xbin"))
            {
                continue;
            }
            paths.push(path);
            if paths.len() >= max_samples.saturating_mul(5) {
                break;
            }
        }
        paths.sort();

        for path in paths {
            if out.len() >= max_samples {
                break;
            }

            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let parsed = std::panic::catch_unwind(|| FileFormat::XBin.from_bytes(&bytes, None));
            let screen = match parsed {
                Ok(Ok(s)) => s,
                _ => continue,
            };

            out.push((format!("xb:{}", path.display()), screen.buffer));
        }
    }

    // --- Then load ANSI files ---
    for root in ansi_roots {
        if !root.exists() {
            continue;
        }

        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.into_path();
            if !path.extension().and_then(|s| s.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("ans")) {
                continue;
            }
            // Skip our own debug dumps if present in workspace root.
            if path.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.starts_with("_debug_")) {
                continue;
            }
            paths.push(path);
            // Avoid enumerating huge trees (e.g. external archives) fully.
            if paths.len() >= max_samples.saturating_mul(20) {
                break;
            }
        }
        paths.sort();

        for path in paths {
            if out.len() >= max_samples {
                break;
            }

            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let sauce_opt = icy_sauce::SauceRecord::from_bytes(&bytes).ok().flatten();
            let load_data = Some(LoadData::new(sauce_opt, None, Some(80)));
            let parsed = std::panic::catch_unwind(|| FileFormat::Ansi.from_bytes(&bytes, load_data));
            let screen = match parsed {
                Ok(Ok(s)) => s,
                _ => continue,
            };

            out.push((format!("ans:{}", path.display()), screen.buffer));
            if out.len() > 15 {
                break;
            }
        }
    }

    std::panic::set_hook(prev_hook);
    out
}

fn bench_ansi_save_legacy_vs_v2(c: &mut Criterion) {
    // Try to get at least 200 inputs; we overshoot a bit and let max_samples cap it.
    let buffers = load_samples(250);
    if buffers.is_empty() {
        eprintln!("Warning: no ANSI samples found for benchmark");
        return;
    }

    let xb_count = buffers.iter().filter(|(n, _)| n.starts_with("xb:")).count();
    let ans_count = buffers.len().saturating_sub(xb_count);

    // One-time size comparison for the selected inputs.
    let legacy_opt = legacy_optimized_options();
    let v2_opt: AnsiSaveOptionsV2 = v2_optimized_options();

    let mut legacy_total = 0usize;
    let mut v2_total = 0usize;
    for (_, buf) in &buffers {
        legacy_total += FileFormat::Ansi.to_bytes(buf, &legacy_opt).unwrap().len();
        v2_total += save_ansi_v2(buf, &v2_opt).unwrap().len();
    }
    eprintln!("ansi_save samples: {} (xb={} ans={})", buffers.len(), xb_count, ans_count);
    if buffers.len() < 200 {
        eprintln!(
            "Warning: only {} samples loaded (<200). Consider placing the sixteencolors archive at /home/mkrueger/work/sixteencolors-archive/all",
            buffers.len()
        );
    }
    eprintln!(
        "ansi_save total bytes: legacy={} v2={} delta={:+} ({:+.2}%)",
        legacy_total,
        v2_total,
        (v2_total as i64 - legacy_total as i64),
        ((v2_total as f64 - legacy_total as f64) * 100.0) / (legacy_total as f64)
    );

    let total_cells: u64 = buffers.iter().map(|(_, b)| (b.width() as u64) * (b.height() as u64)).sum();

    let mut group = c.benchmark_group("ansi_save");
    // With ~200 samples this can take a bit longer; reduce sample size to keep runs manageable.
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(15));
    group.throughput(Throughput::Elements(total_cells));

    group.bench_function("legacy_optimized_all", |b| {
        b.iter(|| {
            for (_, buf) in &buffers {
                black_box(FileFormat::Ansi.to_bytes(buf, &legacy_opt).unwrap());
            }
        })
    });

    group.bench_function("v2_icyterm_all", |b| {
        b.iter(|| {
            for (_, buf) in &buffers {
                black_box(save_ansi_v2(buf, &v2_opt).unwrap());
            }
        })
    });

    group.finish();
}

criterion_group!(benches, bench_ansi_save_legacy_vs_v2);
criterion_main!(benches);
