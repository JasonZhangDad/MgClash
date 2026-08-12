//! Covers the log buffer: bounding, level filtering, source filtering, the
//! `tracing` bridge, and the Core level parsing the panel relies on.

use std::sync::Arc;

use magies_desktop_lib::logs::{
    LOG_CAPACITY, LogBuffer, LogBufferLayer, LogLevel, LogSource, drain_core_lines,
};
use magies_profiles::{DiagnosticRedactor, REDACTED};
use tracing::subscriber::with_default;
use tracing_subscriber::layer::SubscriberExt;

fn buffer() -> LogBuffer {
    LogBuffer::new(8)
}

#[test]
fn keeps_entries_in_arrival_order() {
    let buffer = buffer();

    buffer.record(LogLevel::Info, LogSource::App, "first");
    buffer.record(LogLevel::Info, LogSource::Core, "second");

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].message, "first");
    assert_eq!(entries[1].message, "second");
    assert_eq!(entries[0].source, LogSource::App);
    assert_eq!(entries[1].source, LogSource::Core);
}

#[test]
fn drops_the_oldest_entry_once_full() {
    let buffer = LogBuffer::new(3);

    for index in 0..5 {
        buffer.record(LogLevel::Info, LogSource::Core, format!("line {index}"));
    }

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].message, "line 2");
    assert_eq!(entries[2].message, "line 4");
}

#[test]
fn a_zero_capacity_buffer_still_keeps_one_entry() {
    let buffer = LogBuffer::new(0);

    buffer.record(LogLevel::Info, LogSource::App, "only");

    assert_eq!(buffer.len(), 1);
}

#[test]
fn filters_by_minimum_level() {
    let buffer = buffer();
    buffer.record(LogLevel::Error, LogSource::App, "error");
    buffer.record(LogLevel::Warn, LogSource::App, "warn");
    buffer.record(LogLevel::Info, LogSource::App, "info");
    buffer.record(LogLevel::Debug, LogSource::App, "debug");
    buffer.record(LogLevel::Trace, LogSource::App, "trace");

    let messages: Vec<_> = buffer
        .snapshot(LogLevel::Warn, None)
        .into_iter()
        .map(|entry| entry.message)
        .collect();

    assert_eq!(messages, ["error", "warn"]);
}

#[test]
fn filters_by_source() {
    let buffer = buffer();
    buffer.record(LogLevel::Info, LogSource::App, "app line");
    buffer.record(LogLevel::Info, LogSource::Core, "core line");

    let core: Vec<_> = buffer
        .snapshot(LogLevel::Trace, Some(LogSource::Core))
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    let app: Vec<_> = buffer
        .snapshot(LogLevel::Trace, Some(LogSource::App))
        .into_iter()
        .map(|entry| entry.message)
        .collect();

    assert_eq!(core, ["core line"]);
    assert_eq!(app, ["app line"]);
}

#[test]
fn clearing_empties_the_buffer() {
    let buffer = buffer();
    buffer.record(LogLevel::Info, LogSource::App, "line");
    assert!(!buffer.is_empty());

    buffer.clear();

    assert!(buffer.is_empty());
    assert!(buffer.snapshot(LogLevel::Trace, None).is_empty());
}

#[test]
fn the_default_capacity_is_bounded() {
    let buffer = LogBuffer::default();

    for index in 0..(LOG_CAPACITY + 50) {
        buffer.record(LogLevel::Info, LogSource::Core, format!("line {index}"));
    }

    assert_eq!(buffer.len(), LOG_CAPACITY);
}

#[test]
fn records_tracing_events_with_their_level_and_message() {
    let buffer = Arc::new(LogBuffer::new(16));
    let subscriber = tracing_subscriber::registry().with(LogBufferLayer::new(buffer.clone()));

    with_default(subscriber, || {
        tracing::info!("session connected");
        tracing::warn!("tray refresh failed");
        tracing::error!("session recovery failed");
    });

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].level, LogLevel::Info);
    assert_eq!(entries[0].message, "session connected");
    assert_eq!(entries[0].source, LogSource::App);
    assert_eq!(entries[1].level, LogLevel::Warn);
    assert_eq!(entries[2].level, LogLevel::Error);
}

