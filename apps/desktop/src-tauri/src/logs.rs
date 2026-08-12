//! The in-memory log the desktop shell shows and the sinks that fill it.
//!
//! Two very different producers share one buffer:
//!
//! - application events, recorded through `tracing` and collected by
//!   [`LogBufferLayer`]
//! - the Core's stdout and stderr, which `tracing` knows nothing about because
//!   they belong to a child process; [`spawn_core_log_reader`] consumes them
//!
//! Core lines are redacted before they are stored. A Core can print an outbound
//! it just dialled, and showing that verbatim would put credentials on screen
//! that the diagnostic bundle deliberately removes.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use chrono::Local;
use magies_core_runtime::{CoreOutput, CoreOutputEvent, CoreOutputStream};
use magies_profiles::DiagnosticRedactor;
use serde::{Deserialize, Serialize};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// How many entries the panel keeps. A busy Core prints continuously, so the
/// buffer is bounded and drops the oldest lines rather than growing without
/// limit.
pub const LOG_CAPACITY: usize = 2_000;

/// How long the Core reader waits for the next chunk before checking again
/// whether the channel is still open.
const CORE_POLL_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// Parses the level a Core printed, defaulting to [`Self::Info`].
    ///
    /// sing-box writes lines such as `+0800 2026-08-12 03:04:05 INFO ...`, so
    /// the level is matched anywhere in the leading text rather than at a fixed
    /// offset.
    #[must_use]
    pub fn from_core_line(line: &str) -> Self {
        let head: String = line
            .chars()
            .take(64)
            .collect::<String>()
            .to_ascii_uppercase();
        for (needle, level) in [
            ("ERROR", Self::Error),
            ("FATAL", Self::Error),
            ("PANIC", Self::Error),
            ("WARN", Self::Warn),
            ("DEBUG", Self::Debug),
            ("TRACE", Self::Trace),
        ] {
            if head.contains(needle) {
                return level;
            }
        }
        Self::Info
    }

    const fn from_tracing(level: Level) -> Self {
        match level {
            Level::ERROR => Self::Error,
            Level::WARN => Self::Warn,
            Level::INFO => Self::Info,
            Level::DEBUG => Self::Debug,
            Level::TRACE => Self::Trace,
        }
    }

    /// Whether an entry at this level passes a filter set to `minimum`.
    ///
    /// The variants are ordered most to least severe, so a lower discriminant
    /// is a more severe entry.
    #[must_use]
    pub fn passes(self, minimum: Self) -> bool {
        self <= minimum
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogSource {
    App,
    Core,
}

/// One line as the panel renders it. Credentials never reach this type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp_ms: i64,
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
}

/// A bounded ring of log entries shared by every producer and the UI.
#[derive(Debug)]
pub struct LogBuffer {
    entries: Mutex<VecDeque<LogEntry>>,
    capacity: usize,
}

impl LogBuffer {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity.min(LOG_CAPACITY))),
            capacity: capacity.max(1),
        }
    }

    fn entries(&self) -> MutexGuard<'_, VecDeque<LogEntry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends an entry, dropping the oldest one once the ring is full.
    pub fn push(&self, entry: LogEntry) {
        let mut entries = self.entries();
        while entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// Records a message that did not come from `tracing`.
    pub fn record(&self, level: LogLevel, source: LogSource, message: impl Into<String>) {
        self.push(LogEntry {
            timestamp_ms: Local::now().timestamp_millis(),
            level,
            source,
            message: message.into(),
        });
    }

    /// Returns the entries at or above `minimum`, oldest first.
    #[must_use]
    pub fn snapshot(&self, minimum: LogLevel, source: Option<LogSource>) -> Vec<LogEntry> {
        self.entries()
            .iter()
            .filter(|entry| entry.level.passes(minimum))
            .filter(|entry| source.is_none_or(|wanted| entry.source == wanted))
            .cloned()
            .collect()
    }

    pub fn clear(&self) {
        self.entries().clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(LOG_CAPACITY)
    }
}

/// Collects `tracing` events into a [`LogBuffer`].
pub struct LogBufferLayer {
    buffer: Arc<LogBuffer>,
}

impl LogBufferLayer {
    #[must_use]
    pub const fn new(buffer: Arc<LogBuffer>) -> Self {
        Self { buffer }
    }
}

impl<S: Subscriber> Layer<S> for LogBufferLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut message = MessageVisitor::default();
        event.record(&mut message);
        self.buffer.record(
            LogLevel::from_tracing(*event.metadata().level()),
            LogSource::App,
            message.finish(event.metadata().target()),
        );
    }
}

/// Flattens an event's fields into one line, keeping `message` first.
#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl MessageVisitor {
    fn finish(self, target: &str) -> String {
        let mut line = self.message.unwrap_or_else(|| target.to_owned());
        for field in self.fields {
            line.push(' ');
            line.push_str(&field);
        }
        line
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields.push(format!("{}={value:?}", field.name()));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_owned());
        } else {
            self.fields.push(format!("{}={value}", field.name()));
        }
    }
}

/// Drains a Core's stdout and stderr into `buffer` until the process exits.
///
/// Every line is redacted first: the Core prints its own configuration on
/// startup, and that text can carry the node credential.
pub fn spawn_core_log_reader(output: CoreOutput, buffer: Arc<LogBuffer>) {
    std::thread::spawn(move || read_core_output(&output, &buffer));
}

fn read_core_output(output: &CoreOutput, buffer: &LogBuffer) {
    let redactor = DiagnosticRedactor::new();
    let mut pending = String::new();
    loop {
        match output.recv_timeout(CORE_POLL_TIMEOUT) {
            Ok(CoreOutputEvent::Chunk { bytes, .. }) => {
                pending.push_str(&String::from_utf8_lossy(&bytes));
                drain_core_lines(&mut pending, &redactor, buffer);
            }
            Ok(CoreOutputEvent::ReadFailed { stream, source }) => {
                buffer.record(
                    LogLevel::Error,
                    LogSource::Core,
                    format!("failed to read Core {}: {source}", stream_name(stream)),
                );
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Whatever the Core printed without a trailing newline is still worth
    // showing once it has exited.
    let trailing = pending.trim_end_matches(['\r', '\n']).trim().to_owned();
    if !trailing.is_empty() {
        buffer.record(
            LogLevel::from_core_line(&trailing),
            LogSource::Core,
            redactor.redact_text(&trailing),
        );
    }
}

/// Emits every complete line held in `pending`, keeping any partial tail.
///
/// Public so the redaction and line-splitting behaviour can be tested directly:
/// a [`CoreOutput`] can only be produced by a running Core, and this is the step
/// that keeps credentials off the screen.
///
/// The originating stream is deliberately ignored: stderr from a Core is not
/// automatically an error, because sing-box writes every level there, so the
/// level comes from the line itself.
pub fn drain_core_lines(pending: &mut String, redactor: &DiagnosticRedactor, buffer: &LogBuffer) {
    while let Some(index) = pending.find('\n') {
        let line: String = pending.drain(..=index).collect();
        let line = line.trim_end_matches(['\r', '\n']).trim();
        if line.is_empty() {
            continue;
        }
        buffer.record(
            LogLevel::from_core_line(line),
            LogSource::Core,
            redactor.redact_text(line),
        );
    }
}

const fn stream_name(stream: CoreOutputStream) -> &'static str {
    match stream {
        CoreOutputStream::Stdout => "stdout",
        CoreOutputStream::Stderr => "stderr",
    }
}
