//! Opt-in timing instrumentation for the mail reader.
//!
//! Disabled unless `ICY_MAIL_PERF=1` is set, so normal runs pay nothing but the timings
//! stay available while the remaining components get reworked.

use std::sync::OnceLock;
use std::time::Instant;

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("ICY_MAIL_PERF").is_ok_and(|v| v != "0"))
}

/// Prints how long a scope took, with an optional detail string.
pub struct Timer {
    label: &'static str,
    detail: String,
    start: Option<Instant>,
}

impl Timer {
    #[must_use]
    pub fn new(label: &'static str) -> Self {
        Self::with(label, "")
    }

    pub fn with(label: &'static str, detail: impl std::fmt::Display) -> Self {
        let active = enabled();
        Self {
            label,
            detail: if active { detail.to_string() } else { String::new() },
            start: active.then(Instant::now),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let Some(start) = self.start else { return };
        let micros = start.elapsed().as_micros();
        if self.detail.is_empty() {
            println!("[perf] {:<28} {micros:>9} us", self.label);
        } else {
            println!("[perf] {:<28} {micros:>9} us   ({})", self.label, self.detail);
        }
    }
}

/// Counts `view()` calls and reports the rate once per second.
pub fn count_frame(rows: usize) {
    use std::cell::Cell;
    thread_local! {
        static FRAMES: Cell<u32> = const { Cell::new(0) };
        static WINDOW_START: Cell<Option<Instant>> = const { Cell::new(None) };
    }

    if !enabled() {
        return;
    }

    WINDOW_START.with(|start| {
        let began = start.get().unwrap_or_else(|| {
            let now = Instant::now();
            start.set(Some(now));
            now
        });

        FRAMES.with(|frames| {
            frames.set(frames.get() + 1);
            if began.elapsed().as_secs() >= 1 {
                println!("[perf] views/sec {:>4}   rows in list: {rows}", frames.get());
                frames.set(0);
                start.set(Some(Instant::now()));
            }
        });
    });
}
