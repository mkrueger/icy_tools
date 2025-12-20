use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use icy_engine::{
    Rectangle, RenderOptions, Screen, Size, TextPane,
    formats::{
        FileFormat, LoadData,
        ansi_v2::{AnsiCompatibilityLevel, AnsiSaveOptionsV2, save_ansi_v2},
    },
};

const DEFAULT_ROOT: &str = "/home/mkrueger/work/sixteencolors-archive/all";

const MISMATCH_DIR_NAME: &str = "mismatches";

fn collect_ans_files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(MISMATCH_DIR_NAME))
        {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(err) => {
                eprintln!("WARN: can't read dir {}: {err}", dir.display());
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("WARN: bad dir entry in {}: {err}", dir.display());
                    continue;
                }
            };

            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };

            if ft.is_dir() {
                stack.push(path);
                continue;
            }

            if !ft.is_file() {
                continue;
            }

            if path.extension().and_then(|s| s.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("ans")) {
                out.push(path);
            }
        }
    }

    Ok(())
}

fn render_buffer_rgba(buffer: &icy_engine::TextBuffer, height: i32) -> (Size, Vec<u8>) {
    let height = height.clamp(0, buffer.height());
    let rect = Rectangle::from(0, 0, buffer.width(), height);
    let options = RenderOptions {
        rect: rect.into(),
        blink_on: true,
        ..Default::default()
    };

    let scan_lines = options.override_scan_lines.unwrap_or(false);
    buffer.render_to_rgba(&options, scan_lines)
}

fn load_ansi_with_sauce(bytes: &[u8]) -> icy_engine::Result<icy_engine::formats::LoadedDocument> {
    FileFormat::Ansi.from_bytes(bytes, Some(LoadData::new(None, Some(80))))
}

fn terminalize_for_compare(buffer: &icy_engine::TextBuffer) -> icy_engine::TextBuffer {
    let mut result = buffer.clone();

    // ANSI has no real transparency; for comparisons we treat transparent cells as solid terminal defaults.
    for layer in &mut result.layers {
        // Ensure alpha doesn't affect composition.
        layer.properties.has_alpha_channel = false;

        let height = layer.height();
        let width = layer.width();
        for y in 0..height {
            for x in 0..width {
                let mut ch = layer.char_at(icy_engine::Position::new(x, y));

                if ch.ch == '\0' {
                    ch.ch = ' ';
                }

                // Clear internal invisible markers.
                ch.attribute.attr &= !icy_engine::attribute::INVISIBLE;

                if ch.attribute.is_foreground_transparent() {
                    ch.attribute.set_foreground(7);
                }
                if ch.attribute.is_background_transparent() {
                    ch.attribute.set_background(0);
                }

                layer.set_char(icy_engine::Position::new(x, y), ch);
            }
        }
    }

    result
}

fn first_cell_diff(a: &icy_engine::TextBuffer, b: &icy_engine::TextBuffer) -> Option<String> {
    if a.width() != b.width() {
        return Some(format!("width differs: a={} b={}", a.width(), b.width()));
    }

    // Ignore trailing blank bottom lines: only compare up to max(line_count).
    let cmp_h = a.line_count().max(b.line_count());
    if a.height() < cmp_h || b.height() < cmp_h {
        return Some(format!(
            "height/line_count differs: a={}x{} (lines={}) b={}x{} (lines={})",
            a.width(),
            a.height(),
            a.line_count(),
            b.width(),
            b.height(),
            b.line_count()
        ));
    }

    // Compare merged cells (TextPane::char_at) to match rendering semantics.
    for y in 0..cmp_h {
        for x in 0..a.width() {
            let ca = a.char_at(icy_engine::Position::new(x, y));
            let cb = b.char_at(icy_engine::Position::new(x, y));
            if ca.ch != cb.ch || ca.attribute != cb.attribute || ca.font_page() != cb.font_page() {
                return Some(format!(
                    "first diff at ({x},{y}): a=({:?}, {:?}, font={}) b=({:?}, {:?}, font={})",
                    ca.ch,
                    ca.attribute,
                    ca.font_page(),
                    cb.ch,
                    cb.attribute,
                    cb.font_page()
                ));
            }
        }
    }
    None
}

