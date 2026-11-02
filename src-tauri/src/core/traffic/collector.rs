use super::accounting::{
    DailyKey, DiagnosticRecord, HourlyKey, TrafficAccounting, TrafficBytes, TrafficGroup, TrafficSample, is_proxy_chain,
};
use super::{database::ReadDetail, process::application_identity};
use crate::{
    process::AsyncHandler,
    utils::{connections_stream, dirs},
};
use anyhow::{Context as _, Result};
use chrono::{Local, NaiveDate, Timelike as _};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const HISTORY_FILE: &str = "traffic/history.sqlite3";
const SAVE_INTERVAL: Duration = Duration::from_secs(5);
const STREAM_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct CollectorState {
    accounting: TrafficAccounting,
    connected: bool,
    loaded: bool,
    persistence_error: Option<String>,
    persistence_disabled: bool,
    revision: u64,
    saved_revision: u64,
    observed_in_session: bool,
}

static STATE: LazyLock<Mutex<CollectorState>> = LazyLock::new(|| Mutex::new(CollectorState::default()));
static SAVE_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));
static STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
pub(super) struct StoredHistory {
    pub(super) version: u32,
    pub(super) entries: Vec<StoredEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) hours: Vec<StoredHour>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) slices: Vec<StoredSlices>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) diagnostics: Vec<StoredDiagnostic>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct StoredHour {
    pub(super) hour: String,
    pub(super) app: String,
    pub(super) path: String,
    pub(super) chain: String,
    pub(super) upload: u64,
    pub(super) download: u64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct StoredSlices {
    pub(super) day: String,
    pub(super) app: String,
    pub(super) path: String,
    pub(super) chain: String,
    pub(super) values: Vec<(u16, u64, u64)>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct StoredDiagnostic {
    pub(super) day: String,
    pub(super) app: String,
    pub(super) path: String,
    pub(super) chain: String,
    #[serde(flatten)]
    pub(super) record: DiagnosticRecord,
}

#[derive(Serialize, Deserialize)]
pub(super) struct StoredEntry {
    pub(super) day: String,
    pub(super) app: String,
    pub(super) path: String,
    pub(super) chain: String,
    pub(super) upload: u64,
    pub(super) download: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(super) duration_incomplete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_activity_at: Option<i64>,
}

fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrafficRow {
    app: String,
    path: String,
    chain: String,
    active: bool,
    last_activity_at: Option<i64>,
    day_duration_ms: Option<u64>,
    duration_incomplete: bool,
    upload_rate: f64,
    download_rate: f64,
    day_upload: u64,
    day_download: u64,
    month_upload: u64,
    month_download: u64,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrafficView {
    connected: bool,
    loaded: bool,
    history_error: Option<String>,
    history_path: Option<String>,
    history_bytes: Option<u64>,
    rows: Vec<TrafficRow>,
}

#[derive(Serialize, specta::Type)]
pub struct DailyTrafficPoint {
    day: String,
    upload: u64,
    download: u64,
}

#[derive(Serialize, specta::Type)]
pub struct TrafficSeriesPoint {
    period: String,
    upload: u64,
    download: u64,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrafficDiagnosticView {
    chain: String,
    connection_active: Option<bool>,
    live_process: Option<super::process::ApplicationProcess>,
    #[serde(flatten)]
    record: DiagnosticRecord,
}

pub(crate) fn application_diagnostics(
    app: &str,
    path: &str,
    day: &str,
    chains: &[String],
) -> Result<Vec<TrafficDiagnosticView>> {
    NaiveDate::parse_from_str(day, "%Y-%m-%d").context("Invalid day")?;
    let accounting = queried_accounting(day, day, Some(app), Some(path), ReadDetail::Diagnostics)?;
    let identity = application_identity(app, path);
    let state = STATE.lock();
    let mut records: Vec<_> = accounting
        .diagnostic_records()
        .filter(|(key, _)| {
            key.day == day
                && application_identity(&key.group.app, &key.group.path) == identity
                && (chains.is_empty() || chains.contains(&key.group.chain))
        })
        .map(|(key, record)| TrafficDiagnosticView {
            chain: key.group.chain.clone(),
            connection_active: state
                .accounting
                .connection_active(&record.connection_id)
                .map(|active| state.connected && active),
            live_process: None,
            record: record.clone(),
        })
        .collect();
    records.sort_by(|a, b| b.record.last_seen.cmp(&a.record.last_seen));
    records.truncate(100);
    drop(state);
    #[cfg(windows)]
    if records
        .iter()
        .any(|record| record.connection_active == Some(true) && record.record.lookup_local.is_some())
    {
        let processes = super::process::ProcessSnapshot::capture();
        for record in &mut records {
            if record.connection_active == Some(true)
                && !app.starts_with("WSL:")
                && let Some(local) = record.record.lookup_local
            {
                record.live_process =
                    processes.resolve_identity(&record.record.network, local, record.record.lookup_remote);
            }
        }
    }
    Ok(records)
}

fn series_for_application(
    accounting: &TrafficAccounting,
    app: &str,
    path: &str,
    end: NaiveDate,
    days: u8,
    chains: &[String],
    proxy_only: bool,
) -> Result<Vec<TrafficSeriesPoint>> {
    if ![1, 3, 7, 30].contains(&days) {
        anyhow::bail!("Invalid traffic chart range");
    }
    let start = if days == 30 {
        end.checked_sub_months(chrono::Months::new(1))
            .and_then(|date| date.checked_add_days(chrono::Days::new(1)))
    } else {
        end.checked_sub_days(chrono::Days::new(u64::from(days - 1)))
    }
    .context("Invalid date range")?;
    let first = start.format("%Y-%m-%d").to_string();
    let last = end.format("%Y-%m-%d").to_string();
    let mut points = BTreeMap::<String, TrafficBytes>::new();
    let identity = application_identity(app, path);
    let matches = |group: &TrafficGroup| {
        application_identity(&group.app, &group.path) == identity
            && (chains.is_empty() || chains.contains(&group.chain))
            && (!proxy_only || is_proxy_chain(&group.chain))
    };
    if days == 1 {
        let mut detailed_hours = BTreeMap::<HourlyKey, TrafficBytes>::new();
        for (key, slices) in accounting.slices() {
            if matches(&key.group) && key.day == first {
                for (minute, bytes) in slices {
                    let seconds = u32::from(*minute) * 5;
                    let period = format!(
                        "{}T{:02}:{:02}:{:02}",
                        key.day,
                        seconds / 3600,
                        seconds / 60 % 60,
                        seconds % 60
                    );
                    let total = points.entry(period).or_default();
                    total.upload = total.upload.saturating_add(bytes.upload);
                    total.download = total.download.saturating_add(bytes.download);
                    let hour = detailed_hours
                        .entry(HourlyKey {
                            hour: format!("{}T{:02}", key.day, seconds / 3600),
                            group: key.group.clone(),
                        })
                        .or_default();
                    hour.upload = hour.upload.saturating_add(bytes.upload);
                    hour.download = hour.download.saturating_add(bytes.download);
                }
            }
        }
        for (key, bytes) in accounting.hours() {
            if !matches(&key.group) || !key.hour.starts_with(&first) {
                continue;
            }
            let detailed = detailed_hours.get(key);
            let legacy = TrafficBytes {
                upload: bytes.upload.saturating_sub(detailed.map_or(0, |total| total.upload)),
                download: bytes
                    .download
                    .saturating_sub(detailed.map_or(0, |total| total.download)),
            };
            if legacy.upload != 0 || legacy.download != 0 {
                let total = points.entry(key.hour.clone()).or_default();
                total.upload = total.upload.saturating_add(legacy.upload);
                total.download = total.download.saturating_add(legacy.download);
            }
        }
    } else if days == 3 {
        for (key, bytes) in accounting.hours() {
            let day = &key.hour[..10];
            if matches(&key.group) && day >= first.as_str() && day <= last.as_str() {
                let value = points.entry(key.hour.clone()).or_default();
                value.upload = value.upload.saturating_add(bytes.upload);
                value.download = value.download.saturating_add(bytes.download);
            }
        }
    } else {
        for (key, bytes) in accounting.totals() {
            if matches(&key.group) && key.day >= first && key.day <= last {
                let value = points.entry(key.day.clone()).or_default();
                value.upload = value.upload.saturating_add(bytes.upload);
                value.download = value.download.saturating_add(bytes.download);
            }
        }
    }
    Ok(points
        .into_iter()
        .map(|(period, total)| TrafficSeriesPoint {
            period,
            upload: total.upload,
            download: total.download,
        })
        .collect())
}

pub(crate) fn application_series(
    app: &str,
    path: &str,
    day: &str,
    days: u8,
    chains: &[String],
    proxy_only: bool,
) -> Result<Vec<TrafficSeriesPoint>> {
    let end = NaiveDate::parse_from_str(day, "%Y-%m-%d").context("Invalid day")?;
    let first = if days == 30 {
        end.checked_sub_months(chrono::Months::new(1))
            .and_then(|date| date.checked_add_days(chrono::Days::new(1)))
    } else {
        end.checked_sub_days(chrono::Days::new(u64::from(days.saturating_sub(1))))
    }
    .context("Invalid date range")?;
    let accounting = queried_accounting(
        &first.format("%Y-%m-%d").to_string(),
        day,
        Some(app),
        Some(path),
        match days {
            1 => ReadDetail::Fine,
            3 => ReadDetail::Hours,
            _ => ReadDetail::Totals,
        },
    )?;
    series_for_application(&accounting, app, path, end, days, chains, proxy_only)
}

fn application_month_history(
    totals: &BTreeMap<DailyKey, TrafficBytes>,
    app: &str,
    path: &str,
    month: &str,
) -> Vec<DailyTrafficPoint> {
    let mut days = BTreeMap::<String, TrafficBytes>::new();
    for (key, total) in totals {
        if application_identity(&key.group.app, &key.group.path) != application_identity(app, path)
            || !key.day.starts_with(month)
            || !is_proxy_chain(&key.group.chain)
        {
            continue;
        }
        let day = days.entry(key.day.clone()).or_default();
        day.upload = day.upload.saturating_add(total.upload);
        day.download = day.download.saturating_add(total.download);
    }
    days.into_iter()
        .map(|(day, total)| DailyTrafficPoint {
            day,
            upload: total.upload,
            download: total.download,
        })
        .collect()
}

pub(crate) fn application_history(app: &str, path: &str, month: &str) -> Result<Vec<DailyTrafficPoint>> {
    NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").context("Invalid month")?;
    let accounting = queried_accounting(
        &format!("{month}-01"),
        &format!("{month}-31"),
        Some(app),
        Some(path),
        ReadDetail::Totals,
    )?;
    Ok(application_month_history(accounting.totals(), app, path, month))
}

fn observe_sample(
    state: &mut CollectorState,
    day: &str,
    hour: &str,
    minute: u16,
    samples: &[TrafficSample],
    elapsed: f64,
    first_after_connect: bool,
) {
    state.connected = true;
    let interrupted = elapsed > STREAM_TIMEOUT.as_secs_f64()
        || (first_after_connect && !state.observed_in_session && state.accounting.has_baseline());
    if interrupted {
        state.accounting.reset_interval();
    }
    let activity_changed = if interrupted {
        false
    } else {
        state
            .accounting
            .observe_activity(day, chrono::Utc::now().timestamp_millis(), samples)
    };
    let changed = if interrupted && state.accounting.has_baseline() {
        // An outage delta has no reliable fine-grained timestamps.
        let changed = state.accounting.observe_hour(day, hour, samples, elapsed);
        state.accounting.observe_slice(day, hour, minute, samples, 0.0) || changed
    } else {
        state.accounting.observe_slice(day, hour, minute, samples, elapsed)
    };
    if changed || activity_changed {
        state.revision = state.revision.saturating_add(1);
    }
    if interrupted {
        // Counters can span the outage; they are valid usage, but not an instantaneous speed.
        state.accounting.clear_rates();
    }
    state.observed_in_session = true;
}

fn history_path() -> Result<std::path::PathBuf> {
    Ok(dirs::app_home_dir()?.join(HISTORY_FILE))
}

fn restore_accounting(history: StoredHistory) -> TrafficAccounting {
    let mut accounting = TrafficAccounting::from_totals(
        history
            .entries
            .iter()
            .map(|entry| {
                (
                    DailyKey {
                        day: entry.day.clone(),
                        group: TrafficGroup {
                            app: entry.app.clone(),
                            path: entry.path.clone(),
                            chain: entry.chain.clone(),
                        },
                    },
                    TrafficBytes {
                        upload: entry.upload,
                        download: entry.download,
                    },
                )
            })
            .collect(),
    );
    for entry in history.entries {
        let key = DailyKey {
            day: entry.day,
            group: TrafficGroup {
                app: entry.app,
                path: entry.path,
                chain: entry.chain,
            },
        };
        accounting.restore_activity(key.clone(), entry.last_activity_at);
        accounting.restore_duration(key, entry.duration_ms, entry.duration_incomplete);
    }
    for entry in history.hours {
        accounting.restore_hour(
            HourlyKey {
                hour: entry.hour,
                group: TrafficGroup {
                    app: entry.app,
                    path: entry.path,
                    chain: entry.chain,
                },
            },
            TrafficBytes {
                upload: entry.upload,
                download: entry.download,
            },
        );
    }
    for entry in history.slices {
        let key = DailyKey {
            day: entry.day,
            group: TrafficGroup {
                app: entry.app,
                path: entry.path,
                chain: entry.chain,
            },
        };
        for (slot, upload, download) in entry.values {
            accounting.restore_slice(key.clone(), slot, TrafficBytes { upload, download });
        }
    }
    for entry in history.diagnostics {
        accounting.restore_diagnostic(
            DailyKey {
                day: entry.day,
                group: TrafficGroup {
                    app: entry.app,
                    path: entry.path,
                    chain: entry.chain,
                },
            },
            entry.record,
        );
    }
    accounting
}

async fn load_history() {
    let result = tauri::async_runtime::spawn_blocking(|| -> Result<_> {
        let db = super::database::Database::open(&history_path()?)?;
        super::database::keep_open(&history_path()?)?;
        let day = Local::now().format("%Y-%m-%d").to_string();
        let mut accounting = restore_accounting(db.read(&day, &day, None, None, ReadDetail::Resume)?);
        let (initialized, previous) = db.checkpoint()?;
        accounting.restore_checkpoint(initialized, previous);
        accounting.compact_committed(&day);
        Ok(accounting)
    })
    .await;
    let mut state = STATE.lock();
    match result {
        Ok(Ok(accounting)) => state.accounting = accounting,
        Ok(Err(error)) => {
            state.persistence_error = Some(format!("{error:#}"));
            state.persistence_disabled = true;
        }
        Err(error) => {
            state.persistence_error = Some(error.to_string());
            state.persistence_disabled = true;
        }
    }
    state.loaded = true;
}

fn queried_accounting(
    first: &str,
    last: &str,
    app: Option<&str>,
    path: Option<&str>,
    details: ReadDetail,
) -> Result<TrafficAccounting> {
    // A flush must not compact the live overlay between the database read and snapshot.
    let _save_guard = SAVE_LOCK.blocking_lock();
    let db = super::database::Database::open(&history_path()?)?;
    let mut history = db.read(first, last, app, path, details)?;
    let live = stored_history(&STATE.lock());
    let identity = app.zip(path).map(|(app, path)| application_identity(app, path));
    let matches = |day: &str, candidate_app: &str, candidate_path: &str| {
        day >= first
            && day <= last
            && identity.as_ref().map_or_else(
                || app.is_none_or(|app| app == candidate_app) && path.is_none_or(|path| path == candidate_path),
                |identity| &application_identity(candidate_app, candidate_path) == identity,
            )
    };
    for entry in live
        .entries
        .into_iter()
        .filter(|entry| matches(&entry.day, &entry.app, &entry.path))
    {
        history.entries.retain(|old| {
            (
                old.day.as_str(),
                old.app.as_str(),
                old.path.as_str(),
                old.chain.as_str(),
            ) != (
                entry.day.as_str(),
                entry.app.as_str(),
                entry.path.as_str(),
                entry.chain.as_str(),
            )
        });
        history.entries.push(entry);
    }
    if details.hours() {
        for entry in live
            .hours
            .into_iter()
            .filter(|entry| matches(&entry.hour[..10], &entry.app, &entry.path))
        {
            history.hours.retain(|old| {
                (
                    old.hour.as_str(),
                    old.app.as_str(),
                    old.path.as_str(),
                    old.chain.as_str(),
                ) != (
                    entry.hour.as_str(),
                    entry.app.as_str(),
                    entry.path.as_str(),
                    entry.chain.as_str(),
                )
            });
            history.hours.push(entry);
        }
    }
    if details.slices() {
        for entry in live
            .slices
            .into_iter()
            .filter(|entry| matches(&entry.day, &entry.app, &entry.path))
        {
            if let Some(old) = history.slices.iter_mut().find(|old| {
                (
                    old.day.as_str(),
                    old.app.as_str(),
                    old.path.as_str(),
                    old.chain.as_str(),
                ) == (
                    entry.day.as_str(),
                    entry.app.as_str(),
                    entry.path.as_str(),
                    entry.chain.as_str(),
                )
            }) {
                for value in entry.values {
                    old.values.retain(|old| old.0 != value.0);
                    old.values.push(value);
                }
            } else {
                history.slices.push(entry);
            }
        }
    }
    if details.diagnostics() {
        for entry in live
            .diagnostics
            .into_iter()
            .filter(|entry| matches(&entry.day, &entry.app, &entry.path))
        {
            history.diagnostics.retain(|old| {
                (
                    old.day.as_str(),
                    old.app.as_str(),
                    old.path.as_str(),
                    old.chain.as_str(),
                    old.record.destination.as_str(),
                    old.record.network.as_str(),
                    old.record.reason.as_str(),
                ) != (
                    entry.day.as_str(),
                    entry.app.as_str(),
                    entry.path.as_str(),
                    entry.chain.as_str(),
                    entry.record.destination.as_str(),
                    entry.record.network.as_str(),
                    entry.record.reason.as_str(),
                )
            });
            history.diagnostics.push(entry);
        }
    }
    Ok(restore_accounting(history))
}

#[cfg(test)]
fn samples_from_payload(payload: &serde_json::Value) -> Vec<TrafficSample> {
    samples_with_identities(payload, &mut super::process::ConnectionIdentityCache::default())
}

fn samples_with_identities(
    payload: &serde_json::Value,
    identities: &mut super::process::ConnectionIdentityCache,
) -> Vec<TrafficSample> {
    let Some(connections) = payload.get("connections").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    identities.retain(connections);
    #[cfg(windows)]
    let processes = connections
        .iter()
        .any(|item| {
            if identities.get(item).is_some_and(|(_, path)| !path.is_empty())
                || item["metadata"]["type"]
                    .as_str()
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("inner"))
            {
                return false;
            }
            let metadata = item.get("metadata");
            metadata
                .and_then(|m| m.get("process"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .is_empty()
                || metadata
                    .and_then(|m| m.get("processPath"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .is_empty()
        })
        .then(super::process::ProcessSnapshot::capture);
    connections
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_owned();
            if id.is_empty() {
                return None;
            }
            let metadata = item.get("metadata");
            let text = |key: &str| {
                metadata
                    .and_then(|value| value.get(key))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            let chain = item
                .get("chains")
                .and_then(serde_json::Value::as_array)
                .map(|chains| {
                    chains
                        .iter()
                        .rev()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" / ")
                })
                .unwrap_or_default();
            #[allow(unused_mut)]
            let mut app = text("process");
            #[allow(unused_mut)]
            let mut path = text("processPath");
            #[cfg(windows)]
            if app.is_empty()
                && let Some(source) = super::process::local_executable_path(&path)
            {
                app = source.file_name().unwrap_or_default().to_string_lossy().into_owned();
            }
            if let Some((known_app, known_path)) = identities.get(item)
                && (app.is_empty() || app.eq_ignore_ascii_case(&known_app))
            {
                if app.is_empty() {
                    app = known_app;
                }
                if path.is_empty() {
                    path = known_path;
                }
            }
            #[cfg(windows)]
            let endpoints = metadata.and_then(super::process::metadata_endpoints);
            #[cfg(windows)]
            let owner = endpoints.as_ref().and_then(|(network, local, remote)| {
                processes
                    .as_ref()
                    .and_then(|snapshot| snapshot.resolve_identity(network, *local, *remote))
                    .or_else(|| {
                        text("type")
                            .eq_ignore_ascii_case("tun")
                            .then(|| processes.as_ref()?.resolve_tun_identity(network, *local))
                            .flatten()
                    })
            });
            #[cfg(windows)]
            if let Some((wsl_app, wsl_path)) = metadata.and_then(super::wsl::resolve) {
                app = wsl_app;
                path = wsl_path;
            } else if (app.is_empty() || path.is_empty())
                && let Some(found) = &owner
                && (app.is_empty() || app.eq_ignore_ascii_case(&found.name))
            {
                if app.is_empty() {
                    app = found.name.clone();
                }
                if path.is_empty() {
                    path = found.path.clone();
                }
            }
            if text("type").eq_ignore_ascii_case("inner") {
                app = "@clash-core".into();
                path = "clash-internal://mihomo".into();
            }
            identities.remember(item, &app, &path);
            let unknown_wsl = app.starts_with("WSL:") && app.ends_with("/?");
            let unknown = (app.is_empty() && path.is_empty()) || unknown_wsl;
            let diagnostic = unknown.then(|| {
                let limit = |value: String| value.chars().filter(|c| !c.is_control()).take(255).collect::<String>();
                let port = |key: &str| {
                    metadata
                        .and_then(|value| value.get(key))
                        .map(|value| value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()))
                        .unwrap_or_default()
                };
                let endpoint = |host: String, port: String| {
                    if host.is_empty() || port.is_empty() {
                        return host;
                    }
                    if host.contains(':') && !host.starts_with('[') {
                        format!("[{host}]:{port}")
                    } else {
                        format!("{host}:{port}")
                    }
                };
                let host = text("host");
                let destination = if host.is_empty() { text("destinationIP") } else { host };
                DiagnosticRecord {
                    connection_id: id.clone(),
                    inbound_type: limit(text("type")),
                    #[cfg(windows)]
                    lookup_local: endpoints.as_ref().map(|(_, local, _)| *local),
                    #[cfg(not(windows))]
                    lookup_local: None,
                    #[cfg(windows)]
                    lookup_remote: endpoints.as_ref().and_then(|(_, _, remote)| *remote),
                    #[cfg(not(windows))]
                    lookup_remote: None,
                    #[cfg(windows)]
                    process: if app.starts_with("WSL:") { None } else { owner.clone() },
                    #[cfg(not(windows))]
                    process: None,
                    destination: limit(endpoint(destination, port("destinationPort"))),
                    network: limit(text("network")),
                    reason: if unknown_wsl {
                        "wsl_process_unresolved"
                    } else if metadata.and_then(super::process::metadata_endpoints).is_none() {
                        "missing_endpoints"
                    } else {
                        "process_unresolved"
                    }
                    .to_owned(),
                    source: limit(endpoint(text("sourceIP"), port("sourcePort"))),
                    last_seen: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    upload: 0,
                    download: 0,
                }
            });
            Some(TrafficSample {
                id,
                app,
                path,
                chain,
                upload: item
                    .get("upload")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                download: item
                    .get("download")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                diagnostic,
            })
        })
        .collect()
}

pub(crate) fn start() {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    AsyncHandler::spawn(|| async {
        load_history().await;
        let mut last_saved_at = Instant::now();
        let mut last_sample_at: Option<(Instant, i64)> = None;
        let mut identities = super::process::ConnectionIdentityCache::default();
        loop {
            let mut stream = match connections_stream::connect_connections_stream().await {
                Ok(stream) => stream,
                Err(_) => {
                    {
                        let mut state = STATE.lock();
                        state.connected = false;
                        state.accounting.clear_rates();
                    }
                    tokio::time::sleep(RETRY_DELAY).await;
                    continue;
                }
            };
            let mut first_after_connect = true;
            while let connections_stream::StreamConsumeState::Event(payload) =
                stream.next_event(STREAM_TIMEOUT, || false).await
            {
                let now = Instant::now();
                let local_time = Local::now();
                let elapsed = last_sample_at.map_or(1.0, |(instant, wall)| {
                    now.duration_since(instant)
                        .as_secs_f64()
                        .max((local_time.timestamp_millis() - wall).max(0) as f64 / 1000.0)
                });
                last_sample_at = Some((now, local_time.timestamp_millis()));
                let day = local_time.format("%Y-%m-%d").to_string();
                let hour = local_time.format("%Y-%m-%dT%H").to_string();
                let minute = ((local_time.hour() * 3600 + local_time.minute() * 60 + local_time.second()) / 5) as u16;
                #[cfg(windows)]
                if !super::wsl::status().enabled {
                    identities.forget_wsl();
                }
                let samples = samples_with_identities(&payload, &mut identities);
                {
                    let mut state = STATE.lock();
                    observe_sample(&mut state, &day, &hour, minute, &samples, elapsed, first_after_connect);
                }
                first_after_connect = false;
                if last_saved_at.elapsed() >= SAVE_INTERVAL {
                    let _ = flush().await;
                    last_saved_at = Instant::now();
                }
            }
            {
                let mut state = STATE.lock();
                state.connected = false;
                state.accounting.clear_rates();
            }
            let _ = flush().await;
            connections_stream::disconnect_connection(stream.connection_id).await;
            tokio::time::sleep(RETRY_DELAY).await;
        }
    });
}

fn stored_history(state: &CollectorState) -> StoredHistory {
    let entries = state
        .accounting
        .totals()
        .iter()
        .map(|(key, total)| StoredEntry {
            day: key.day.clone(),
            app: key.group.app.clone(),
            path: key.group.path.clone(),
            chain: key.group.chain.clone(),
            upload: total.upload,
            download: total.download,
            duration_ms: state.accounting.duration(key),
            duration_incomplete: state.accounting.duration_incomplete(key),
            last_activity_at: state.accounting.last_activity(key),
        })
        .collect();
    let hours = state
        .accounting
        .hours()
        .iter()
        .map(|(key, total)| StoredHour {
            hour: key.hour.clone(),
            app: key.group.app.clone(),
            path: key.group.path.clone(),
            chain: key.group.chain.clone(),
            upload: total.upload,
            download: total.download,
        })
        .collect();
    let slices = state
        .accounting
        .slices()
        .iter()
        .map(|(key, values)| StoredSlices {
            day: key.day.clone(),
            app: key.group.app.clone(),
            path: key.group.path.clone(),
            chain: key.group.chain.clone(),
            values: values
                .iter()
                .map(|(minute, total)| (*minute, total.upload, total.download))
                .collect(),
        })
        .collect();
    let diagnostics = state
        .accounting
        .diagnostic_records()
        .map(|(key, record)| StoredDiagnostic {
            day: key.day.clone(),
            app: key.group.app.clone(),
            path: key.group.path.clone(),
            chain: key.group.chain.clone(),
            record: record.clone(),
        })
        .collect();
    StoredHistory {
        version: 2,
        entries,
        hours,
        slices,
        diagnostics,
    }
}

pub(crate) async fn flush() -> Result<()> {
    let _save_guard = SAVE_LOCK.lock().await;
    let (history, checkpoint, revision) = {
        let state = STATE.lock();
        if !state.loaded || state.persistence_disabled || state.revision == state.saved_revision {
            return Ok(());
        }
        (stored_history(&state), state.accounting.checkpoint(), state.revision)
    };
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        super::database::Database::open(&history_path()?)?.save(&history, &checkpoint, None)
    })
    .await
    .context("Traffic database writer stopped")?;
    let mut state = STATE.lock();
    match result {
        Ok(()) => {
            state.saved_revision = revision;
            state.persistence_error = None;
            if state.revision == revision {
                state
                    .accounting
                    .compact_committed(&Local::now().format("%Y-%m-%d").to_string());
            }
            Ok(())
        }
        Err(error) => {
            state.persistence_error = Some(format!("{error:#}"));
            Err(error)
        }
    }
}

pub(crate) async fn backup_database(destination: String) -> Result<()> {
    let path = std::path::PathBuf::from(&destination);
    anyhow::ensure!(
        path.is_absolute()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("sqlite3") || value.eq_ignore_ascii_case("db")),
        "Choose an absolute SQLite backup path"
    );
    anyhow::ensure!(
        !path
            .to_string_lossy()
            .eq_ignore_ascii_case(&history_path()?.to_string_lossy()),
        "The backup must be separate from the active database"
    );
    flush().await?;
    tauri::async_runtime::spawn_blocking(move || super::database::Database::open(&history_path()?)?.backup(&path))
        .await
        .context("Traffic backup worker stopped")?
}

pub(crate) fn storage_info(check_integrity: bool) -> Result<super::database::StorageInfo> {
    super::database::Database::open(&history_path()?)?.storage_info(check_integrity)
}

pub(crate) async fn compact_database() -> Result<super::database::StorageInfo> {
    flush().await?;
    let _save_guard = SAVE_LOCK.lock().await;
    tauri::async_runtime::spawn_blocking(|| {
        let database = super::database::Database::open(&history_path()?)?;
        database.compact()?;
        database.storage_info(false)
    })
    .await
    .context("Traffic storage worker stopped")?
}

pub(crate) fn snapshot(day: &str, month: &str) -> Result<TrafficView> {
    NaiveDate::parse_from_str(day, "%Y-%m-%d").context("Invalid day")?;
    NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").context("Invalid month")?;
    let accounting = queried_accounting(
        &format!("{month}-01").min(day.to_owned()),
        &format!("{month}-31").max(day.to_owned()),
        None,
        None,
        ReadDetail::Totals,
    )?;
    let mut view = build_snapshot(day, month, &accounting, &STATE.lock());
    view.history_bytes = view
        .history_path
        .as_ref()
        .and_then(|path| std::fs::metadata(path).ok())
        .map(|meta| meta.len());
    Ok(view)
}

fn build_snapshot(day: &str, month: &str, accounting: &TrafficAccounting, state: &CollectorState) -> TrafficView {
    let mut rows = BTreeMap::<TrafficGroup, TrafficRow>::new();
    let make_row = |group: &TrafficGroup| TrafficRow {
        app: group.app.clone(),
        path: group.path.clone(),
        chain: group.chain.clone(),
        active: false,
        last_activity_at: None,
        day_duration_ms: None,
        duration_incomplete: false,
        upload_rate: 0.0,
        download_rate: 0.0,
        day_upload: 0,
        day_download: 0,
        month_upload: 0,
        month_download: 0,
    };
    for (group, rate) in state.accounting.rates() {
        let row = rows.entry(group.clone()).or_insert_with(|| make_row(group));
        row.upload_rate = rate.upload;
        row.download_rate = rate.download;
        row.active = state.connected;
    }
    for (key, total) in accounting.totals() {
        let is_day = key.day == day;
        let is_month = key.day.starts_with(month);
        if !is_day && !is_month {
            continue;
        }
        let row = rows.entry(key.group.clone()).or_insert_with(|| make_row(&key.group));
        if is_day {
            row.day_duration_ms = accounting.duration(key);
            row.duration_incomplete = accounting.duration_incomplete(key);
            row.last_activity_at = accounting.last_activity(key);
            row.day_upload = row.day_upload.saturating_add(total.upload);
            row.day_download = row.day_download.saturating_add(total.download);
        }
        if is_month {
            row.month_upload = row.month_upload.saturating_add(total.upload);
            row.month_download = row.month_download.saturating_add(total.download);
        }
    }
    let mut rows: Vec<_> = rows.into_values().collect();
    rows.sort_by(|left, right| {
        let total = |row: &TrafficRow| row.month_upload.saturating_add(row.month_download);
        total(right)
            .cmp(&total(left))
            .then_with(|| left.app.cmp(&right.app))
            .then_with(|| left.chain.cmp(&right.chain))
    });
    TrafficView {
        connected: state.connected,
        loaded: state.loaded,
        history_error: state.persistence_error.clone(),
        history_path: history_path().ok().map(|path| path.to_string_lossy().into_owned()),
        history_bytes: None,
        rows,
    }
}

pub(crate) async fn clear_history(day: Option<String>) -> Result<()> {
    if let Some(day) = &day {
        NaiveDate::parse_from_str(day, "%Y-%m-%d").context("Invalid day")?;
    }
    let _save_guard = SAVE_LOCK.lock().await;
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = STATE.lock();
        if !state.loaded || state.persistence_disabled {
            anyhow::bail!("Traffic history is not available for writing");
        }
        let previous = state.accounting.clone();
        state.accounting.clear_history(day.as_deref());
        let result: Result<()> = (|| {
            super::database::Database::open(&history_path()?)?.save(
                &stored_history(&state),
                &state.accounting.checkpoint(),
                Some(day.as_deref()),
            )
        })();
        match result {
            Ok(()) => {
                state.revision = state.revision.saturating_add(1);
                state.saved_revision = state.revision;
                state.persistence_error = None;
                Ok(())
            }
            Err(error) => {
                state.accounting = previous;
                state.persistence_error = Some(format!("{error:#}"));
                Err(error)
            }
        }
    })
    .await
    .context("Traffic clear worker stopped")?
}
#[cfg(test)]
mod tests {
    use super::samples_from_payload;