#[test]
fn appends_tracing_fields_after_the_message() {
    let buffer = Arc::new(LogBuffer::new(16));
    let subscriber = tracing_subscriber::registry().with(LogBufferLayer::new(buffer.clone()));

    with_default(subscriber, || {
        tracing::info!(node = "Tokyo", "session connected");
    });

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries[0].message, "session connected node=Tokyo");
}

#[test]
fn reads_the_level_a_core_printed() {
    assert_eq!(
        LogLevel::from_core_line("+0800 2026-08-12 03:04:05 ERROR outbound failed"),
        LogLevel::Error
    );
    assert_eq!(
        LogLevel::from_core_line("+0800 2026-08-12 03:04:05 WARN slow response"),
        LogLevel::Warn
    );
    assert_eq!(
        LogLevel::from_core_line("+0800 2026-08-12 03:04:05 DEBUG dialing"),
        LogLevel::Debug
    );
    assert_eq!(
        LogLevel::from_core_line("+0800 2026-08-12 03:04:05 TRACE packet"),
        LogLevel::Trace
    );
    assert_eq!(
        LogLevel::from_core_line("FATAL cannot bind"),
        LogLevel::Error
    );
}

#[test]
fn an_unlabelled_core_line_is_treated_as_info() {
    assert_eq!(LogLevel::from_core_line("sing-box started"), LogLevel::Info);
}

#[test]
fn a_level_word_late_in_a_line_does_not_change_it() {
    // Only the leading text is inspected, so a message mentioning "error"
    // far into the line is not promoted.
    let line = format!("2026-08-12 INFO {} error", "x".repeat(80));

    assert_eq!(LogLevel::from_core_line(&line), LogLevel::Info);
}

#[test]
fn core_lines_are_redacted_before_they_reach_the_panel() {
    let buffer = LogBuffer::new(16);
    let redactor = DiagnosticRedactor::new().with_secret("hunter2");
    let mut pending = String::from("2026-08-12 INFO outbound dialed with password hunter2\n");

    drain_core_lines(&mut pending, &redactor, &buffer);

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries.len(), 1);
    assert!(
        !entries[0].message.contains("hunter2"),
        "credential leaked into the panel: {}",
        entries[0].message
    );
    assert!(entries[0].message.contains(REDACTED));
}

#[test]
fn a_line_split_across_chunks_is_emitted_once_whole() {
    let buffer = LogBuffer::new(16);
    let redactor = DiagnosticRedactor::new();
    let mut pending = String::from("2026-08-12 INFO star");

    // First chunk ends mid-line: nothing is emitted yet.
    drain_core_lines(&mut pending, &redactor, &buffer);
    assert!(buffer.is_empty());

    pending.push_str("ted listening\n");
    drain_core_lines(&mut pending, &redactor, &buffer);

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].message, "2026-08-12 INFO started listening");
    assert!(pending.is_empty());
}

#[test]
fn core_lines_carry_their_own_level_and_source() {
    let buffer = LogBuffer::new(16);
    let redactor = DiagnosticRedactor::new();
    let mut pending = String::from(
        "2026-08-12 INFO started\n2026-08-12 ERROR dial failed\n2026-08-12 WARN retrying\n",
    );

    drain_core_lines(&mut pending, &redactor, &buffer);

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().all(|entry| entry.source == LogSource::Core));
    assert_eq!(entries[0].level, LogLevel::Info);
    assert_eq!(entries[1].level, LogLevel::Error);
    assert_eq!(entries[2].level, LogLevel::Warn);
}

#[test]
fn blank_and_carriage_returned_core_lines_are_skipped_and_trimmed() {
    let buffer = LogBuffer::new(16);
    let redactor = DiagnosticRedactor::new();
    let mut pending = String::from("\n   \n2026-08-12 INFO started\r\n");

    drain_core_lines(&mut pending, &redactor, &buffer);

    let entries = buffer.snapshot(LogLevel::Trace, None);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].message, "2026-08-12 INFO started");
}

#[test]
fn level_filtering_keeps_more_severe_entries() {
    assert!(LogLevel::Error.passes(LogLevel::Warn));
    assert!(LogLevel::Warn.passes(LogLevel::Warn));
    assert!(!LogLevel::Info.passes(LogLevel::Warn));
    assert!(LogLevel::Trace.passes(LogLevel::Trace));
}