fn first_char_diff(a: &icy_engine::TextBuffer, b: &icy_engine::TextBuffer) -> Option<String> {
    if a.width() != b.width() {
        return Some(format!("width differs: a={} b={}", a.width(), b.width()));
    }

    let cmp_h = a.line_count().max(b.line_count());
    if a.height() < cmp_h || b.height() < cmp_h {
        return Some(format!(
            "height/line_count differs: a={}x{} (lines={}) b={}x{} (lines={})",
            a.width(),
            a.height(),
            a.line_count(),
            b.width(),
            b.height(),
            b.line_count()
        ));
    }

    for y in 0..cmp_h {
        for x in 0..a.width() {
            let ca = a.char_at(icy_engine::Position::new(x, y));
            let cb = b.char_at(icy_engine::Position::new(x, y));
            if ca.ch != cb.ch || ca.font_page() != cb.font_page() {
                return Some(format!(
                    "first CHAR diff at ({x},{y}): a=({:?}, font={}) b=({:?}, font={})",
                    ca.ch,
                    ca.font_page(),
                    cb.ch,
                    cb.font_page()
                ));
            }
        }
    }

    None
}

fn find_char_in_window(buf: &icy_engine::TextBuffer, target: char, x0: i32, y0: i32, x1: i32, y1: i32) -> Option<(i32, i32)> {
    let x0 = x0.max(0);
    let y0 = y0.max(0);
    let x1 = x1.min(buf.width() - 1);
    let y1 = y1.min(buf.line_count().min(buf.height()) - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let ch = buf.char_at(icy_engine::Position::new(x, y)).ch;
            if ch == target {
                return Some((x, y));
            }
        }
    }
    None
}

fn first_rgba_diff(a_size: Size, a: &[u8], b_size: Size, b: &[u8], cell_size: Size) -> Option<String> {
    if a_size != b_size {
        return Some(format!(
            "rgba size differs: a={}x{} b={}x{}",
            a_size.width, a_size.height, b_size.width, b_size.height
        ));
    }
    let len = a.len().min(b.len());
    for i in 0..len {
        if a[i] != b[i] {
            let px = i / 4;
            let w = a_size.width.max(1) as usize;
            let x_px = (px % w) as i32;
            let y_px = (px / w) as i32;
            let x_cell = if cell_size.width > 0 { x_px / cell_size.width } else { -1 };
            let y_cell = if cell_size.height > 0 { y_px / cell_size.height } else { -1 };
            return Some(format!(
                "first rgba byte diff at i={i} (px={x_px},{y_px} cell={x_cell},{y_cell}): a={} b={} (channel={})",
                a[i],
                b[i],
                i % 4
            ));
        }
    }
    None
}

fn first_rgba_diff_cell(a_size: Size, a: &[u8], b_size: Size, b: &[u8], cell_size: Size) -> Option<(i32, i32)> {
    if a_size != b_size {
        return None;
    }
    if cell_size.width <= 0 || cell_size.height <= 0 {
        return None;
    }
    let len = a.len().min(b.len());
    for i in 0..len {
        if a[i] != b[i] {
            let px = i / 4;
            let w = a_size.width.max(1) as usize;
            let x_px = (px % w) as i32;
            let y_px = (px / w) as i32;
            return Some((x_px / cell_size.width, y_px / cell_size.height));
        }
    }
    None
}

fn tag_stats(buf: &icy_engine::TextBuffer) -> (usize, Option<i32>) {
    if buf.tags.is_empty() {
        return (0, None);
    }
    let max_y = buf.tags.iter().map(|t| t.position.y).max();
    (buf.tags.len(), max_y)
}

fn sauce_stats(bytes: &[u8]) -> Option<(u16, u16)> {
    let sauce = icy_sauce::SauceRecord::from_bytes(bytes).ok().flatten()?;
    match sauce.capabilities() {
        Some(icy_sauce::Capabilities::Character(c)) => Some((c.columns, c.lines)),
        Some(icy_sauce::Capabilities::Binary(c)) => Some((c.columns, c.lines)),
        _ => None,
    }
}