    #[test]
    fn connection_identity_survives_missing_metadata_but_not_connection_reuse() {
        let mut cache = super::super::process::ConnectionIdentityCache::default();
        let mut item = serde_json::json!({"id":"same","start":"2026-10-30T12:00:00Z","metadata":{"process":"a.exe","processPath":"C:\\Apps\\a.exe","network":"tcp","type":"HTTP","sourceIP":"127.0.0.1","sourcePort":"62000"}});
        super::samples_with_identities(&serde_json::json!({"connections":[item.clone()]}), &mut cache);
        item["metadata"]["process"] = serde_json::json!("");
        item["metadata"]["processPath"] = serde_json::json!("");
        let same = super::samples_with_identities(&serde_json::json!({"connections":[item.clone()]}), &mut cache);
        assert_eq!(same[0].app, "a.exe");
        assert_eq!(same[0].path, r"C:\Apps\a.exe");
        item["start"] = serde_json::json!("2026-10-30T13:00:00Z");
        let different = super::samples_with_identities(&serde_json::json!({"connections":[item.clone()]}), &mut cache);
        assert!(different[0].app.is_empty());
        super::samples_with_identities(&serde_json::json!({"connections":[]}), &mut cache);
        assert!(cache.get(&item).is_none());
        let internal =
            samples_from_payload(&serde_json::json!({"connections":[{"id":"core","metadata":{"type":"Inner"}}]}));
        assert_eq!(internal[0].app, "@clash-core");
        assert!(internal[0].diagnostic.is_none());
    }

