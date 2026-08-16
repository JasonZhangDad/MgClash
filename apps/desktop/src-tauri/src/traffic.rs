//! Live traffic samples from sing-box's loopback-only Clash API.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{Datelike, NaiveDate};
use magies_profiles::ensure_rustls_crypto_provider;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const SAMPLE_BODY_LIMIT: usize = 1_024;
const PERSIST_INTERVAL: Duration = Duration::from_secs(60);
const MAX_TRAFFIC_BYTES: u64 = i64::MAX as u64;

/// Upload and download bytes observed during the previous second.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficRate {
    pub upload_bytes_per_second: u64,
    pub download_bytes_per_second: u64,
}

/// One node's own traffic, split by direction as the node table shows it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTraffic {
    pub today_upload_bytes: u64,
    pub today_download_bytes: u64,
    pub total_upload_bytes: u64,
    pub total_download_bytes: u64,
}

impl NodeTraffic {
    /// Clears the daily counters when the calendar has moved on.
    fn rolled_to(mut self, day: NaiveDate, stored_day: NaiveDate) -> Self {
        if stored_day != day {
            self.today_upload_bytes = 0;
            self.today_download_bytes = 0;
        }
        self
    }

    fn with_added(mut self, rate: TrafficRate) -> Result<Self, TrafficCounterError> {
        self.today_upload_bytes = add(self.today_upload_bytes, rate.upload_bytes_per_second)?;
        self.today_download_bytes = add(self.today_download_bytes, rate.download_bytes_per_second)?;
        self.total_upload_bytes = add(self.total_upload_bytes, rate.upload_bytes_per_second)?;
        self.total_download_bytes = add(self.total_download_bytes, rate.download_bytes_per_second)?;
        Ok(self)
    }
}

fn add(total: u64, bytes: u64) -> Result<u64, TrafficCounterError> {
    total
        .checked_add(bytes)
        .filter(|total| *total <= MAX_TRAFFIC_BYTES)
        .ok_or(TrafficCounterError::CounterOverflow)
}

/// Current live rate and the three V0.1 persisted traffic counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSnapshot {
    pub upload_bytes_per_second: u64,
    pub download_bytes_per_second: u64,
    pub today_bytes: u64,
    pub month_bytes: u64,
    pub total_bytes: u64,
}

/// One calendar day's persisted aggregate, as `traffic_daily` stores it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTraffic {
    pub day: String,
    pub bytes: u64,
}

/// Live snapshot plus the persisted daily series for the Traffic page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficReport {
    pub upload_bytes_per_second: u64,
    pub download_bytes_per_second: u64,
    pub today_bytes: u64,
    pub month_bytes: u64,
    pub total_bytes: u64,
    pub daily: Vec<DailyTraffic>,
}