fn newline_stats(bytes: &[u8]) -> (usize, usize, usize) {
    let mut cr = 0usize;
    let mut lf = 0usize;
    let mut crlf = 0usize;

    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                cr += 1;
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    crlf += 1;
                    i += 2;
                    continue;
                }
            }
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }

    (cr, lf, crlf)
}

#[derive(Default, Debug, Clone, Copy)]
struct EscapeStats {
    esc: usize,
    csi: usize,
    ind: usize,     // ESC D
    nel: usize,     // ESC E
    ri: usize,      // ESC M
    csi_cud: usize, // CSI ... B
    csi_cup: usize, // CSI ... H / f
    csi_vpa: usize, // CSI ... d
}

fn escape_stats(bytes: &[u8]) -> EscapeStats {
    let mut stats = EscapeStats::default();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != 0x1B {
            i += 1;
            continue;
        }

        stats.esc += 1;
        if i + 1 >= bytes.len() {
            break;
        }

        match bytes[i + 1] {
            b'D' => {
                stats.ind += 1;
                i += 2;
                continue;
            }
            b'E' => {
                stats.nel += 1;
                i += 2;
                continue;
            }
            b'M' => {
                stats.ri += 1;
                i += 2;
                continue;
            }
            b'[' => {
                stats.csi += 1;
                // Parse CSI until final byte in 0x40..=0x7E
                let mut j = i + 2;
                while j < bytes.len() {
                    let b = bytes[j];
                    if (0x40..=0x7E).contains(&b) {
                        match b {
                            b'B' => stats.csi_cud += 1,
                            b'H' | b'f' => stats.csi_cup += 1,
                            b'd' => stats.csi_vpa += 1,
                            _ => {}
                        }
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
            _ => {}
        }

        i += 2;
    }

    stats
}

fn main() -> icy_engine::Result<()> {
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut single_file: Option<PathBuf> = None;
    let mut limit: Option<usize> = None;
    let mut fail_fast = false;
    let mut only_level: Option<AnsiCompatibilityLevel> = None;
    let mut quarantine_mismatches = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                if let Some(p) = args.next() {
                    root = PathBuf::from(p);
                }
            }
            "--file" => {
                if let Some(p) = args.next() {
                    single_file = Some(PathBuf::from(p));
                }
            }
            "--limit" => {
                if let Some(n) = args.next() {
                    limit = n.parse::<usize>().ok();
                }
            }
            "--fail-fast" => fail_fast = true,
            "--quarantine-mismatches" => quarantine_mismatches = true,
            "--only-level" => {
                if let Some(lvl) = args.next() {
                    let lvl = lvl.to_ascii_lowercase();
                    only_level = match lvl.as_str() {
                        "ansisys" | "ansi" => Some(AnsiCompatibilityLevel::AnsiSys),
                        "vt100" => Some(AnsiCompatibilityLevel::Vt100),
                        "icyterm" | "icy" => Some(AnsiCompatibilityLevel::IcyTerm),
                        "utf8terminal" | "utf8" => Some(AnsiCompatibilityLevel::Utf8Terminal),
                        _ => {
                            eprintln!("Unknown level: {lvl}");
                            return Ok(());
                        }
                    };
                }
            }
            _ => {
                eprintln!("Unknown arg: {arg}");
                eprintln!("Usage: ansi_archive_roundtrip [--root PATH] [--file PATH] [--limit N] [--fail-fast] [--only-level LEVEL] [--quarantine-mismatches]");
                return Ok(());
            }
        }
    }

    let start = Instant::now();

    let is_single_file = single_file.is_some();

    let mut files = Vec::new();
    if let Some(f) = single_file {
        files.push(f);
    } else {
        collect_ans_files(&root, &mut files)?;
        files.sort();
    }

    if let Some(lim) = limit {
        files.truncate(lim);
    }

    if is_single_file {
        println!("Testing 1 file");
    } else {
        println!("Found {} *.ans under {}", files.len(), root.display());
    }

    let mut levels = vec![
        AnsiCompatibilityLevel::AnsiSys,
        AnsiCompatibilityLevel::Vt100,
        AnsiCompatibilityLevel::IcyTerm,
        AnsiCompatibilityLevel::Utf8Terminal,
    ];
    if let Some(lvl) = only_level {
        levels.retain(|l| *l == lvl);
    }

    let mut total = 0usize;
    let mut v2_mismatches = 0usize;
    let mut load_errors = 0usize;
    let mut save_errors = 0usize;
    let mut total_new_icy_bytes: u64 = 0;

    let mismatch_root = root.join(MISMATCH_DIR_NAME);
    let mismatch_v2_root = mismatch_root.join("v2");

    let mut moved_v2 = 0usize;

    'files: for (idx, path) in files.iter().enumerate() {
        total += 1;

        if idx % 250 == 0 {
            println!("[{idx}/{}] ...", files.len());
        }

        let file_bytes = match fs::read(path) {
            Ok(b) => b,
            Err(err) => {
                eprintln!("LOAD-IO-ERR: {}: {err}", path.display());
                load_errors += 1;
                if fail_fast {
                    break;
                }
                continue;
            }
        };

        let original = match FileFormat::Ansi.load(path, None) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("LOAD-ERR: {}: {err}", path.display());
                load_errors += 1;
                if fail_fast {
                    break;
                }
                continue;
            }
        };

        let original_buf = terminalize_for_compare(&original.screen.buffer);
        let orig_lines = original_buf.line_count().min(original_buf.height());

        // Preserve SAUCE if present
        let sauce_opt = icy_sauce::SauceRecord::from_bytes(&file_bytes).ok().flatten();

        let control_char_handling = AnsiSaveOptionsV2::default().control_char_handling;

        // New v2 levels
        let mut v2_failed_icyterm = false;
        for &level in &levels {
            let mut v2_opt = AnsiSaveOptionsV2::default();
            v2_opt.level = Some(level);
            v2_opt.compress = true;
            v2_opt.preserve_line_length = false;
            v2_opt.output_line_length = None;
            v2_opt.longer_terminal_output = false;
            v2_opt.control_char_handling = control_char_handling;
            v2_opt.save_sauce = sauce_opt.clone();

            let v2_bytes = match save_ansi_v2(&original_buf, &v2_opt) {
                Ok(b) => b,
                Err(err) => {
                    eprintln!("SAVE-V2-ERR ({level:?}): {}: {err}", path.display());
                    save_errors += 1;
                    if fail_fast {
                        break;
                    }
                    continue;
                }
            };

            if is_single_file {
                let _ = fs::write(format!("_debug_v2_{level:?}.ans"), &v2_bytes);
            }

            if level == AnsiCompatibilityLevel::IcyTerm {
                total_new_icy_bytes += v2_bytes.len() as u64;
            }

            let v2_rt = match load_ansi_with_sauce(&v2_bytes) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("RELOAD-V2-ERR ({level:?}): {}: {err}", path.display());
                    load_errors += 1;
                    if fail_fast {
                        break;
                    }
                    continue;
                }
            };

            let v2_rt_buf = terminalize_for_compare(&v2_rt.screen.buffer);
            let v2_lines = v2_rt_buf.line_count().min(v2_rt_buf.height());
            let cmp_lines = orig_lines.max(v2_lines);

            let (v2_size, v2_rgba) = render_buffer_rgba(&v2_rt_buf, cmp_lines);
            let (orig_cmp_size, orig_cmp_rgba) = render_buffer_rgba(&original_buf, cmp_lines);
            let v2_failed = v2_size != orig_cmp_size || v2_rgba != orig_cmp_rgba;
            if v2_failed {
                eprintln!("MISMATCH v2 {level:?}: {}", path.display());
                v2_mismatches += 1;
                if level == AnsiCompatibilityLevel::IcyTerm {
                    v2_failed_icyterm = true;
                }
                if fail_fast {
                    eprintln!(
                        "orig ice_colors={} rt ice_colors={}",
                        original.screen.terminal_state().ice_colors,
                        v2_rt.screen.terminal_state().ice_colors
                    );
                    let (tcount, tmaxy) = tag_stats(&original_buf);
                    eprintln!("orig tags={tcount} max_y={:?}", tmaxy);
                    eprintln!("v2 bytes={} sauce dims={:?}", v2_bytes.len(), sauce_stats(&v2_bytes));
                    let (cr, lf, crlf) = newline_stats(&v2_bytes);
                    eprintln!("v2 newlines: CR={cr} LF={lf} CRLF={crlf}");
                    let esc = escape_stats(&v2_bytes);
                    eprintln!(
                        "v2 escapes: esc={} csi={} IND={} NEL={} RI={} CSI_CUD={} CSI_CUP={} CSI_VPA={}",
                        esc.esc, esc.csi, esc.ind, esc.nel, esc.ri, esc.csi_cud, esc.csi_cup, esc.csi_vpa
                    );
                    if let Some(msg) = first_rgba_diff(orig_cmp_size, &orig_cmp_rgba, v2_size, &v2_rgba, original_buf.font_dimensions()) {
                        eprintln!("{msg}");
                    }
                    if let Some((x, y)) = first_rgba_diff_cell(orig_cmp_size, &orig_cmp_rgba, v2_size, &v2_rgba, original_buf.font_dimensions()) {
                        let pa = icy_engine::Position::new(x, y);
                        let ca = original_buf.char_at(pa);
                        let cb = v2_rt_buf.char_at(pa);
                        eprintln!("rgba-diff cell ({x},{y}): orig={ca:?} rt={cb:?}");
                    }
                    if let Some(msg) = first_cell_diff(&original_buf, &v2_rt_buf) {
                        eprintln!("{msg}");
                    }
                    if let Some(msg) = first_char_diff(&original_buf, &v2_rt_buf) {
                        eprintln!("{msg}");
                        // Try to locate the expected glyph near the diff position.
                        if let Some((x, y)) = first_rgba_diff_cell(orig_cmp_size, &orig_cmp_rgba, v2_size, &v2_rgba, original_buf.font_dimensions()) {
                            let expected = original_buf.char_at(icy_engine::Position::new(x, y)).ch;
                            if expected != ' ' {
                                if let Some((fx, fy)) = find_char_in_window(&v2_rt_buf, expected, x - 10, y - 3, x + 10, y + 3) {
                                    eprintln!("expected char {:?} found near diff at ({fx},{fy})", expected);
                                } else {
                                    eprintln!("expected char {:?} not found near diff window", expected);
                                }
                            }
                        }
                    }
                    break 'files;
                }
            }

            if level == AnsiCompatibilityLevel::IcyTerm {
                // Size sanity check per-file (not failing, just reporting big deltas)
                let _ = path;
            }
        }

        if v2_failed_icyterm && quarantine_mismatches {
            if let Ok(rel) = path.strip_prefix(&root) {
                let dst = mismatch_v2_root.join(rel);
                if let Some(parent) = dst.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if fs::rename(path, &dst).is_err() {
                    if fs::copy(path, &dst).is_ok() {
                        let _ = fs::remove_file(path);
                    }
                }
                moved_v2 += 1;
            }
        }
    }

    let elapsed = start.elapsed();

    println!("\nDone.");
    println!("files     : {total}");
    println!("v2 mismatches    : {v2_mismatches}");
    println!("load errs : {load_errors}");
    println!("save errs : {save_errors}");
    println!("time      : {:.1?}", elapsed);

    if quarantine_mismatches {
        println!("\nQuarantine moved: v2={} (root: {})", moved_v2, mismatch_root.display());
        println!("Moved to: {}", mismatch_v2_root.display());
    }

    if total_new_icy_bytes > 0 {
        println!("\nSize totals (IcyTerm):");
        println!("v2 bytes    : {total_new_icy_bytes}");
    }

    Ok(())
}