    #[test]
    fn five_second_series_preserves_the_last_bucket_without_u16_overflow() {
        let mut accounting = super::TrafficAccounting::default();
        accounting.restore_slice(
            super::DailyKey {
                day: "2026-10-30".into(),
                group: super::TrafficGroup {
                    app: "app".into(),
                    path: "app.exe".into(),
                    chain: "Proxy".into(),
                },
            },
            17279,
            super::TrafficBytes {
                upload: 10,
                download: 20,
            },
        );
        let points = super::series_for_application(
            &accounting,
            "app",
            "app.exe",
            chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
            1,
            &[],
            true,
        )
        .unwrap();
        assert_eq!(points[0].period, "2026-10-30T23:59:55");
        assert_eq!(points[0].upload, 10);
    }

    #[test]
    fn old_history_loads_without_fabricating_duration() {
        let history: super::StoredHistory = serde_json::from_value(serde_json::json!({
            "version": 1,
            "entries": [{"day":"2026-10-30", "app":"old.exe", "path":"old.exe", "chain":"DIRECT", "upload":12, "download":34}]
        })).unwrap();
        let entry = &history.entries[0];
        assert_eq!((entry.upload, entry.download), (12, 34));
        assert_eq!(entry.duration_ms, None);
        let key = super::DailyKey {
            day: entry.day.clone(),
            group: super::TrafficGroup {
                app: entry.app.clone(),
                path: entry.path.clone(),
                chain: entry.chain.clone(),
            },
        };
        let mut accounting = super::TrafficAccounting::default();
        accounting.restore_duration(key.clone(), entry.duration_ms, entry.duration_incomplete);
        assert!(accounting.duration_incomplete(&key));
        assert_eq!(accounting.duration(&key), None);
        assert!(history.slices.is_empty());
        assert!(history.diagnostics.is_empty());
    }

