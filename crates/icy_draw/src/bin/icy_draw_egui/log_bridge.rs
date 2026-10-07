//! Forwards `tracing` events to the `log` logger, prefixed with their span context.
//!
//! Without a tracing subscriber, crates built with tracing's `log` feature (pulled in by calloop)
//! print every span *entry* as a log line at the span's level. The Copilot SDK opens an ERROR-level
//! span for each tool call, which showed up as an "ERROR" line per drawing step although nothing
//! failed. This subscriber logs events only and uses spans as context.

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Write},
    sync::atomic::{AtomicU64, Ordering},
};

use parking_lot::Mutex;
use tracing::{
    field::{Field, Visit},
    span, Event, Level, Metadata, Subscriber,
};

thread_local! {
    static ENTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

struct SpanData {
    text: String,
    references: usize,
}

pub struct LogBridge {
    max_level: log::LevelFilter,
    next_id: AtomicU64,
    spans: Mutex<HashMap<u64, SpanData>>,
    sink: fn(&log::Record),
}

impl LogBridge {
    /// Forwards to the installed `log` logger; call after the logger has been started.
    pub fn new() -> Self {
        Self::with_sink(log::max_level(), |record| log::logger().log(record))
    }

    fn with_sink(max_level: log::LevelFilter, sink: fn(&log::Record)) -> Self {
        Self {
            max_level,
            next_id: AtomicU64::new(1),
            spans: Mutex::new(HashMap::new()),
            sink,
        }
    }

    /// Installs the bridge as the global tracing subscriber.
    pub fn install() {
        if tracing::subscriber::set_global_default(Self::new()).is_err() {
            log::warn!("A tracing subscriber was already installed");
        }
    }
}

fn level(level: &Level) -> log::Level {
    match *level {
        Level::ERROR => log::Level::Error,
        Level::WARN => log::Level::Warn,
        Level::INFO => log::Level::Info,
        Level::DEBUG => log::Level::Debug,
        Level::TRACE => log::Level::Trace,
    }
}

/// Collects `message` and `name=value` fields.
#[derive(Default)]
struct Fields(String);

impl Fields {
    fn push(&mut self, field: &Field, value: fmt::Arguments<'_>) {
        if field.name() == "message" {
            let rest = std::mem::take(&mut self.0);
            let _ = write!(self.0, "{value}{rest}");
        } else {
            let _ = write!(self.0, " {}={value}", field.name());
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, format_args!("{value}"));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.push(field, format_args!("{value}"));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push(field, format_args!("{value:?}"));
    }
}

impl Subscriber for LogBridge {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        level(metadata.level()) <= self.max_level
    }

    fn new_span(&self, attributes: &span::Attributes<'_>) -> span::Id {
        let mut fields = Fields::default();
        attributes.record(&mut fields);
        let mut text = attributes.metadata().name().to_owned();
        if !fields.0.is_empty() {
            let _ = write!(text, "{{{}}}", fields.0.trim_start());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.spans.lock().insert(id, SpanData { text, references: 1 });
        span::Id::from_u64(id)
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        let mut fields = Fields::default();
        values.record(&mut fields);
        if let Some(data) = self.spans.lock().get_mut(&span.into_u64()) {
            data.text.push_str(&fields.0);
        }
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    fn event(&self, event: &Event<'_>) {
        let metadata = event.metadata();
        let mut fields = Fields::default();
        event.record(&mut fields);
        let context = ENTERED.with(|entered| {
            let spans = self.spans.lock();
            entered
                .borrow()
                .iter()
                .filter_map(|id| spans.get(id).map(|data| data.text.clone()))
                .collect::<Vec<_>>()
                .join(": ")
        });
        let message = fields.0.trim_start();
        let separator = if context.is_empty() { "" } else { ": " };
        (self.sink)(
            &log::Record::builder()
                .level(level(metadata.level()))
                .target(metadata.target())
                .module_path(metadata.module_path())
                .file(metadata.file())
                .line(metadata.line())
                .args(format_args!("{context}{separator}{message}"))
                .build(),
        );
    }

    fn enter(&self, span: &span::Id) {
        ENTERED.with(|entered| entered.borrow_mut().push(span.into_u64()));
    }

    fn exit(&self, span: &span::Id) {
        let id = span.into_u64();
        ENTERED.with(|entered| {
            let mut entered = entered.borrow_mut();
            if let Some(index) = entered.iter().rposition(|entry| *entry == id) {
                entered.remove(index);
            }
        });
    }

    fn clone_span(&self, span: &span::Id) -> span::Id {
        if let Some(data) = self.spans.lock().get_mut(&span.into_u64()) {
            data.references += 1;
        }
        span.clone()
    }

    fn try_close(&self, span: span::Id) -> bool {
        let mut spans = self.spans.lock();
        let id = span.into_u64();
        let closed = spans.get_mut(&id).is_some_and(|data| {
            data.references -= 1;
            data.references == 0
        });
        if closed {
            spans.remove(&id);
        }
        closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static LINES: Mutex<Vec<(log::Level, String)>> = Mutex::new(Vec::new());

    fn capture(record: &log::Record) {
        LINES.lock().push((record.level(), record.args().to_string()));
    }

    #[test]
    fn spans_are_context_not_log_lines() {
        let bridge = LogBridge::with_sink(log::LevelFilter::Info, capture);
        tracing::subscriber::with_default(bridge, || {
            let span = tracing::error_span!("external_tool_handler", request_id = "r1");
            let _entered = span.enter();
            tracing::debug!("hidden below the level");
            tracing::warn!(tool = "icy_fill_rect", "tool failed");
        });
        let lines = LINES.lock();
        assert_eq!(
            *lines,
            [(
                log::Level::Warn,
                "external_tool_handler{request_id=r1}: tool failed tool=icy_fill_rect".to_owned()
            )]
        );
    }
}
