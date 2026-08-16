use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use magies_desktop_lib::traffic::{SqliteTrafficCounter, TrafficCounterError, TrafficRate};
use uuid::Uuid;

#[test]
fn rolls_today_and_month_without_losing_the_lifetime_total() {
    let started_at = Instant::now();
    let mut counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), started_at).unwrap();

    counter
        .record(
            day(2026, 8, 11),
            TrafficRate {
                upload_bytes_per_second: 100,
                download_bytes_per_second: 300,
            },
            started_at,
            None,
        )
        .unwrap();
    assert_totals(counter.snapshot(), 400, 400, 400);

    counter
        .record(
            day(2026, 8, 12),
            TrafficRate {
                upload_bytes_per_second: 10,
                download_bytes_per_second: 20,
            },
            started_at + Duration::from_secs(1),
            None,
        )
        .unwrap();
    assert_totals(counter.snapshot(), 30, 430, 430);

    counter
        .record(
            day(2026, 9, 1),
            TrafficRate {
                upload_bytes_per_second: 1,
                download_bytes_per_second: 2,
            },
            started_at + Duration::from_secs(2),
            None,
        )
        .unwrap();
    assert_totals(counter.snapshot(), 3, 3, 433);
}

#[test]
fn saves_accumulated_bytes_once_the_minute_boundary_is_reached() {
    let database = TestDatabase::new("minute");
    let started_at = Instant::now();
    {
        let mut counter =
            SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), started_at).unwrap();
        counter
            .record(day(2026, 8, 11), rate(100), started_at, None)
            .unwrap();
        counter
            .record(
                day(2026, 8, 11),
                rate(200),
                started_at + Duration::from_secs(60),
                None,
            )
            .unwrap();
    }

    let reopened =
        SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), Instant::now()).unwrap();
    assert_totals(reopened.snapshot(), 300, 300, 300);
}

#[test]
fn explicit_flush_persists_bytes_before_exit() {
    let database = TestDatabase::new("exit");
    let started_at = Instant::now();
    {
        let mut counter =
            SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), started_at).unwrap();
        counter
            .record(day(2026, 8, 11), rate(512), started_at, None)
            .unwrap();
        counter.flush().unwrap();
    }

    let reopened =
        SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), Instant::now()).unwrap();
    assert_totals(reopened.snapshot(), 512, 512, 512);
}

#[test]
fn idle_tick_clears_the_rate_and_persists_a_day_rollover() {
    let database = TestDatabase::new("idle-rollover");
    let started_at = Instant::now();
    {
        let mut counter =
            SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), started_at).unwrap();
        counter
            .record(day(2026, 8, 11), rate(128), started_at, None)
            .unwrap();
        assert_eq!(counter.snapshot().upload_bytes_per_second, 128);

        counter
            .tick(day(2026, 8, 12), started_at + Duration::from_secs(60))
            .unwrap();
        assert_eq!(counter.snapshot().upload_bytes_per_second, 0);
        assert_totals(counter.snapshot(), 0, 128, 128);
    }

    let reopened =
        SqliteTrafficCounter::open(database.path(), day(2026, 8, 12), Instant::now()).unwrap();
    assert_totals(reopened.snapshot(), 0, 128, 128);
}

#[test]
fn reopening_in_a_new_month_resets_periods_but_keeps_the_total() {
    let database = TestDatabase::new("startup-rollover");
    let started_at = Instant::now();
    {
        let mut counter =
            SqliteTrafficCounter::open(database.path(), day(2026, 8, 31), started_at).unwrap();
        counter
            .record(day(2026, 8, 31), rate(64), started_at, None)
            .unwrap();
        counter.flush().unwrap();
    }

    let reopened =
        SqliteTrafficCounter::open(database.path(), day(2026, 9, 1), Instant::now()).unwrap();
    assert_totals(reopened.snapshot(), 0, 0, 64);
}

#[test]
fn rejects_a_counter_that_cannot_fit_in_sqlite() {
    let started_at = Instant::now();
    let mut counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), started_at).unwrap();
    counter
        .record(
            day(2026, 8, 11),
            TrafficRate {
                upload_bytes_per_second: i64::MAX as u64,
                download_bytes_per_second: 0,
            },
            started_at,
            None,
        )
        .unwrap();

    assert!(matches!(
        counter.record(
            day(2026, 8, 11),
            rate(1),
            started_at + Duration::from_secs(1),
            None,
        ),
        Err(TrafficCounterError::CounterOverflow)
    ));
    assert_totals(
        counter.snapshot(),
        i64::MAX as u64,
        i64::MAX as u64,
        i64::MAX as u64,
    );
}

fn assert_totals(
    snapshot: magies_desktop_lib::traffic::TrafficSnapshot,
    today: u64,
    month: u64,
    total: u64,
) {
    assert_eq!(snapshot.today_bytes, today);
    assert_eq!(snapshot.month_bytes, month);
    assert_eq!(snapshot.total_bytes, total);
}

const fn rate(bytes: u64) -> TrafficRate {
    TrafficRate {
        upload_bytes_per_second: bytes,
        download_bytes_per_second: 0,
    }
}

