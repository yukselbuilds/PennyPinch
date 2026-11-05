use super::normalize::{
    merge_event, validate_event, EngineProjection, NormalizedBatch, NormalizedEvent,
    MAX_OBSERVATIONS,
};
use super::query::{aggregate_event_refs, aggregate_unique_events, AggregateStatus, PeriodTotals};
use crate::integrations::config_io::{pin_existing_directory_chain, PinnedDirectoryChain};
use chrono::{DateTime, Days, Duration, Local, SecondsFormat, TimeZone, Timelike, Utc};
use rusqlite::{params, Connection, ErrorCode, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration as StdDuration;

pub const RETENTION_DAYS: u16 = 30;
const APPLICATION_ID: i64 = 0x48524d31;
const MAX_DB_BYTES: u64 = 128 * 1024 * 1024;
const MAX_EVENTS: i64 = 100_000;
const MAX_EVENT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricsError {
    InvalidInput,
    UnsafePath,
    Busy,
    UnsupportedSchema,
    Corrupt,
    Capacity,
    Unavailable,
    InvalidLocalTime,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Session,
    Today,
    SevenDays,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CaptureFlags {
    pub rolling_window: bool,
    pub aggregate_only: bool,
    pub unidentified_rows_observed: bool,
}

#[derive(Debug, Serialize)]
pub struct IngestResult {
    pub inserted: u64,
    pub replayed: u64,
    pub conflicted: u64,
    pub expired: u64,
}

#[derive(Debug, Default, Serialize)]
pub struct Coverage {
    pub events: u64,
    pub distinct_origins: u64,
    pub unplaced_events: u64,
    pub rolling_window: bool,
    pub aggregate_only: bool,
    pub unidentified_rows_observed: bool,
    pub complete_traffic: bool,
}

#[derive(Debug, Serialize)]
pub struct PeriodMetrics {
    pub monitoring_session_id: String,
    pub period: Period,
    pub from_utc: Option<String>,
    pub to_utc: String,
    pub retention_days: u16,
    pub last_event_utc: Option<String>,
    pub totals: PeriodTotals,
    pub series: Vec<MetricBucket>,
    pub coverage: Coverage,
}

#[derive(Debug, Serialize)]
pub struct MetricBucket {
    pub at_utc: String,
    pub totals: PeriodTotals,
}

#[derive(Clone)]
pub struct MetricsStore {
    root: Option<PathBuf>,
    monitoring_session_id: String,
}

impl MetricsStore {
    pub fn new(app_local_data: Option<PathBuf>) -> Self {
        #[cfg(debug_assertions)]
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|p| p.join("work/app-state/metrics"));
        #[cfg(not(debug_assertions))]
        let root = app_local_data.map(|p| p.join("metrics"));
        #[cfg(debug_assertions)]
        let _ = app_local_data;
        Self {
            root,
            monitoring_session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    #[cfg(test)]
    pub(crate) fn in_directory(root: &Path) -> Self {
        Self {
            root: Some(root.to_owned()),
            monitoring_session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub(crate) fn ingest_batch(
        &self,
        batch: &NormalizedBatch,
        flags: CaptureFlags,
        now: DateTime<Utc>,
    ) -> Result<IngestResult, MetricsError> {
        if batch.schema_version != 1 || batch.events.len() > MAX_OBSERVATIONS {
            return Err(MetricsError::InvalidInput);
        }
        let mut identities = BTreeSet::new();
        for event in &batch.events {
            validate_event(event).map_err(|_| MetricsError::InvalidInput)?;
            if !identities.insert(&event.event_id) {
                return Err(MetricsError::InvalidInput);
            }
        }
        let cutoff = retention_cutoff(now)?;
        let mut db = self.open(true)?.ok_or(MetricsError::Unavailable)?;
        let tx = db
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        let mut result = IngestResult {
            inserted: 0,
            replayed: 0,
            conflicted: 0,
            expired: prune(&tx, cutoff)?,
        };
        for incoming in &batch.events {
            let existing: Option<String> = tx
                .query_row(
                    "SELECT payload FROM events WHERE event_id=?1",
                    [&incoming.event_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql_error)?;
            let current = match existing.as_deref() {
                Some(encoded) => {
                    let mut value = decode(encoded)?;
                    if value.event_id != incoming.event_id || value.origin_id != incoming.origin_id
                    {
                        return Err(MetricsError::Corrupt);
                    }
                    result.replayed += 1;
                    if merge_event(&mut value, incoming) {
                        result.conflicted += 1;
                    }
                    value
                }
                None => {
                    if event_time(incoming)?.is_some_and(|stamp| stamp < cutoff) {
                        result.expired += 1;
                        continue;
                    }
                    result.inserted += 1;
                    incoming.clone()
                }
            };
            // Validate the combined representation too; persistence never raises provenance.
            validate_event(&current).map_err(|_| MetricsError::Corrupt)?;
            let encoded =
                serde_json::to_string(&current).map_err(|_| MetricsError::InvalidInput)?;
            if encoded.len() > MAX_EVENT_BYTES {
                return Err(MetricsError::InvalidInput);
            }
            let occurred = event_time(&current)?.map(|stamp| stamp.timestamp());
            tx.execute("INSERT INTO events(event_id, origin_id, session_id, first_seen, occurred_at, payload)
                VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(event_id) DO UPDATE SET occurred_at=excluded.occurred_at,payload=excluded.payload",
                params![current.event_id, current.origin_id, self.monitoring_session_id, now.timestamp(), occurred, encoded]).map_err(sql_error)?;
        }
        let count: i64 = tx
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .map_err(sql_error)?;
        if count > MAX_EVENTS {
            return Err(MetricsError::Capacity);
        }
        let bucket = now.timestamp().div_euclid(86400) * 86400;
        tx.execute("INSERT INTO captures(session_id,bucket_utc,rolling,aggregate_only,unidentified) VALUES(?1,?2,?3,?4,?5)
            ON CONFLICT(session_id,bucket_utc) DO UPDATE SET rolling=max(rolling,excluded.rolling),aggregate_only=max(aggregate_only,excluded.aggregate_only),unidentified=max(unidentified,excluded.unidentified)",
            params![self.monitoring_session_id, bucket, flags.rolling_window, flags.aggregate_only, flags.unidentified_rows_observed]).map_err(sql_error)?;
        tx.commit().map_err(sql_error)?;
        Ok(result)
    }

    pub fn ingest_projection(
        &self,
        projection: &EngineProjection,
        now: DateTime<Utc>,
    ) -> Result<IngestResult, MetricsError> {
        self.ingest_batch(
            &projection.batch,
            CaptureFlags {
                rolling_window: projection.rolling_window,
                aggregate_only: projection.aggregate_only,
                unidentified_rows_observed: projection.unidentified_rows > 0,
            },
            now,
        )
    }

    pub fn query_at<Tz: TimeZone>(
        &self,
        period: Period,
        now: DateTime<Tz>,
    ) -> Result<PeriodMetrics, MetricsError> {
        let zone = now.timezone();
        let from = match period {
            Period::Session => None,
            Period::Today | Period::SevenDays => {
                let date = now
                    .date_naive()
                    .checked_sub_days(Days::new(if period == Period::SevenDays { 6 } else { 0 }))
                    .ok_or(MetricsError::InvalidLocalTime)?;
                let midnight = date
                    .and_hms_opt(0, 0, 0)
                    .ok_or(MetricsError::InvalidLocalTime)?;
                Some(
                    now.timezone()
                        .from_local_datetime(&midnight)
                        .earliest()
                        .ok_or(MetricsError::InvalidLocalTime)?
                        .with_timezone(&Utc),
                )
            }
        };
        let now = now.with_timezone(&Utc);
        let cutoff = retention_cutoff(now)?;
        let mut coverage = Coverage::default();
        let mut events = Vec::new();
        let mut last_event = None;
        if let Some(mut db) = self.open(false)? {
            let tx = db
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql_error)?;
            prune(&tx, cutoff)?;
            let session =
                (period == Period::Session).then_some(self.monitoring_session_id.as_str());
            let mut statement = tx.prepare("SELECT event_id,origin_id,payload FROM events WHERE (?1 IS NULL OR session_id=?1)
                AND (occurred_at IS NULL OR (occurred_at>=?2 AND occurred_at<=?3)) ORDER BY event_id LIMIT 100001").map_err(sql_error)?;
            let rows = statement
                .query_map(
                    params![session, from.unwrap_or(cutoff).timestamp(), now.timestamp()],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )
                .map_err(sql_error)?;
            let mut origins = BTreeSet::new();
            for row in rows {
                let (id, origin, encoded) = row.map_err(sql_error)?;
                let event = decode(&encoded)?;
                if event.event_id != id || event.origin_id != origin {
                    return Err(MetricsError::Corrupt);
                }
                match event_time(&event)? {
                    Some(stamp) if stamp > now || from.is_some_and(|start| stamp < start) => {
                        continue
                    }
                    None => coverage.unplaced_events += 1,
                    Some(stamp) => {
                        last_event =
                            Some(last_event.map_or(stamp, |last: DateTime<Utc>| last.max(stamp)))
                    }
                }
                origins.insert(event.origin_id.clone());
                events.push(event);
                if events.len() > MAX_EVENTS as usize {
                    return Err(MetricsError::Capacity);
                }
            }
            drop(statement);
            coverage.events = events.len() as u64;
            coverage.distinct_origins = origins.len() as u64;
            let flags: (bool,bool,bool) = tx.query_row("SELECT coalesce(max(rolling),0),coalesce(max(aggregate_only),0),coalesce(max(unidentified),0)
                FROM captures WHERE (?1 IS NULL OR session_id=?1) AND bucket_utc>=?2 AND bucket_utc<=?3",
                params![session, from.unwrap_or(cutoff).timestamp().div_euclid(86400)*86400, now.timestamp()],
                |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(sql_error)?;
            coverage.rolling_window = flags.0;
            coverage.aggregate_only = flags.1;
            coverage.unidentified_rows_observed = flags.2;
            tx.commit().map_err(sql_error)?;
        }
        let totals = aggregate_unique_events(&events);
        let series = if totals.status == AggregateStatus::Measured {
            build_series(&events, period, now, from, &zone)?
        } else {
            Vec::new()
        };
        Ok(PeriodMetrics {
            monitoring_session_id: self.monitoring_session_id.clone(),
            period,
            from_utc: from.map(utc_string),
            to_utc: utc_string(now),
            retention_days: RETENTION_DAYS,
            last_event_utc: last_event.map(utc_string),
            totals,
            series,
            coverage,
        })
    }

    pub fn query(&self, request: MetricsQueryRequest) -> MetricsReply {
        if !request.is_valid() {
            return Self::failure(request.request_id, MetricsError::InvalidInput);
        }
        match self.query_at(request.period, Local::now()) {
            Ok(data) => MetricsReply::Snapshot(MetricsSnapshot {
                schema_version: 1,
                request_id: request.request_id,
                status: "ok",
                data,
            }),
            Err(error) => Self::failure(request.request_id, error),
        }
    }

    pub fn failure(request_id: String, code: MetricsError) -> MetricsReply {
        MetricsReply::Error(Self::error_detail(request_id, code))
    }

    pub(crate) fn error_detail(request_id: String, code: MetricsError) -> MetricsFailure {
        let safe_id = uuid::Uuid::parse_str(&request_id)
            .unwrap_or(uuid::Uuid::nil())
            .to_string();
        MetricsFailure {
            schema_version: 1,
            request_id: safe_id,
            status: "error",
            code,
            retryable: matches!(code, MetricsError::Busy | MetricsError::Unavailable),
        }
    }

    fn open(&self, create: bool) -> Result<Option<Database>, MetricsError> {
        let root = self.root.as_ref().ok_or(MetricsError::Unavailable)?;
        let Some(directories) = prepare_root(root, create)? else {
            return Ok(None);
        };
        let path = root.join("metrics.sqlite3");
        let mut created = false;
        let file = match open_pinned_file(&path, false) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                match open_pinned_file(&path, true) {
                    Ok(file) => {
                        created = true;
                        file
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        open_pinned_file(&path, false).map_err(|_| MetricsError::Unavailable)?
                    }
                    Err(_) => return Err(MetricsError::Unavailable),
                }
            }
            Err(_) => return Err(MetricsError::UnsafePath),
        };
        validate_file(&file)?;
        for suffix in ["-journal", "-wal", "-shm"] {
            let sidecar = root.join(format!("metrics.sqlite3{suffix}"));
            match open_pinned_file(&sidecar, false) {
                Ok(file) => validate_file(&file)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(MetricsError::UnsafePath),
            }
        }
        directories
            .validate()
            .map_err(|_| MetricsError::UnsafePath)?;
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(sql_error)?;
        connection
            .busy_timeout(StdDuration::from_millis(250))
            .map_err(sql_error)?;
        let application: i64 = connection
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(sql_error)?;
        let schema: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(sql_error)?;
        if created {
            if application != 0 || schema != 0 {
                return Err(MetricsError::UnsupportedSchema);
            }
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql_error)?;
            tx.pragma_update(None, "application_id", APPLICATION_ID)
                .map_err(sql_error)?;
            tx.pragma_update(None, "user_version", 1)
                .map_err(sql_error)?;
            tx.execute_batch("CREATE TABLE events(event_id TEXT PRIMARY KEY,origin_id TEXT NOT NULL,session_id TEXT NOT NULL,first_seen INTEGER NOT NULL,occurred_at INTEGER,payload TEXT NOT NULL CHECK(length(payload)<=4096));
                CREATE INDEX events_time ON events(occurred_at); CREATE INDEX events_session ON events(session_id);
                CREATE TABLE captures(session_id TEXT NOT NULL,bucket_utc INTEGER NOT NULL,rolling INTEGER NOT NULL,aggregate_only INTEGER NOT NULL,unidentified INTEGER NOT NULL,PRIMARY KEY(session_id,bucket_utc));").map_err(sql_error)?;
            tx.commit().map_err(sql_error)?;
        } else if application != APPLICATION_ID || schema != 1 {
            return Err(MetricsError::UnsupportedSchema);
        }
        connection
            .execute_batch(
                "PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;",
            )
            .map_err(sql_error)?;
        let page_size: i64 = connection
            .pragma_query_value(None, "page_size", |r| r.get(0))
            .map_err(sql_error)?;
        if !(512..=65536).contains(&page_size) || !(page_size as u64).is_power_of_two() {
            return Err(MetricsError::Corrupt);
        }
        connection
            .pragma_update(
                None,
                "max_page_count",
                (MAX_DB_BYTES / page_size as u64) as i64,
            )
            .map_err(sql_error)?;
        Ok(Some(Database {
            connection,
            _file: file,
            _directories: directories,
        }))
    }
}

struct Database {
    connection: Connection,
    _file: File,
    _directories: PinnedDirectoryChain,
}

fn utc_string(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn build_series<Tz: TimeZone>(
    events: &[NormalizedEvent],
    period: Period,
    now: DateTime<Utc>,
    from: Option<DateTime<Utc>>,
    zone: &Tz,
) -> Result<Vec<MetricBucket>, MetricsError> {
    let mut timed = Vec::new();
    for event in events.iter().filter(|event| event.main_metric_eligible) {
        timed.push((event, event_time(event)?.ok_or(MetricsError::Corrupt)?));
    }
    let first = timed
        .iter()
        .map(|(_, stamp)| *stamp)
        .min()
        .ok_or(MetricsError::Corrupt)?;
    let hourly = period == Period::Today
        || (period == Period::Session && now.signed_duration_since(first) <= Duration::days(1));
    let anchor = from.unwrap_or(
        first
            .date_naive()
            .and_hms_opt(first.hour(), 0, 0)
            .ok_or(MetricsError::InvalidLocalTime)?
            .and_utc(),
    );
    let mut buckets: BTreeMap<DateTime<Utc>, Vec<&NormalizedEvent>> = BTreeMap::new();
    for (event, stamp) in timed {
        let at = if hourly {
            anchor + Duration::hours(stamp.signed_duration_since(anchor).num_hours())
        } else {
            let date = stamp
                .with_timezone(zone)
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .ok_or(MetricsError::InvalidLocalTime)?;
            zone.from_local_datetime(&date)
                .earliest()
                .ok_or(MetricsError::InvalidLocalTime)?
                .with_timezone(&Utc)
        };
        buckets.entry(at).or_default().push(event);
    }
    if buckets.len() > 64 {
        return Err(MetricsError::Capacity);
    }
    Ok(buckets
        .into_iter()
        .map(|(at, events)| MetricBucket {
            at_utc: utc_string(at),
            totals: aggregate_event_refs(events),
        })
        .collect())
}

fn event_time(value: &NormalizedEvent) -> Result<Option<DateTime<Utc>>, MetricsError> {
    value
        .timestamp_utc
        .as_deref()
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&Utc))
                .map_err(|_| MetricsError::Corrupt)
        })
        .transpose()
}

fn retention_cutoff(now: DateTime<Utc>) -> Result<DateTime<Utc>, MetricsError> {
    now.checked_sub_signed(Duration::days(RETENTION_DAYS as i64))
        .ok_or(MetricsError::InvalidInput)
}

fn prune(tx: &rusqlite::Transaction<'_>, cutoff: DateTime<Utc>) -> Result<u64, MetricsError> {
    let expired = tx
        .execute(
            "DELETE FROM events WHERE min(coalesce(occurred_at,first_seen),first_seen) < ?1",
            [cutoff.timestamp()],
        )
        .map_err(sql_error)?;
    tx.execute(
        "DELETE FROM captures WHERE bucket_utc+86400 <= ?1",
        [cutoff.timestamp()],
    )
    .map_err(sql_error)?;
    Ok(expired as u64)
}

fn decode(encoded: &str) -> Result<NormalizedEvent, MetricsError> {
    if encoded.len() > MAX_EVENT_BYTES {
        return Err(MetricsError::Corrupt);
    }
    let value: NormalizedEvent =
        serde_json::from_str(encoded).map_err(|_| MetricsError::Corrupt)?;
    validate_event(&value).map_err(|_| MetricsError::Corrupt)?;
    Ok(value)
}

fn sql_error(error: rusqlite::Error) -> MetricsError {
    match error {
        rusqlite::Error::SqliteFailure(code, _) => match code.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => MetricsError::Busy,
            ErrorCode::DiskFull => MetricsError::Capacity,
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => MetricsError::Corrupt,
            _ => MetricsError::Unavailable,
        },
        _ => MetricsError::Corrupt,
    }
}

fn prepare_root(root: &Path, create: bool) -> Result<Option<PinnedDirectoryChain>, MetricsError> {
    if !root.is_absolute()
        || root.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(MetricsError::UnsafePath);
    }
    let mut ancestor = root.to_owned();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or(MetricsError::UnsafePath)?
                        .to_owned(),
                );
                ancestor = ancestor
                    .parent()
                    .ok_or(MetricsError::UnsafePath)?
                    .to_owned();
            }
            Err(_) => return Err(MetricsError::UnsafePath),
        }
    }
    let mut pin = pin_existing_directory_chain(&ancestor).map_err(|_| MetricsError::UnsafePath)?;
    if !missing.is_empty() && !create {
        return Ok(None);
    }
    for component in missing.into_iter().rev() {
        if component.to_string_lossy().contains(':') {
            return Err(MetricsError::UnsafePath);
        }
        ancestor.push(component);
        match fs::create_dir(&ancestor) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(MetricsError::Unavailable),
        }
        let next = pin_existing_directory_chain(&ancestor).map_err(|_| MetricsError::UnsafePath)?;
        pin.validate().map_err(|_| MetricsError::UnsafePath)?;
        pin = next;
    }
    pin.validate().map_err(|_| MetricsError::UnsafePath)?;
    Ok(Some(pin))
}

fn open_pinned_file(path: &Path, create: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(create);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

fn validate_file(file: &File) -> Result<(), MetricsError> {
    let metadata = file.metadata().map_err(|_| MetricsError::UnsafePath)?;
    if !metadata.is_file() {
        return Err(MetricsError::UnsafePath);
    }
    if metadata.len() > MAX_DB_BYTES {
        return Err(MetricsError::Capacity);
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
        };
        let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut information) } == 0
            || information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || information.nNumberOfLinks != 1
        {
            return Err(MetricsError::UnsafePath);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(MetricsError::UnsafePath);
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsQueryRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub period: Period,
}

impl MetricsQueryRequest {
    pub(crate) fn is_valid(&self) -> bool {
        self.schema_version == 1
            && uuid::Uuid::parse_str(&self.request_id)
                .is_ok_and(|id| !id.is_nil() && id.to_string() == self.request_id)
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum MetricsReply {
    Snapshot(MetricsSnapshot),
    Error(MetricsFailure),
}

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub data: PeriodMetrics,
}

#[derive(Debug, Serialize)]
pub struct MetricsFailure {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub code: MetricsError,
    pub retryable: bool,
}