    #[test]
    fn minute_history_serializes_compactly_without_losing_hourly_totals() {
        let mut state = super::CollectorState::default();
        let sample = |upload| super::TrafficSample {
            id: "a".into(),
            app: "app".into(),
            path: "app.exe".into(),
            chain: "DIRECT".into(),
            upload,
            download: upload * 2,
            diagnostic: None,
        };
        state
            .accounting
            .observe_slice("2026-10-30", "2026-10-30T10", 600, &[sample(0)], 1.0);
        state
            .accounting
            .observe_slice("2026-10-30", "2026-10-30T10", 600, &[sample(10)], 1.0);
        state
            .accounting
            .observe_slice("2026-10-30", "2026-10-30T10", 601, &[sample(25)], 1.0);
        let encoded = serde_json::to_vec(&super::stored_history(&state)).unwrap();
        let history: super::StoredHistory = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(history.version, 2);
        assert_eq!(history.slices.len(), 1);
        assert_eq!(history.slices[0].values, vec![(600, 10, 20), (601, 15, 30)]);
        assert_eq!((history.hours[0].upload, history.hours[0].download), (25, 50));
        assert_eq!((history.entries[0].upload, history.entries[0].download), (25, 50));
    }

    #[test]
    fn minute_series_keeps_legacy_remainder_and_respects_chain_filter() {
        use super::{DailyKey, HourlyKey, TrafficAccounting, TrafficBytes, TrafficGroup, series_for_application};
        let group = |chain: &str| TrafficGroup {
            app: "app".into(),
            path: "app.exe".into(),
            chain: chain.into(),
        };
        let mut accounting = TrafficAccounting::default();
        accounting.restore_hour(
            HourlyKey {
                hour: "2026-10-30T14".into(),
                group: group("DIRECT"),
            },
            TrafficBytes {
                upload: 100,
                download: 200,
            },
        );
        accounting.restore_slice(
            DailyKey {
                day: "2026-10-30".into(),
                group: group("DIRECT"),
            },
            (14 * 60 + 15) * 12,
            TrafficBytes {
                upload: 30,
                download: 80,
            },
        );
        accounting.restore_hour(
            HourlyKey {
                hour: "2026-10-30T15".into(),
                group: group("Proxy"),
            },
            TrafficBytes { upload: 7, download: 9 },
        );
        accounting.restore_slice(
            DailyKey {
                day: "2026-10-30".into(),
                group: group("Proxy"),
            },
            (15 * 60 + 30) * 12,
            TrafficBytes { upload: 7, download: 9 },
        );
        let end = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let all = series_for_application(&accounting, "app", "app.exe", end, 1, &[], false).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(
            (&*all[0].period, all[0].upload, all[0].download),
            ("2026-10-30T14", 70, 120)
        );
        assert_eq!(
            (&*all[1].period, all[1].upload, all[1].download),
            ("2026-10-30T14:15:00", 30, 80)
        );
        assert_eq!(
            (&*all[2].period, all[2].upload, all[2].download),
            ("2026-10-30T15:30:00", 7, 9)
        );
        let filtered = series_for_application(&accounting, "app", "app.exe", end, 1, &["Proxy".into()], false).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].period, "2026-10-30T15:30:00");
        let proxy = series_for_application(&accounting, "app", "app.exe", end, 1, &[], true).unwrap();
        assert_eq!(proxy.len(), 1);
        assert_eq!((proxy[0].upload, proxy[0].download), (7, 9));
        assert!(
            series_for_application(&accounting, "app", "app.exe", end, 1, &["DIRECT".into()], true)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unknown_payload_diagnostics_report_only_observed_endpoints() {
        let payload = serde_json::json!({"connections": [{"id":"a", "metadata": {
            "network":"tcp", "host":"example.test", "destinationPort":"443",
            "sourceIP":"127.0.0.1", "sourcePort":"12345", "type":"HTTP", "inboundPort":"7890"
        }, "chains":["DIRECT"], "upload":2, "download":3}]});
        let samples = samples_from_payload(&payload);
        let diagnostic = samples[0].diagnostic.as_ref().unwrap();
        assert_eq!(diagnostic.destination, "example.test:443");
        assert_eq!(diagnostic.source, "127.0.0.1:12345");
        assert_eq!(diagnostic.reason, "process_unresolved");
        assert_eq!((diagnostic.upload, diagnostic.download), (0, 0));
    }

    #[test]
    fn chart_uses_hourly_data_for_short_ranges_and_daily_data_for_long_ranges() {
        use super::{DailyKey, HourlyKey, TrafficAccounting, TrafficBytes, TrafficGroup, series_for_application};
        use std::collections::BTreeMap;
        let group = |chain: &str| TrafficGroup {
            app: "app".into(),
            path: "app.exe".into(),
            chain: chain.into(),
        };
        let mut daily = BTreeMap::new();
        daily.insert(
            DailyKey {
                day: "2026-10-29".into(),
                group: group("DIRECT"),
            },
            TrafficBytes {
                upload: 4,
                download: 20,
            },
        );
        daily.insert(
            DailyKey {
                day: "2026-10-30".into(),
                group: group("Proxy"),
            },
            TrafficBytes {
                upload: 9,
                download: 50,
            },
        );
        let mut accounting = TrafficAccounting::from_totals(daily);
        for chain in ["DIRECT", "Proxy"] {
            accounting.restore_hour(
                HourlyKey {
                    hour: "2026-10-30T14".into(),
                    group: group(chain),
                },
                TrafficBytes { upload: 1, download: 3 },
            );
        }
        let end = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let hour = series_for_application(&accounting, "app", "app.exe", end, 1, &[], false).unwrap();
        assert_eq!(hour.len(), 1);
        assert_eq!(
            (&*hour[0].period, hour[0].upload, hour[0].download),
            ("2026-10-30T14", 2, 6)
        );
        let week = series_for_application(&accounting, "app", "app.exe", end, 7, &[], false).unwrap();
        assert_eq!(week.len(), 2);
        assert_eq!(week[0].download, 20);
        let proxy_week = series_for_application(&accounting, "app", "app.exe", end, 7, &[], true).unwrap();
        assert_eq!(proxy_week.len(), 1);
        assert_eq!(proxy_week[0].download, 50);
        assert!(series_for_application(&accounting, "app", "app.exe", end, 2, &[], false).is_err());
    }

    #[test]
    fn application_history_combines_chains_only_for_the_requested_application_and_month() {
        use super::{DailyKey, TrafficBytes, TrafficGroup, application_month_history};
        use std::collections::BTreeMap;
        let mut totals = BTreeMap::new();
        let mut add = |day: &str, app: &str, chain: &str, upload, download| {
            totals.insert(
                DailyKey {
                    day: day.into(),
                    group: TrafficGroup {
                        app: app.into(),
                        path: format!("{app}.exe"),
                        chain: chain.into(),
                    },
                },
                TrafficBytes { upload, download },
            );
        };
        add("2026-10-02", "browser", "DIRECT", 2, 3);
        add("2026-10-02", "browser", "Japan", 5, 7);
        add("2026-10-03", "browser", "Japan", 11, 13);
        add("2026-10-02", "other", "DIRECT", 100, 100);
        add("2026-11-02", "browser", "DIRECT", 100, 100);
        let points = application_month_history(&totals, "browser", "browser.exe", "2026-10");
        assert_eq!(points.len(), 2);
        assert_eq!(
            (&*points[0].day, points[0].upload, points[0].download),
            ("2026-10-02", 5, 7)
        );
        assert_eq!(
            (&*points[1].day, points[1].upload, points[1].download),
            ("2026-10-03", 11, 13)
        );
    }

    #[test]
    fn reconnect_records_bytes_without_exposing_an_artificial_speed_spike() {
        use super::{CollectorState, TrafficSample, observe_sample};
        let sample = |upload| TrafficSample {
            id: "a".into(),
            app: "browser".into(),
            path: "browser.exe".into(),
            chain: "DIRECT".into(),
            upload,
            download: 0,
            diagnostic: None,
        };
        let mut state = CollectorState::default();
        observe_sample(&mut state, "2026-10-30", "2026-10-30T12", 720, &[sample(0)], 1.0, true);
        observe_sample(
            &mut state,
            "2026-10-30",
            "2026-10-30T12",
            720,
            &[sample(100)],
            3600.0,
            true,
        );
        assert!(state.accounting.rates().is_empty());
        assert_eq!(state.accounting.totals().values().next().unwrap().upload, 100);
        observe_sample(
            &mut state,
            "2026-10-30",
            "2026-10-30T12",
            720,
            &[sample(110)],
            1.0,
            false,
        );
        assert_eq!(state.accounting.rates().values().next().unwrap().upload, 10.0);
        assert_eq!(state.accounting.hours().values().next().unwrap().upload, 110);
        assert_eq!(state.accounting.slices().values().next().unwrap()[&720].upload, 10);
    }

    #[test]
    fn sleep_without_a_reconnect_preserves_fine_samples_and_separates_the_unknown_delta() {
        let sample = |upload| super::TrafficSample {
            id: "same".into(),
            app: "a".into(),
            path: "a.exe".into(),
            chain: "Proxy".into(),
            upload,
            download: 0,
            diagnostic: None,
        };
        let mut state = super::CollectorState::default();
        super::observe_sample(
            &mut state,
            "2026-10-30",
            "2026-10-30T17",
            12240,
            &[sample(0)],
            1.0,
            true,
        );
        super::observe_sample(
            &mut state,
            "2026-10-30",
            "2026-10-30T17",
            12241,
            &[sample(10)],
            1.0,
            false,
        );
        super::observe_sample(
            &mut state,
            "2026-10-30",
            "2026-10-30T17",
            12300,
            &[sample(110)],
            300.0,
            false,
        );
        assert!(state.accounting.rates().is_empty());
        super::observe_sample(
            &mut state,
            "2026-10-30",
            "2026-10-30T17",
            12301,
            &[sample(120)],
            1.0,
            false,
        );
        let points = super::series_for_application(
            &state.accounting,
            "a",
            "a.exe",
            chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
            1,
            &[],
            true,
        )
        .unwrap();
        assert_eq!(
            points.iter().find(|point| point.period.len() == 13).unwrap().upload,
            100
        );
        assert_eq!(
            points
                .iter()
                .filter(|point| point.period.len() == 19)
                .map(|point| point.upload)
                .sum::<u64>(),
            20
        );
        super::observe_sample(
            &mut state,
            "2026-10-30",
            "2026-10-30T17",
            12302,
            &[sample(125)],
            1.0,
            true,
        );
        assert_eq!(state.accounting.slices().values().next().unwrap()[&12302].upload, 5);
        assert_eq!(state.accounting.totals().values().next().unwrap().upload, 125);
    }

    #[test]
    fn recognizes_application_and_reverses_the_mihomo_chain() {
        let payload = serde_json::json!({
            "connections": [{
                "id": "connection-1",
                "metadata": {"process": "browser.exe", "processPath": "C:\\\\Apps\\\\browser.exe"},
                "chains": ["Japan03", "AI"],
                "upload": 7,
                "download": 15
            }]
        });
        let samples = samples_from_payload(&payload);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].app, "browser.exe");
        assert_eq!(samples[0].chain, "AI / Japan03");
        assert_eq!((samples[0].upload, samples[0].download), (7, 15));
    }

    #[test]
    fn history_view_uses_the_selected_day_and_month() {
        use super::{CollectorState, TrafficSample, build_snapshot};
        let sample = |upload, download| TrafficSample {
            id: "a".into(),
            app: "browser.exe".into(),
            path: "C:\\browser.exe".into(),
            chain: "DIRECT".into(),
            upload,
            download,
            diagnostic: None,
        };
        let mut state = CollectorState::default();
        state.accounting.observe("2026-10-29", &[], 1.0);
        state.accounting.observe("2026-10-29", &[sample(10, 20)], 1.0);
        state.accounting.observe("2026-10-30", &[sample(40, 50)], 1.0);
        state.accounting.observe("2026-11-01", &[sample(50, 60)], 1.0);
        let view = build_snapshot("2026-10-29", "2026-10", &state.accounting, &state);
        assert_eq!(view.rows.len(), 1);
        assert_eq!((view.rows[0].day_upload, view.rows[0].day_download), (10, 20));
        assert_eq!((view.rows[0].month_upload, view.rows[0].month_download), (40, 50));
    }

    #[test]
    fn retains_direct_connections_with_unknown_application() {
        let payload = serde_json::json!({
            "connections": [{"id": "direct", "metadata": {}, "chains": ["DIRECT"], "upload": 1, "download": 2}]
        });
        let samples = samples_from_payload(&payload);
        assert_eq!(samples.len(), 1);
        assert!(samples[0].app.is_empty());
        assert_eq!(samples[0].chain, "DIRECT");
    }
}