fn day(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

struct TestDatabase {
    path: std::path::PathBuf,
}

impl TestDatabase {
    fn new(name: &str) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            path: std::env::temp_dir().join(format!(
                "mgclash-traffic-{name}-{}-{id}.sqlite",
                std::process::id()
            )),
        }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn traffic_is_attributed_to_the_node_that_was_active() {
    let started_at = Instant::now();
    let mut counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), started_at).unwrap();
    let tokyo = Uuid::from_u128(1);
    let osaka = Uuid::from_u128(2);

    counter
        .record(
            day(2026, 8, 11),
            split_rate(100, 900),
            started_at,
            Some(tokyo),
        )
        .unwrap();
    counter
        .record(
            day(2026, 8, 11),
            split_rate(10, 20),
            started_at,
            Some(osaka),
        )
        .unwrap();
    counter
        .record(day(2026, 8, 11), split_rate(1, 2), started_at, Some(tokyo))
        .unwrap();

    let totals = counter.node_totals();
    assert_eq!(totals[&tokyo].today_upload_bytes, 101);
    assert_eq!(totals[&tokyo].today_download_bytes, 902);
    assert_eq!(totals[&osaka].today_upload_bytes, 10);
    assert_eq!(totals[&osaka].today_download_bytes, 20);
    // The global counters still see every byte, whichever node carried it.
    assert_eq!(counter.snapshot().today_bytes, 1_033);
}

#[test]
fn traffic_with_no_selected_node_is_counted_globally_only() {
    let started_at = Instant::now();
    let mut counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), started_at).unwrap();

    counter
        .record(day(2026, 8, 11), split_rate(5, 7), started_at, None)
        .unwrap();

    // Attributing to a node that is not there would be inventing a number.
    assert!(counter.node_totals().is_empty());
    assert_eq!(counter.snapshot().today_bytes, 12);
}

#[test]
fn a_node_daily_counter_rolls_over_but_its_lifetime_total_does_not() {
    let started_at = Instant::now();
    let mut counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), started_at).unwrap();
    let node = Uuid::from_u128(1);
    counter
        .record(
            day(2026, 8, 11),
            split_rate(100, 200),
            started_at,
            Some(node),
        )
        .unwrap();

    counter
        .record(day(2026, 8, 12), split_rate(1, 2), started_at, Some(node))
        .unwrap();

    let totals = counter.node_totals();
    assert_eq!(totals[&node].today_upload_bytes, 1);
    assert_eq!(totals[&node].today_download_bytes, 2);
    assert_eq!(totals[&node].total_upload_bytes, 101);
    assert_eq!(totals[&node].total_download_bytes, 202);
}

#[test]
fn node_totals_survive_a_restart() {
    let database = TestDatabase::new("node-traffic");
    let started_at = Instant::now();
    let node = Uuid::from_u128(7);
    {
        let mut counter =
            SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), started_at).unwrap();
        counter
            .record(day(2026, 8, 11), split_rate(30, 40), started_at, Some(node))
            .unwrap();
        counter.flush().unwrap();
    }

    let counter =
        SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), started_at).unwrap();

    let totals = counter.node_totals();
    assert_eq!(totals[&node].total_upload_bytes, 30);
    assert_eq!(totals[&node].total_download_bytes, 40);
}

#[test]
fn clearing_zeros_aggregate_and_per_node_totals() {
    let started_at = Instant::now();
    let mut counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), started_at).unwrap();
    let node = Uuid::from_u128(7);
    counter
        .record(day(2026, 8, 11), split_rate(12, 34), started_at, Some(node))
        .unwrap();

    counter.clear(day(2026, 8, 11), started_at).unwrap();

    assert!(counter.node_totals().is_empty());
    let snapshot = counter.snapshot();
    assert_eq!(snapshot.today_bytes, 0);
    assert_eq!(snapshot.month_bytes, 0);
    assert_eq!(snapshot.total_bytes, 0);
    assert_eq!(snapshot.upload_bytes_per_second, 0);
    assert_eq!(snapshot.download_bytes_per_second, 0);
}

#[test]
fn keeps_prior_days_in_daily_history_across_a_day_boundary() {
    let started_at = Instant::now();
    let mut counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), started_at).unwrap();

    counter
        .record(day(2026, 8, 11), rate(400), started_at, None)
        .unwrap();
    counter
        .record(
            day(2026, 8, 12),
            rate(30),
            started_at + Duration::from_secs(1),
            None,
        )
        .unwrap();

    let history = counter.daily_history();
    assert_eq!(
        history
            .iter()
            .map(|row| (row.day.as_str(), row.bytes))
            .collect::<Vec<_>>(),
        vec![("2026-08-11", 400), ("2026-08-12", 30)]
    );
}

#[test]
fn daily_history_is_empty_when_nothing_was_recorded() {
    let counter = SqliteTrafficCounter::open_in_memory(day(2026, 8, 11), Instant::now()).unwrap();
    assert!(counter.daily_history().is_empty());
}

#[test]
fn daily_history_survives_reopen_on_the_next_day() {
    let database = TestDatabase::new("daily-history");
    let started_at = Instant::now();
    {
        let mut counter =
            SqliteTrafficCounter::open(database.path(), day(2026, 8, 11), started_at).unwrap();
        counter
            .record(day(2026, 8, 11), rate(400), started_at, None)
            .unwrap();
        counter.flush().unwrap();
    }

    let mut reopened =
        SqliteTrafficCounter::open(database.path(), day(2026, 8, 12), Instant::now()).unwrap();
    reopened
        .record(day(2026, 8, 12), rate(30), Instant::now(), None)
        .unwrap();

    let history = reopened.daily_history();
    assert_eq!(
        history
            .iter()
            .map(|row| (row.day.as_str(), row.bytes))
            .collect::<Vec<_>>(),
        vec![("2026-08-11", 400), ("2026-08-12", 30)]
    );
}

fn split_rate(upload: u64, download: u64) -> TrafficRate {
    TrafficRate {
        upload_bytes_per_second: upload,
        download_bytes_per_second: download,
    }
}
