use super::{
    accounting::TrafficBytes,
    collector::{StoredDiagnostic, StoredEntry, StoredHistory, StoredHour, StoredSlices},
};
use anyhow::{Context as _, Result};
use rusqlite::{Connection, OptionalExtension as _, params, params_from_iter};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadDetail {
    Totals,
    Hours,
    Fine,
    Diagnostics,
    Resume,
}

impl ReadDetail {
    pub(super) const fn hours(self) -> bool {
        matches!(self, Self::Hours | Self::Fine | Self::Resume)
    }
    pub(super) const fn slices(self) -> bool {
        matches!(self, Self::Fine | Self::Resume)
    }
    pub(super) const fn diagnostics(self) -> bool {
        matches!(self, Self::Diagnostics | Self::Resume)
    }
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageInfo {
    pub path: String,
    pub database_bytes: u64,
    pub wal_bytes: u64,
    pub reusable_bytes: u64,
    pub daily_rows: u64,
    pub sample_rows: u64,
    pub recorded_days: u64,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    pub last_saved_at: Option<i64>,
    pub integrity: Option<String>,
}

fn query_filter(column: &str, first: &str, last: &str, app: Option<&str>, path: Option<&str>) -> (String, Vec<String>) {
    let mut clause = format!("{column}>=?1 AND {column}<=?2");
    let mut values = vec![first.to_owned(), last.to_owned()];
    for (column, value) in [("app", app), ("path", path)] {
        if let Some(value) = value {
            values.push(value.to_owned());
            clause.push_str(&format!(" AND {column}=?{}", values.len()));
        }
    }
    (clause, values)
}

fn sql_bytes(value: u64) -> Result<i64> {
    i64::try_from(value).context("Traffic counter exceeds SQLite integer range")
}
fn read_bytes(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
fn read_duration(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    let value: Option<i64> = row.get(index)?;
    value
        .map(|value| u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value)))
        .transpose()
}
pub(crate) struct Database {
    connection: Connection,
    path: PathBuf,
}
static ANCHOR: OnceLock<parking_lot::Mutex<Connection>> = OnceLock::new();
static INITIALIZE: std::sync::LazyLock<parking_lot::Mutex<()>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(()));

pub(crate) fn keep_open(path: &Path) -> Result<()> {
    if ANCHOR.get().is_none() {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        // SQLite opens its WAL lazily; a connection without a read does not keep it open.
        connection.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get::<_, u32>(0))?;
        let _ = ANCHOR.set(parking_lot::Mutex::new(connection));
    }
    Ok(())
}