impl TrafficReport {
    #[must_use]
    pub fn from_counter(counter: &SqliteTrafficCounter) -> Self {
        let snapshot = counter.snapshot();
        Self {
            upload_bytes_per_second: snapshot.upload_bytes_per_second,
            download_bytes_per_second: snapshot.download_bytes_per_second,
            today_bytes: snapshot.today_bytes,
            month_bytes: snapshot.month_bytes,
            total_bytes: snapshot.total_bytes,
            daily: counter.daily_history(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrafficTotals {
    day: NaiveDate,
    today_bytes: u64,
    month_bytes: u64,
    total_bytes: u64,
}

impl TrafficTotals {
    const fn empty(day: NaiveDate) -> Self {
        Self {
            day,
            today_bytes: 0,
            month_bytes: 0,
            total_bytes: 0,
        }
    }

    fn rolled_to(mut self, day: NaiveDate) -> (Self, bool) {
        if self.day == day {
            return (self, false);
        }
        if self.day.year() != day.year() || self.day.month() != day.month() {
            self.month_bytes = 0;
        }
        self.day = day;
        self.today_bytes = 0;
        (self, true)
    }

    fn with_added_bytes(mut self, bytes: u64) -> Result<Self, TrafficCounterError> {
        self.today_bytes = checked_counter_add(self.today_bytes, bytes)?;
        self.month_bytes = checked_counter_add(self.month_bytes, bytes)?;
        self.total_bytes = checked_counter_add(self.total_bytes, bytes)?;
        Ok(self)
    }
}

/// SQLite-backed aggregate traffic counter. It stores no destination or
/// per-connection data.
#[derive(Debug)]
pub struct SqliteTrafficCounter {
    connection: Connection,
    totals: TrafficTotals,
    rate: TrafficRate,
    dirty: bool,
    last_persisted_at: Instant,
    node_totals: HashMap<Uuid, NodeTraffic>,
    /// The day each node's daily counters belong to, so a node untouched since
    /// yesterday rolls over when it next carries traffic rather than on load.
    node_days: HashMap<Uuid, NaiveDate>,
    /// Per-day aggregates that survive a calendar roll (`traffic_daily`).
    daily: BTreeMap<NaiveDate, u64>,
}

impl SqliteTrafficCounter {
    /// Opens the traffic counter and rolls persisted values to `today`.
    ///
    /// # Errors
    ///
    /// Returns a typed database or stored-value error.
    pub fn open(
        path: impl AsRef<Path>,
        today: NaiveDate,
        now: Instant,
    ) -> Result<Self, TrafficCounterError> {
        Self::from_connection(Connection::open(path)?, today, now)
    }

    /// Creates an isolated counter for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the schema cannot be initialized.
    pub fn open_in_memory(today: NaiveDate, now: Instant) -> Result<Self, TrafficCounterError> {
        Self::from_connection(Connection::open_in_memory()?, today, now)
    }

    /// Adds a one-second Core sample and persists once per minute.
    ///
    /// # Errors
    ///
    /// Returns a typed overflow or database error. Failed persistence leaves
    /// the in-memory counters dirty so the next sample retries the write.
    pub fn record(
        &mut self,
        today: NaiveDate,
        rate: TrafficRate,
        now: Instant,
        node: Option<Uuid>,
    ) -> Result<(), TrafficCounterError> {
        // Only one node carries traffic at a time, so the second's bytes belong
        // to whichever node the session is running. With none selected there is
        // nothing to attribute them to, and inventing an owner would be worse
        // than leaving the per-node counters alone.
        if let Some(node) = node {
            let previous = self.node_totals.get(&node).copied().unwrap_or_default();
            let rolled =
                previous.rolled_to(today, self.node_days.get(&node).copied().unwrap_or(today));
            self.node_totals.insert(node, rolled.with_added(rate)?);
            self.node_days.insert(node, today);
        }
        let bytes = rate
            .upload_bytes_per_second
            .checked_add(rate.download_bytes_per_second)
            .filter(|bytes| *bytes <= MAX_TRAFFIC_BYTES)
            .ok_or(TrafficCounterError::CounterOverflow)?;
        let (totals, _) = self.totals.rolled_to(today);
        self.totals = totals.with_added_bytes(bytes)?;
        let day_total = self.daily.get(&today).copied().unwrap_or(0);
        self.daily.insert(today, add(day_total, bytes)?);
        self.rate = rate;
        self.dirty = true;
        self.persist_if_due(now)
    }

    /// Advances the calendar and clears a stale live rate while idle.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when a due write fails.
    pub fn tick(&mut self, today: NaiveDate, now: Instant) -> Result<(), TrafficCounterError> {
        self.rate = TrafficRate::default();
        let (totals, rolled) = self.totals.rolled_to(today);
        self.totals = totals;
        self.dirty |= rolled;
        self.persist_if_due(now)
    }

    /// Every node's own counters, for the columns the node table shows.
    #[must_use]
    pub fn node_totals(&self) -> HashMap<Uuid, NodeTraffic> {
        self.node_totals.clone()
    }

    /// Zeros aggregate and per-node counters and persists immediately.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the wipe cannot be written.
    pub fn clear(&mut self, today: NaiveDate, now: Instant) -> Result<(), TrafficCounterError> {
        self.totals = TrafficTotals::empty(today);
        self.rate = TrafficRate::default();
        self.node_totals.clear();
        self.node_days.clear();
        self.daily.clear();
        self.dirty = true;
        self.connection.execute("DELETE FROM node_traffic", [])?;
        self.connection.execute("DELETE FROM traffic_daily", [])?;
        self.flush()?;
        self.last_persisted_at = now;
        Ok(())
    }

    /// Persisted daily aggregates, oldest first. Days with no bytes are omitted.
    #[must_use]
    pub fn daily_history(&self) -> Vec<DailyTraffic> {
        self.daily
            .iter()
            .filter(|(_, bytes)| **bytes > 0)
            .map(|(day, bytes)| DailyTraffic {
                day: day.format("%Y-%m-%d").to_string(),
                bytes: *bytes,
            })
            .collect()
    }

    #[must_use]
    pub const fn snapshot(&self) -> TrafficSnapshot {
        TrafficSnapshot {
            upload_bytes_per_second: self.rate.upload_bytes_per_second,
            download_bytes_per_second: self.rate.download_bytes_per_second,
            today_bytes: self.totals.today_bytes,
            month_bytes: self.totals.month_bytes,
            total_bytes: self.totals.total_bytes,
        }
    }

    /// Writes pending counters immediately, including during app shutdown.
    ///
    /// # Errors
    ///
    /// Returns a typed database error and keeps the counter dirty on failure.
    pub fn flush(&mut self) -> Result<(), TrafficCounterError> {
        if !self.dirty {
            return Ok(());
        }
        self.connection.execute(
            "INSERT INTO traffic_totals (
                 id, day, today_bytes, month_bytes, total_bytes
             ) VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 day = excluded.day,
                 today_bytes = excluded.today_bytes,
                 month_bytes = excluded.month_bytes,
                 total_bytes = excluded.total_bytes",
            params![
                self.totals.day.format("%Y-%m-%d").to_string(),
                self.totals.today_bytes,
                self.totals.month_bytes,
                self.totals.total_bytes,
            ],
        )?;
        for (day, bytes) in &self.daily {
            if *bytes == 0 {
                continue;
            }
            self.connection.execute(
                "INSERT INTO traffic_daily (day, bytes) VALUES (?1, ?2)
                 ON CONFLICT(day) DO UPDATE SET bytes = excluded.bytes",
                params![day.format("%Y-%m-%d").to_string(), bytes],
            )?;
        }
        for (id, totals) in &self.node_totals {
            let day = self.node_days.get(id).copied().unwrap_or(self.totals.day);
            self.connection.execute(
                "INSERT INTO node_traffic (
                     node_id, day, today_upload, today_download, total_upload, total_download
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(node_id) DO UPDATE SET
                     day = excluded.day,
                     today_upload = excluded.today_upload,
                     today_download = excluded.today_download,
                     total_upload = excluded.total_upload,
                     total_download = excluded.total_download",
                params![
                    id.to_string(),
                    day.format("%Y-%m-%d").to_string(),
                    totals.today_upload_bytes,
                    totals.today_download_bytes,
                    totals.total_upload_bytes,
                    totals.total_download_bytes,
                ],
            )?;
        }
        self.dirty = false;
        Ok(())
    }

    fn from_connection(
        connection: Connection,
        today: NaiveDate,
        now: Instant,
    ) -> Result<Self, TrafficCounterError> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS traffic_totals (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 day TEXT NOT NULL,
                 today_bytes INTEGER NOT NULL CHECK (today_bytes >= 0),
                 month_bytes INTEGER NOT NULL CHECK (month_bytes >= 0),
                 total_bytes INTEGER NOT NULL CHECK (total_bytes >= 0)
             );
             CREATE TABLE IF NOT EXISTS node_traffic (
                 node_id TEXT PRIMARY KEY,
                 day TEXT NOT NULL,
                 today_upload INTEGER NOT NULL CHECK (today_upload >= 0),
                 today_download INTEGER NOT NULL CHECK (today_download >= 0),
                 total_upload INTEGER NOT NULL CHECK (total_upload >= 0),
                 total_download INTEGER NOT NULL CHECK (total_download >= 0)
             );
             CREATE TABLE IF NOT EXISTS traffic_daily (
                 day TEXT PRIMARY KEY,
                 bytes INTEGER NOT NULL CHECK (bytes >= 0)
             );",
        )?;
        let stored = connection
            .query_row(
                "SELECT day, today_bytes, month_bytes, total_bytes
                 FROM traffic_totals WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let totals = match stored {
            None => TrafficTotals::empty(today),
            Some((day, today_bytes, month_bytes, total_bytes)) => TrafficTotals {
                day: NaiveDate::parse_from_str(&day, "%Y-%m-%d").map_err(|source| {
                    TrafficCounterError::InvalidStoredDay { value: day, source }
                })?,
                today_bytes: decode_counter("today_bytes", today_bytes)?,
                month_bytes: decode_counter("month_bytes", month_bytes)?,
                total_bytes: decode_counter("total_bytes", total_bytes)?,
            },
        };
        let mut daily = load_daily_traffic(&connection)?;
        if totals.today_bytes > 0 {
            daily.entry(totals.day).or_insert(totals.today_bytes);
        }
        let (totals, dirty) = totals.rolled_to(today);
        let (node_totals, node_days) = load_node_traffic(&connection)?;
        Ok(Self {
            connection,
            totals,
            rate: TrafficRate::default(),
            dirty,
            last_persisted_at: now,
            node_totals,
            node_days,
            daily,
        })
    }

    fn persist_if_due(&mut self, now: Instant) -> Result<(), TrafficCounterError> {
        if now.saturating_duration_since(self.last_persisted_at) < PERSIST_INTERVAL {
            return Ok(());
        }
        self.flush()?;
        self.last_persisted_at = now;
        Ok(())
    }
}

fn load_daily_traffic(
    connection: &Connection,
) -> Result<BTreeMap<NaiveDate, u64>, TrafficCounterError> {
    let mut statement = connection.prepare("SELECT day, bytes FROM traffic_daily")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut daily = BTreeMap::new();
    for row in rows {
        let (day, bytes) = row?;
        let Ok(day) = NaiveDate::parse_from_str(&day, "%Y-%m-%d") else {
            continue;
        };
        daily.insert(day, decode_counter("bytes", bytes)?);
    }
    Ok(daily)
}

/// Every node's stored counters, with the day each one's daily figures belong to.
type StoredNodeTraffic = (HashMap<Uuid, NodeTraffic>, HashMap<Uuid, NaiveDate>);

/// Reads every node's stored counters and the day they belong to.
fn load_node_traffic(connection: &Connection) -> Result<StoredNodeTraffic, TrafficCounterError> {
    let mut statement = connection.prepare(
        "SELECT node_id, day, today_upload, today_download, total_upload, total_download
         FROM node_traffic",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;

    let mut totals = HashMap::new();
    let mut days = HashMap::new();
    for row in rows {
        let (id, day, today_upload, today_download, total_upload, total_download) = row?;
        // A row whose node id or day cannot be read is skipped rather than
        // failing the whole load: one unreadable counter must not cost the user
        // every other node's history.
        let (Ok(id), Ok(day)) = (
            Uuid::parse_str(&id),
            NaiveDate::parse_from_str(&day, "%Y-%m-%d"),
        ) else {
            continue;
        };
        totals.insert(
            id,
            NodeTraffic {
                today_upload_bytes: decode_counter("today_upload", today_upload)?,
                today_download_bytes: decode_counter("today_download", today_download)?,
                total_upload_bytes: decode_counter("total_upload", total_upload)?,
                total_download_bytes: decode_counter("total_download", total_download)?,
            },
        );
        days.insert(id, day);
    }
    Ok((totals, days))
}

fn checked_counter_add(current: u64, added: u64) -> Result<u64, TrafficCounterError> {
    current
        .checked_add(added)
        .filter(|value| *value <= MAX_TRAFFIC_BYTES)
        .ok_or(TrafficCounterError::CounterOverflow)
}

fn decode_counter(column: &'static str, value: i64) -> Result<u64, TrafficCounterError> {
    u64::try_from(value).map_err(|_| TrafficCounterError::InvalidStoredCounter { column, value })
}

/// Why aggregate traffic values could not be updated or persisted.
#[derive(Debug, Error)]
pub enum TrafficCounterError {
    #[error("the traffic counter exceeded SQLite's integer range")]
    CounterOverflow,
    #[error("traffic counter database operation failed")]
    Database {
        #[source]
        source: rusqlite::Error,
    },
    #[error("stored traffic day {value} is invalid")]
    InvalidStoredDay {
        value: String,
        #[source]
        source: chrono::ParseError,
    },
    #[error("stored traffic counter {column} is negative: {value}")]
    InvalidStoredCounter { column: &'static str, value: i64 },
}

impl From<rusqlite::Error> for TrafficCounterError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Database { source }
    }
}

#[derive(Debug, Deserialize)]
struct ApiTrafficRate {
    up: u64,
    down: u64,
}

/// Why the local Core API could not produce a live traffic sample.
#[derive(Debug, Error)]
pub enum TrafficSampleError {
    #[error("the traffic sample timeout must be greater than zero")]
    InvalidTimeout,
    #[error("the traffic sample HTTP client could not be built")]
    ClientBuild(#[source] reqwest::Error),
    #[error("the traffic sample timed out")]
    TimedOut,
    #[error("the traffic sample request failed")]
    Request(#[source] reqwest::Error),
    #[error("the traffic API returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("the traffic API closed before emitting a sample")]
    StreamEnded,
    #[error("the traffic API sample exceeded the size limit")]
    BodyTooLarge,
    #[error("the traffic API emitted an invalid sample")]
    InvalidBody(#[source] serde_json::Error),
}

/// Reads the next one-second sample from sing-box's `/traffic` stream.
///
/// # Errors
///
/// Returns a typed timeout, request, status, size, or JSON error.
pub async fn sample_traffic(
    api_address: SocketAddr,
    timeout: Duration,
) -> Result<TrafficRate, TrafficSampleError> {
    if timeout.is_zero() {
        return Err(TrafficSampleError::InvalidTimeout);
    }

    ensure_rustls_crypto_provider();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .build()
        .map_err(|source| TrafficSampleError::ClientBuild(source.without_url()))?;
    let mut response = client
        .get(format!("http://{api_address}/traffic"))
        .send()
        .await
        .map_err(map_request_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(TrafficSampleError::HttpStatus {
            status: status.as_u16(),
        });
    }

    let mut body = Vec::new();
    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(map_request_error)?
            .ok_or(TrafficSampleError::StreamEnded)?;
        body.extend_from_slice(&chunk);
        if body.len() > SAMPLE_BODY_LIMIT {
            return Err(TrafficSampleError::BodyTooLarge);
        }
        let Some(line_end) = body.iter().position(|byte| *byte == b'\n') else {
            continue;
        };
        let sample: ApiTrafficRate =
            serde_json::from_slice(&body[..line_end]).map_err(TrafficSampleError::InvalidBody)?;
        return Ok(TrafficRate {
            upload_bytes_per_second: sample.up,
            download_bytes_per_second: sample.down,
        });
    }
}

fn map_request_error(source: reqwest::Error) -> TrafficSampleError {
    if source.is_timeout() {
        TrafficSampleError::TimedOut
    } else {
        TrafficSampleError::Request(source.without_url())
    }
}
