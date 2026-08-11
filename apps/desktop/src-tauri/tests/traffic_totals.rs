use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use magies_desktop_lib::traffic::{SqliteTrafficCounter, TrafficCounterError, TrafficRate};

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
            .record(day(2026, 8, 11), rate(100), started_at)
            .unwrap();
        counter
            .record(
                day(2026, 8, 11),
                rate(200),
                started_at + Duration::from_secs(60),
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
            .record(day(2026, 8, 11), rate(512), started_at)
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
            .record(day(2026, 8, 11), rate(128), started_at)
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
            .record(day(2026, 8, 31), rate(64), started_at)
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
        )
        .unwrap();

    assert!(matches!(
        counter.record(
            day(2026, 8, 11),
            rate(1),
            started_at + Duration::from_secs(1),
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