impl Database {
    fn filter(
        &self,
        column: &str,
        first: &str,
        last: &str,
        app: Option<&str>,
        path: Option<&str>,
    ) -> Result<(String, Vec<String>)> {
        let (Some(app), Some(path)) = (app, path) else {
            return Ok(query_filter(column, first, last, app, path));
        };
        let identity = super::process::application_identity(app, path);
        let mut members = std::collections::BTreeSet::from([(app.to_owned(), path.to_owned())]);
        let mut statement = self
            .connection
            .prepare("SELECT DISTINCT app,path FROM daily WHERE day>=?1 AND day<=?2")?;
        for row in statement.query_map([&first[..10], &last[..10]], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })? {
            let member = row?;
            if super::process::application_identity(&member.0, &member.1) == identity {
                members.insert(member);
            }
        }
        let (mut filter, mut values) = query_filter(column, first, last, None, None);
        let mut pairs = Vec::new();
        for (app, path) in members {
            pairs.push(format!("(?{},?{})", values.len() + 1, values.len() + 2));
            values.push(app);
            values.push(path);
        }
        filter.push_str(&format!(" AND (app,path) IN (VALUES {})", pairs.join(",")));
        Ok((filter, values))
    }

    pub(crate) fn open(path: &Path) -> Result<Self> {
        let _initialization = INITIALIZE.lock();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "journal_size_limit", 16 * 1024 * 1024)?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        anyhow::ensure!(version <= 1, "Unsupported traffic database schema: {version}");
        if version == 0 {
            let tables: u32 = connection.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )?;
            anyhow::ensure!(tables == 0, "The file contains another database; it was preserved");
            connection.execute_batch("BEGIN IMMEDIATE;
                CREATE TABLE daily(day TEXT NOT NULL, app TEXT NOT NULL, path TEXT NOT NULL, chain TEXT NOT NULL, upload INTEGER NOT NULL, download INTEGER NOT NULL, duration_ms INTEGER, incomplete INTEGER NOT NULL, PRIMARY KEY(day,app,path,chain)) WITHOUT ROWID;
                CREATE TABLE hourly(hour TEXT NOT NULL, app TEXT NOT NULL, path TEXT NOT NULL, chain TEXT NOT NULL, upload INTEGER NOT NULL, download INTEGER NOT NULL, PRIMARY KEY(hour,app,path,chain)) WITHOUT ROWID;
                CREATE TABLE slices(day TEXT NOT NULL, slot INTEGER NOT NULL CHECK(slot>=0 AND slot<17280), app TEXT NOT NULL, path TEXT NOT NULL, chain TEXT NOT NULL, upload INTEGER NOT NULL, download INTEGER NOT NULL, PRIMARY KEY(day,app,path,chain,slot)) WITHOUT ROWID;
                CREATE INDEX slices_app ON slices(app,path,day,slot);
                CREATE INDEX daily_app ON daily(app,path,day);
                CREATE INDEX hourly_app ON hourly(app,path,hour);
                CREATE TABLE diagnostics(day TEXT NOT NULL, app TEXT NOT NULL, path TEXT NOT NULL, chain TEXT NOT NULL, destination TEXT NOT NULL, network TEXT NOT NULL, reason TEXT NOT NULL, last_seen TEXT NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(day,app,path,chain,destination,network,reason)) WITHOUT ROWID;
                CREATE INDEX diagnostics_app ON diagnostics(app,path,day,last_seen);
                CREATE TABLE checkpoint(id TEXT PRIMARY KEY, upload INTEGER NOT NULL, download INTEGER NOT NULL) WITHOUT ROWID;
                CREATE TABLE metadata(key TEXT PRIMARY KEY, value INTEGER NOT NULL) WITHOUT ROWID;
                PRAGMA application_id=1129731154;
                PRAGMA user_version=1;
                COMMIT;")?;
        }
        let id: u32 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
        anyhow::ensure!(id == 1129731154, "This is not a traffic database");
        let has_activity: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='activity' AND type='table')",
            [],
            |row| row.get(0),
        )?;
        if !has_activity {
            use chrono::TimeZone as _;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch("CREATE TABLE activity(day TEXT NOT NULL,app TEXT NOT NULL,path TEXT NOT NULL,chain TEXT NOT NULL,active_at INTEGER NOT NULL,PRIMARY KEY(day,app,path,chain)) WITHOUT ROWID;")?;
            let day = chrono::Local::now().date_naive();
            let rows = transaction.prepare("SELECT app,path,chain,max(slot) FROM slices WHERE day=?1 AND (upload>0 OR download>0) GROUP BY app,path,chain")?.query_map([day.format("%Y-%m-%d").to_string()], |row| Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?, row.get::<_,u32>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            for (app, path, chain, slot) in rows {
                let time = day.and_hms_opt(0, 0, 0).context("Invalid activity day")?
                    + chrono::Duration::seconds(i64::from(slot) * 5);
                if let Some(time) = chrono::Local.from_local_datetime(&time).earliest() {
                    transaction.execute(
                        "INSERT INTO activity VALUES(?1,?2,?3,?4,?5)",
                        params![
                            day.format("%Y-%m-%d").to_string(),
                            app,
                            path,
                            chain,
                            time.timestamp_millis()
                        ],
                    )?;
                }
            }
            transaction.commit()?;
        }
        Ok(Self {
            connection,
            path: path.to_owned(),
        })
    }

    pub(super) fn save(
        &mut self,
        history: &StoredHistory,
        checkpoint: &(bool, Vec<(String, TrafficBytes)>),
        clear: Option<Option<&str>>,
    ) -> Result<()> {
        let transaction = self.connection.transaction()?;
        if let Some(day) = clear {
            for table in ["daily", "slices", "diagnostics", "activity"] {
                transaction.execute(&format!("DELETE FROM {table} WHERE (?1 IS NULL OR day=?1)"), [day])?;
            }
            transaction.execute("DELETE FROM hourly WHERE (?1 IS NULL OR substr(hour,1,10)=?1)", [day])?;
        }
        {
            let mut daily = transaction.prepare_cached("INSERT INTO daily VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(day,app,path,chain) DO UPDATE SET upload=excluded.upload,download=excluded.download,duration_ms=excluded.duration_ms,incomplete=excluded.incomplete WHERE daily.upload IS NOT excluded.upload OR daily.download IS NOT excluded.download OR daily.duration_ms IS NOT excluded.duration_ms OR daily.incomplete IS NOT excluded.incomplete")?;
            for row in &history.entries {
                if let Some(at) = row.last_activity_at {
                    transaction.execute("INSERT INTO activity VALUES(?1,?2,?3,?4,?5) ON CONFLICT(day,app,path,chain) DO UPDATE SET active_at=max(activity.active_at,excluded.active_at)", params![row.day,row.app,row.path,row.chain,at])?;
                }
                daily.execute(params![
                    row.day,
                    row.app,
                    row.path,
                    row.chain,
                    sql_bytes(row.upload)?,
                    sql_bytes(row.download)?,
                    row.duration_ms.map(i64::try_from).transpose()?,
                    row.duration_incomplete
                ])?;
            }
            let mut hourly = transaction.prepare_cached("INSERT INTO hourly VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(hour,app,path,chain) DO UPDATE SET upload=excluded.upload,download=excluded.download WHERE hourly.upload IS NOT excluded.upload OR hourly.download IS NOT excluded.download")?;
            for row in &history.hours {
                hourly.execute(params![
                    row.hour,
                    row.app,
                    row.path,
                    row.chain,
                    sql_bytes(row.upload)?,
                    sql_bytes(row.download)?
                ])?;
            }
            let mut slices = transaction.prepare_cached("INSERT INTO slices VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(day,app,path,chain,slot) DO UPDATE SET upload=excluded.upload,download=excluded.download WHERE slices.upload IS NOT excluded.upload OR slices.download IS NOT excluded.download")?;
            for row in &history.slices {
                for (slot, upload, download) in &row.values {
                    slices.execute(params![
                        row.day,
                        slot,
                        row.app,
                        row.path,
                        row.chain,
                        sql_bytes(*upload)?,
                        sql_bytes(*download)?
                    ])?;
                }
            }
            let mut diagnostics = transaction.prepare_cached("INSERT INTO diagnostics VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(day,app,path,chain,destination,network,reason) DO UPDATE SET last_seen=excluded.last_seen,payload=excluded.payload WHERE diagnostics.payload IS NOT excluded.payload")?;
            let mut groups = std::collections::BTreeSet::new();
            for row in &history.diagnostics {
                diagnostics.execute(params![
                    row.day,
                    row.app,
                    row.path,
                    row.chain,
                    row.record.destination,
                    row.record.network,
                    row.record.reason,
                    row.record.last_seen,
                    serde_json::to_string(&row.record)?
                ])?;
                groups.insert((&row.day, &row.app, &row.path, &row.chain));
            }
            for (day, app, path, chain) in groups {
                transaction.execute("DELETE FROM diagnostics WHERE day=?1 AND app=?2 AND path=?3 AND chain=?4 AND (destination,network,reason) NOT IN (SELECT destination,network,reason FROM diagnostics WHERE day=?1 AND app=?2 AND path=?3 AND chain=?4 ORDER BY last_seen DESC,destination DESC LIMIT 32)", params![day,app,path,chain])?;
            }
            transaction.execute("DELETE FROM checkpoint", [])?;
            let mut previous = transaction.prepare_cached("INSERT INTO checkpoint VALUES(?1,?2,?3)")?;
            for (id, total) in &checkpoint.1 {
                previous.execute(params![id, sql_bytes(total.upload)?, sql_bytes(total.download)?])?;
            }
            transaction.execute(
                "INSERT INTO metadata VALUES('initialized',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [checkpoint.0],
            )?;
            transaction.execute(
                "INSERT INTO metadata VALUES('saved_at',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [chrono::Utc::now().timestamp_millis()],
            )?;
        }
        transaction
            .commit()
            .context("Could not commit traffic counters and checkpoint")
    }

    pub(crate) fn checkpoint(&self) -> Result<(bool, Vec<(String, TrafficBytes)>)> {
        let initialized = self
            .connection
            .query_row("SELECT value FROM metadata WHERE key='initialized'", [], |row| {
                row.get::<_, bool>(0)
            })
            .unwrap_or(false);
        let values = self
            .connection
            .prepare("SELECT id,upload,download FROM checkpoint")?
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    TrafficBytes {
                        upload: read_bytes(row, 1)?,
                        download: read_bytes(row, 2)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((initialized, values))
    }

    pub(super) fn read(
        &self,
        first: &str,
        last: &str,
        app: Option<&str>,
        path: Option<&str>,
        details: ReadDetail,
    ) -> Result<StoredHistory> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut history = StoredHistory {
            version: 2,
            entries: Vec::new(),
            hours: Vec::new(),
            slices: Vec::new(),
            diagnostics: Vec::new(),
        };
        let (filter, values) = self.filter("day", first, last, app, path)?;
        if details != ReadDetail::Diagnostics {
            history.entries = self
                .connection
                .prepare(&format!(
                    "SELECT day,app,path,chain,upload,download,duration_ms,incomplete,(SELECT active_at FROM activity WHERE activity.day=daily.day AND activity.app=daily.app AND activity.path=daily.path AND activity.chain=daily.chain) FROM daily WHERE {filter}"
                ))?
                .query_map(params_from_iter(&values), |row| {
                    Ok(StoredEntry {
                        day: row.get(0)?,
                        app: row.get(1)?,
                        path: row.get(2)?,
                        chain: row.get(3)?,
                        upload: read_bytes(row, 4)?,
                        download: read_bytes(row, 5)?,
                        duration_ms: read_duration(row, 6)?,
                        duration_incomplete: row.get(7)?,
                        last_activity_at: row.get(8)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
        }
        if details.hours() {
            let (hour_filter, hour_values) =
                self.filter("hour", &format!("{first}T00"), &format!("{last}T99"), app, path)?;
            history.hours = self
                .connection
                .prepare(&format!(
                    "SELECT hour,app,path,chain,upload,download FROM hourly WHERE {hour_filter}"
                ))?
                .query_map(params_from_iter(&hour_values), |row| {
                    Ok(StoredHour {
                        hour: row.get(0)?,
                        app: row.get(1)?,
                        path: row.get(2)?,
                        chain: row.get(3)?,
                        upload: read_bytes(row, 4)?,
                        download: read_bytes(row, 5)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
        }
        if details.slices() {
            let selection = if details == ReadDetail::Resume {
                std::borrow::Cow::Owned(format!(
                    "(day,app,path,chain,slot) IN (SELECT day,app,path,chain,MAX(slot) FROM slices WHERE {filter} GROUP BY day,app,path,chain)"
                ))
            } else {
                std::borrow::Cow::Borrowed(filter.as_str())
            };
            let mut statement = self.connection.prepare(&format!(
                "SELECT day,app,path,chain,slot,upload,download FROM slices WHERE {selection}"
            ))?;
            let rows = statement.query_map(params_from_iter(&values), |row| {
                Ok((
                    (
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ),
                    (row.get::<_, u16>(4)?, read_bytes(row, 5)?, read_bytes(row, 6)?),
                ))
            })?;
            let mut groups = BTreeMap::<_, Vec<_>>::new();
            for row in rows {
                let (key, value) = row?;
                groups.entry(key).or_default().push(value);
            }
            history.slices = groups
                .into_iter()
                .map(|((day, app, path, chain), values)| StoredSlices {
                    day,
                    app,
                    path,
                    chain,
                    values,
                })
                .collect();
        }
        if details.diagnostics() {
            let rows = self
                .connection
                .prepare(&format!(
                    "SELECT day,app,path,chain,payload FROM diagnostics WHERE {filter} ORDER BY last_seen DESC"
                ))?
                .query_map(params_from_iter(&values), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (day, app, path, chain, payload) in rows {
                history.diagnostics.push(StoredDiagnostic {
                    day,
                    app,
                    path,
                    chain,
                    record: serde_json::from_str(&payload)?,
                });
            }
        }
        transaction.commit()?;
        Ok(history)
    }

    pub(crate) fn storage_info(&self, check_integrity: bool) -> Result<StorageInfo> {
        let transaction = self.connection.unchecked_transaction()?;
        let (first_day, last_day, recorded_days, daily_rows) = self.connection.query_row(
            "SELECT min(day),max(day),count(DISTINCT day),count(*) FROM daily",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, read_bytes(row, 2)?, read_bytes(row, 3)?)),
        )?;
        let sample_rows = self
            .connection
            .query_row("SELECT count(*) FROM slices", [], |row| read_bytes(row, 0))?;
        let last_saved_at = self
            .connection
            .query_row("SELECT value FROM metadata WHERE key='saved_at'", [], |row| row.get(0))
            .optional()?;
        let page_size: u64 = self
            .connection
            .pragma_query_value(None, "page_size", |row| read_bytes(row, 0))?;
        let free_pages: u64 = self
            .connection
            .pragma_query_value(None, "freelist_count", |row| read_bytes(row, 0))?;
        let integrity = if check_integrity {
            Some(
                self.connection
                    .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?,
            )
        } else {
            None
        };
        transaction.commit()?;
        Ok(StorageInfo {
            path: self.path.to_string_lossy().into_owned(),
            database_bytes: std::fs::metadata(&self.path)?.len(),
            wal_bytes: std::fs::metadata(format!("{}-wal", self.path.display())).map_or(0, |meta| meta.len()),
            reusable_bytes: page_size.saturating_mul(free_pages),
            daily_rows,
            sample_rows,
            recorded_days,
            first_day,
            last_day,
            last_saved_at,
            integrity,
        })
    }

    pub(crate) fn compact(&self) -> Result<()> {
        self.connection.execute_batch("VACUUM; PRAGMA optimize;")?;
        // Readers may postpone truncation; the committed database remains valid either way.
        self.connection
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    }

    pub(crate) fn backup(&self, destination: &Path) -> Result<()> {
        let target = if destination.exists() {
            destination.canonicalize()?
        } else {
            destination
                .parent()
                .context("Backup directory is missing")?
                .canonicalize()?
                .join(destination.file_name().context("Backup filename is missing")?)
        };
        anyhow::ensure!(
            !self
                .path
                .canonicalize()?
                .to_string_lossy()
                .eq_ignore_ascii_case(&target.to_string_lossy()),
            "The backup must be separate from the active database"
        );
        let mut destination = Connection::open(destination)?;
        let backup = rusqlite::backup::Backup::new(&self.connection, &mut destination)?;
        backup.run_to_completion(128, Duration::from_millis(5), None)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TemporaryDatabase(std::path::PathBuf);
    impl TemporaryDatabase {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(format!("clash-traffic-db-{nonce}"));
            std::fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }
        fn path(&self) -> std::path::PathBuf {
            self.0.join("history.sqlite3")
        }
    }
    impl Drop for TemporaryDatabase {
        fn drop(&mut self) {
            if self.0.parent() == Some(std::env::temp_dir().as_path()) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
    fn history(upload: u64, slot: u16) -> StoredHistory {
        StoredHistory {
            version: 2,
            entries: vec![StoredEntry {
                day: "2026-10-30".into(),
                app: "app.exe".into(),
                path: "C:\\App\\app.exe".into(),
                chain: "Proxy / JP".into(),
                upload,
                download: upload * 2,
                duration_ms: Some(5000),
                duration_incomplete: false,
                last_activity_at: None,
            }],
            hours: vec![StoredHour {
                hour: "2026-10-30T06".into(),
                app: "app.exe".into(),
                path: "C:\\App\\app.exe".into(),
                chain: "Proxy / JP".into(),
                upload,
                download: upload * 2,
            }],
            slices: vec![StoredSlices {
                day: "2026-10-30".into(),
                app: "app.exe".into(),
                path: "C:\\App\\app.exe".into(),
                chain: "Proxy / JP".into(),
                values: vec![(slot, upload, upload * 2)],
            }],
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn activity_is_atomic_and_v8_records_seed_the_recent_activity_table() {
        use chrono::TimeZone as _;
        let temporary = TemporaryDatabase::new();
        let mut db = Database::open(&temporary.path()).unwrap();
        let day = chrono::Local::now().date_naive();
        let mut records = history(20, 12345);
        records.entries[0].day = day.to_string();
        records.hours[0].hour = format!("{day}T17");
        records.slices[0].day = day.to_string();
        records.entries[0].last_activity_at = Some(123456789);
        db.save(&records, &(true, vec![]), None).unwrap();
        let read = |db: &Database| {
            db.read(&day.to_string(), &day.to_string(), None, None, ReadDetail::Totals)
                .unwrap()
                .entries[0]
                .last_activity_at
        };
        assert_eq!(read(&db), Some(123456789));
        records.entries[0].last_activity_at = Some(999999999);
        records.slices[0].values[0].0 = 17280;
        assert!(db.save(&records, &(true, vec![]), None).is_err());
        assert_eq!(read(&db), Some(123456789));
        db.connection.execute_batch("DROP TABLE activity;").unwrap();
        drop(db);
        let mut reopened = Database::open(&temporary.path()).unwrap();
        let at = day.and_hms_opt(0, 0, 0).unwrap() + chrono::Duration::seconds(12345 * 5);
        assert_eq!(
            read(&reopened),
            chrono::Local
                .from_local_datetime(&at)
                .earliest()
                .map(|time| time.timestamp_millis())
        );
        reopened
            .save(
                &StoredHistory {
                    version: 2,
                    entries: vec![],
                    hours: vec![],
                    slices: vec![],
                    diagnostics: vec![],
                },
                &(true, vec![]),
                Some(None),
            )
            .unwrap();
        assert_eq!(
            reopened
                .connection
                .query_row("SELECT count(*) FROM activity", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn indexed_family_queries_keep_versions_and_avoid_loading_unneeded_samples() {
        let temporary = TemporaryDatabase::new();
        let mut db = Database::open(&temporary.path()).unwrap();
        let first = r"C:\Users\tester\OpenAI\Codex\bin\0ab0e7a8f3779036\codex.exe";
        let second = r"C:\Users\tester\OpenAI\Codex\bin\22cfb1d302613eae\codex.exe";
        for path in [first, second, r"D:\Other\codex.exe"] {
            let mut records = history(20, 11);
            records.entries[0].app = "codex.exe".into();
            records.entries[0].path = path.into();
            records.hours[0].app = "codex.exe".into();
            records.hours[0].path = path.into();
            records.slices[0].app = "codex.exe".into();
            records.slices[0].path = path.into();
            records.slices[0].values.insert(0, (10, 0, 0));
            db.save(&records, &(true, vec![]), None).unwrap();
        }
        let read = |detail| {
            db.read("2026-10-30", "2026-10-30", Some("codex.exe"), Some(second), detail)
                .unwrap()
        };
        let fine = read(ReadDetail::Fine);
        assert_eq!(fine.entries.len(), 2);
        assert_eq!(fine.slices.iter().map(|group| group.values.len()).sum::<usize>(), 4);
        assert!(read(ReadDetail::Hours).slices.is_empty());
        assert!(read(ReadDetail::Diagnostics).entries.is_empty());
        assert!(read(ReadDetail::Diagnostics).slices.is_empty());
        let resume = read(ReadDetail::Resume);
        assert!(resume.slices.iter().all(|group| group.values == vec![(11, 20, 40)]));
        let (filter, parameters) = db
            .filter("day", "2026-10-01", "2026-10-31", Some("codex.exe"), Some(second))
            .unwrap();
        let plan = db
            .connection
            .prepare(&format!("EXPLAIN QUERY PLAN SELECT * FROM slices WHERE {filter}"))
            .unwrap()
            .query_map(params_from_iter(&parameters), |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
            .join(" ");
        assert!(plan.contains("app=? AND path=?"), "{plan}");
        let info = db.storage_info(true).unwrap();
        assert_eq!((info.recorded_days, info.sample_rows, info.daily_rows), (1, 6, 3));
        assert_eq!(info.integrity.as_deref(), Some("ok"));
        assert!(info.last_saved_at.is_some_and(|time| time > 0));
        db.save(
            &StoredHistory {
                version: 2,
                entries: vec![],
                hours: vec![],
                slices: vec![],
                diagnostics: vec![],
            },
            &(true, vec![]),
            Some(None),
        )
        .unwrap();
        db.compact().unwrap();
        assert_eq!(db.storage_info(true).unwrap().sample_rows, 0);
        assert!(db.checkpoint().unwrap().0);
    }
    #[test]
    fn sqlite_commits_counters_with_checkpoint_rolls_back_failure_and_backs_up() {
        let temporary = TemporaryDatabase::new();
        let mut db = Database::open(&temporary.path()).unwrap();
        assert!(rusqlite::version_number() >= 3_051_003);
        assert_eq!(
            db.connection
                .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        let baseline = (
            true,
            vec![(
                "connection-a".into(),
                TrafficBytes {
                    upload: 100,
                    download: 200,
                },
            )],
        );
        db.save(&history(100, 4320), &baseline, None).unwrap();
        db.save(&history(100, 4320), &baseline, None).unwrap();
        assert!(db.save(&history(999, 17280), &(true, vec![]), None).is_err());
        let restored = db
            .read("2026-10-30", "2026-10-30", None, None, ReadDetail::Fine)
            .unwrap();
        assert_eq!(restored.entries[0].upload, 100);
        assert_eq!(restored.slices[0].values, vec![(4320, 100, 200)]);
        assert_eq!(db.checkpoint().unwrap().1[0].1.upload, 100);
        let backup = temporary.0.join("backup.sqlite3");
        std::fs::create_dir(temporary.0.join("nested")).unwrap();
        assert!(db.backup(&temporary.0.join("nested/../history.sqlite3")).is_err());
        db.backup(&backup).unwrap();
        let copy = Database::open(&backup).unwrap();
        assert_eq!(
            copy.read("2026-10-30", "2026-10-30", None, None, ReadDetail::Fine)
                .unwrap()
                .entries[0]
                .upload,
            100
        );
        drop(copy);
        drop(db);
        let mut reopened = Database::open(&temporary.path()).unwrap();
        let (initialized, checkpoint) = reopened.checkpoint().unwrap();
        let group = super::super::accounting::TrafficGroup {
            app: "app.exe".into(),
            path: "C:\\App\\app.exe".into(),
            chain: "Proxy / JP".into(),
        };
        let key = super::super::accounting::DailyKey {
            day: "2026-10-30".into(),
            group,
        };
        let mut accounting = super::super::accounting::TrafficAccounting::from_totals(
            [(
                key.clone(),
                TrafficBytes {
                    upload: 100,
                    download: 200,
                },
            )]
            .into_iter()
            .collect(),
        );
        accounting.restore_checkpoint(initialized, checkpoint);
        accounting.observe_hour(
            "2026-10-30",
            "2026-10-30T06",
            &[super::super::accounting::TrafficSample {
                id: "connection-a".into(),
                app: "app.exe".into(),
                path: "C:\\App\\app.exe".into(),
                chain: "Proxy / JP".into(),
                upload: 160,
                download: 320,
                diagnostic: None,
            }],
            1.0,
        );
        assert_eq!(accounting.totals()[&key].upload, 160);
        reopened
            .save(
                &StoredHistory {
                    version: 2,
                    entries: vec![],
                    hours: vec![],
                    slices: vec![],
                    diagnostics: vec![],
                },
                &accounting.checkpoint(),
                Some(Some("2026-10-30")),
            )
            .unwrap();
        assert!(
            reopened
                .read("2026-10-30", "2026-10-30", None, None, ReadDetail::Fine)
                .unwrap()
                .entries
                .is_empty()
        );
        assert_eq!(reopened.checkpoint().unwrap().1[0].1.upload, 160);
    }
}
