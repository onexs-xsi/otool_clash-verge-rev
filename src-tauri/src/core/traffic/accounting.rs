use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const MAX_DIAGNOSTICS_PER_GROUP_DAY: usize = 32;

pub(crate) fn is_proxy_chain(chain: &str) -> bool {
    !matches!(
        chain.rsplit(" / ").next().unwrap_or_default().trim(),
        "" | "DIRECT" | "REJECT" | "REJECT-DROP" | "PASS" | "COMPATIBLE"
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticRecord {
    pub destination: String,
    pub network: String,
    pub reason: String,
    pub source: String,
    pub last_seen: String,
    pub upload: u64,
    pub download: u64,
    #[serde(default)]
    pub connection_id: String,
    #[serde(default)]
    pub inbound_type: String,
    #[serde(default)]
    #[specta(type = Option<String>)]
    pub lookup_local: Option<std::net::SocketAddr>,
    #[serde(default)]
    #[specta(type = Option<String>)]
    pub lookup_remote: Option<std::net::SocketAddr>,
    #[serde(default)]
    pub process: Option<super::process::ApplicationProcess>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DiagnosticKey {
    destination: String,
    network: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrafficSample {
    pub id: String,
    pub app: String,
    pub path: String,
    pub chain: String,
    pub upload: u64,
    pub download: u64,
    pub diagnostic: Option<DiagnosticRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TrafficGroup {
    pub app: String,
    pub path: String,
    pub chain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DailyKey {
    pub day: String,
    pub group: TrafficGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HourlyKey {
    pub hour: String,
    pub group: TrafficGroup,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TrafficBytes {
    pub upload: u64,
    pub download: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct TrafficRate {
    pub upload: f64,
    pub download: f64,
}

#[derive(Clone, Default)]
pub(crate) struct TrafficAccounting {
    initialized: bool,
    previous: HashMap<String, TrafficBytes>,
    totals: BTreeMap<DailyKey, TrafficBytes>,
    hours: BTreeMap<HourlyKey, TrafficBytes>,
    slices: BTreeMap<DailyKey, BTreeMap<u16, TrafficBytes>>,
    diagnostics: BTreeMap<DailyKey, BTreeMap<DiagnosticKey, DiagnosticRecord>>,
    rates: BTreeMap<TrafficGroup, TrafficRate>,
    durations: BTreeMap<DailyKey, u64>,
    incomplete_durations: BTreeSet<DailyKey>,
    previous_groups: BTreeSet<TrafficGroup>,
    previous_day: Option<String>,
    activity: BTreeMap<DailyKey, i64>,
}

impl TrafficAccounting {
    pub fn observe_activity(&mut self, day: &str, at: i64, samples: &[TrafficSample]) -> bool {
        if !self.initialized {
            return false;
        }
        let mut changed = false;
        for sample in samples {
            let active = self
                .previous
                .get(&sample.id)
                .is_none_or(|previous| sample.upload > previous.upload || sample.download > previous.download);
            if !active {
                continue;
            }
            let key = DailyKey {
                day: day.to_owned(),
                group: TrafficGroup {
                    app: sample.app.clone(),
                    path: sample.path.clone(),
                    chain: sample.chain.clone(),
                },
            };
            let previous = self.activity.entry(key.clone()).or_insert(0);
            if at > *previous {
                *previous = at;
                changed = true;
            }
            self.totals.entry(key).or_default();
        }
        changed
    }

    pub fn last_activity(&self, key: &DailyKey) -> Option<i64> {
        self.activity.get(key).copied()
    }

    pub fn restore_activity(&mut self, key: DailyKey, at: Option<i64>) {
        if let Some(at) = at {
            self.activity.insert(key, at);
        }
    }
    pub fn connection_active(&self, id: &str) -> Option<bool> {
        (!id.is_empty()).then(|| self.previous.contains_key(id))
    }

    pub fn from_totals(totals: BTreeMap<DailyKey, TrafficBytes>) -> Self {
        Self {
            totals,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub fn observe(&mut self, day: &str, samples: &[TrafficSample], elapsed_seconds: f64) -> bool {
        self.observe_inner(day, None, None, samples, elapsed_seconds)
    }

    pub fn observe_hour(&mut self, day: &str, hour: &str, samples: &[TrafficSample], elapsed_seconds: f64) -> bool {
        self.observe_inner(day, Some(hour), None, samples, elapsed_seconds)
    }

    pub fn observe_slice(
        &mut self,
        day: &str,
        hour: &str,
        slice: u16,
        samples: &[TrafficSample],
        elapsed_seconds: f64,
    ) -> bool {
        self.observe_inner(day, Some(hour), Some(slice), samples, elapsed_seconds)
    }

    fn observe_inner(
        &mut self,
        day: &str,
        hour: Option<&str>,
        slice: Option<u16>,
        samples: &[TrafficSample],
        elapsed_seconds: f64,
    ) -> bool {
        let mut changed = false;
        let mut next_previous = HashMap::with_capacity(samples.len());
        self.rates.clear();
        let elapsed = if elapsed_seconds.is_finite() && elapsed_seconds > 0.0 {
            elapsed_seconds
        } else {
            1.0
        };
        let mut groups = BTreeSet::new();
        for sample in samples {
            let current = TrafficBytes {
                upload: sample.upload,
                download: sample.download,
            };
            let group = TrafficGroup {
                app: sample.app.clone(),
                path: sample.path.clone(),
                chain: sample.chain.clone(),
            };
            groups.insert(group.clone());
            let delta = if !self.initialized {
                TrafficBytes::default()
            } else if let Some(previous) = self.previous.get(&sample.id) {
                TrafficBytes {
                    upload: current.upload.saturating_sub(previous.upload),
                    download: current.download.saturating_sub(previous.download),
                }
            } else {
                current
            };
            next_previous.insert(sample.id.clone(), current);
            if let Some(diagnostic) = &sample.diagnostic {
                let key = DailyKey {
                    day: day.to_owned(),
                    group: group.clone(),
                };
                let records = self.diagnostics.entry(key).or_default();
                let diagnostic_key = DiagnosticKey {
                    destination: diagnostic.destination.clone(),
                    network: diagnostic.network.clone(),
                    reason: diagnostic.reason.clone(),
                };
                if !records.contains_key(&diagnostic_key)
                    && records.len() >= MAX_DIAGNOSTICS_PER_GROUP_DAY
                    && let Some(oldest) = records
                        .iter()
                        .min_by(|(_, a), (_, b)| a.last_seen.cmp(&b.last_seen))
                        .map(|(key, _)| key.clone())
                {
                    records.remove(&oldest);
                }
                let record = records.entry(diagnostic_key).or_insert_with(|| diagnostic.clone());
                record.source.clone_from(&diagnostic.source);
                record.connection_id.clone_from(&diagnostic.connection_id);
                record.inbound_type.clone_from(&diagnostic.inbound_type);
                record.lookup_local = diagnostic.lookup_local;
                record.lookup_remote = diagnostic.lookup_remote;
                record.process.clone_from(&diagnostic.process);
                record.last_seen.clone_from(&diagnostic.last_seen);
                record.upload = record.upload.saturating_add(delta.upload);
                record.download = record.download.saturating_add(delta.download);
                changed = true;
            }
            let rate = self.rates.entry(group.clone()).or_default();
            rate.upload += delta.upload as f64 / elapsed;
            rate.download += delta.download as f64 / elapsed;
            if let Some(hour) = hour {
                use std::collections::btree_map::Entry;
                let hourly = match self.hours.entry(HourlyKey {
                    hour: hour.to_owned(),
                    group: group.clone(),
                }) {
                    Entry::Vacant(entry) => {
                        changed = true;
                        entry.insert(TrafficBytes::default())
                    }
                    Entry::Occupied(entry) => entry.into_mut(),
                };
                hourly.upload = hourly.upload.saturating_add(delta.upload);
                hourly.download = hourly.download.saturating_add(delta.download);
            }
            if let Some(slice) = slice {
                use std::collections::btree_map::Entry;
                let per_day = self
                    .slices
                    .entry(DailyKey {
                        day: day.to_owned(),
                        group: group.clone(),
                    })
                    .or_default();
                let recorded = match per_day.entry(slice) {
                    Entry::Vacant(entry) => {
                        changed = true;
                        entry.insert(TrafficBytes::default())
                    }
                    Entry::Occupied(entry) => entry.into_mut(),
                };
                recorded.upload = recorded.upload.saturating_add(delta.upload);
                recorded.download = recorded.download.saturating_add(delta.download);
            }
            if delta.upload != 0 || delta.download != 0 {
                changed = true;
                let key = DailyKey {
                    day: day.to_owned(),
                    group,
                };
                self.durations.entry(key.clone()).or_default();
                let total = self.totals.entry(key).or_default();
                total.upload = total.upload.saturating_add(delta.upload);
                total.download = total.download.saturating_add(delta.download);
            }
        }
        if self.previous_day.as_deref() == Some(day)
            && elapsed_seconds.is_finite()
            && (0.0..=5.0).contains(&elapsed_seconds)
        {
            let increment = (elapsed_seconds * 1000.0).round() as u64;
            for group in groups.intersection(&self.previous_groups) {
                let key = DailyKey {
                    day: day.to_owned(),
                    group: group.clone(),
                };
                let duration = self.durations.entry(key.clone()).or_default();
                *duration = duration.saturating_add(increment);
                self.totals.entry(key).or_default();
                changed |= increment > 0;
            }
        }
        self.previous_groups = groups;
        self.previous_day = Some(day.to_owned());
        self.previous = next_previous;
        self.initialized = true;
        changed
    }

    pub fn checkpoint(&self) -> (bool, Vec<(String, TrafficBytes)>) {
        (
            self.initialized,
            self.previous.iter().map(|(id, value)| (id.clone(), *value)).collect(),
        )
    }
    pub fn restore_checkpoint(&mut self, initialized: bool, values: Vec<(String, TrafficBytes)>) {
        self.initialized = initialized;
        self.previous = values.into_iter().collect();
    }
    pub fn compact_committed(&mut self, day: &str) {
        self.activity.retain(|key, _| key.day == day);
        self.totals.retain(|key, _| key.day.as_str() >= day);
        self.durations.retain(|key, _| key.day.as_str() >= day);
        self.incomplete_durations.retain(|key| key.day.as_str() >= day);
        self.hours.retain(|key, _| key.hour.as_str() >= day);
        self.diagnostics.retain(|key, _| key.day.as_str() >= day);
        self.slices.retain(|key, _| key.day.as_str() >= day);
        for values in self.slices.values_mut() {
            if let Some(last) = values.keys().next_back().copied() {
                values.retain(|slot, _| *slot == last);
            }
        }
    }
    pub const fn totals(&self) -> &BTreeMap<DailyKey, TrafficBytes> {
        &self.totals
    }
    pub const fn hours(&self) -> &BTreeMap<HourlyKey, TrafficBytes> {
        &self.hours
    }
    pub const fn slices(&self) -> &BTreeMap<DailyKey, BTreeMap<u16, TrafficBytes>> {
        &self.slices
    }
    pub fn restore_slice(&mut self, key: DailyKey, slice: u16, bytes: TrafficBytes) {
        let total = self.slices.entry(key).or_default().entry(slice).or_default();
        total.upload = total.upload.saturating_add(bytes.upload);
        total.download = total.download.saturating_add(bytes.download);
    }
    pub fn diagnostic_records(&self) -> impl Iterator<Item = (&DailyKey, &DiagnosticRecord)> {
        self.diagnostics
            .iter()
            .flat_map(|(key, records)| records.values().map(move |record| (key, record)))
    }
    pub fn restore_diagnostic(&mut self, key: DailyKey, record: DiagnosticRecord) {
        let records = self.diagnostics.entry(key).or_default();
        let diagnostic_key = DiagnosticKey {
            destination: record.destination.clone(),
            network: record.network.clone(),
            reason: record.reason.clone(),
        };
        if records.len() < MAX_DIAGNOSTICS_PER_GROUP_DAY {
            records.insert(diagnostic_key, record);
        }
    }
    pub fn restore_hour(&mut self, key: HourlyKey, bytes: TrafficBytes) {
        let total = self.hours.entry(key).or_default();
        total.upload = total.upload.saturating_add(bytes.upload);
        total.download = total.download.saturating_add(bytes.download);
    }
    pub const fn rates(&self) -> &BTreeMap<TrafficGroup, TrafficRate> {
        &self.rates
    }
    pub fn clear_rates(&mut self) {
        self.rates.clear();
    }
    pub const fn has_baseline(&self) -> bool {
        self.initialized
    }
    pub fn reset_interval(&mut self) {
        self.previous_groups.clear();
        self.previous_day = None;
    }
    pub fn duration(&self, key: &DailyKey) -> Option<u64> {
        self.durations.get(key).copied()
    }
    pub fn duration_incomplete(&self, key: &DailyKey) -> bool {
        self.incomplete_durations.contains(key)
    }
    pub fn restore_duration(&mut self, key: DailyKey, value: Option<u64>, incomplete: bool) {
        if let Some(duration) = value {
            self.durations.insert(key.clone(), duration);
        }
        if value.is_none() || incomplete {
            self.incomplete_durations.insert(key);
        }
    }
    pub fn clear_history(&mut self, day: Option<&str>) {
        self.activity.retain(|key, _| day.is_some_and(|day| key.day != day));
        self.totals.retain(|key, _| day.is_some_and(|day| key.day != day));
        self.durations.retain(|key, _| day.is_some_and(|day| key.day != day));
        self.incomplete_durations
            .retain(|key| day.is_some_and(|day| key.day != day));
        self.hours
            .retain(|key, _| day.is_some_and(|day| !key.hour.starts_with(day)));
        self.slices.retain(|key, _| day.is_some_and(|day| key.day != day));
        self.diagnostics.retain(|key, _| day.is_some_and(|day| key.day != day));
        self.reset_interval();
    }
}
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Invalid fixtures should fail tests."
)]
mod tests {
    use super::*;

    #[test]
    fn activity_tracks_new_connections_and_bytes_without_refreshing_idle_connections() {
        let mut accounting = TrafficAccounting::default();
        let samples = [sample("a", "a.exe", "a.exe", "Proxy", 100, 200)];
        assert!(!accounting.observe_activity("2026-10-30", 1000, &samples));
        accounting.observe("2026-10-30", &samples, 1.0);
        assert!(!accounting.observe_activity("2026-10-30", 2000, &samples));
        let active = [sample("a", "a.exe", "a.exe", "Proxy", 110, 200)];
        assert!(accounting.observe_activity("2026-10-30", 3000, &active));
        accounting.observe("2026-10-30", &active, 1.0);
        assert!(!accounting.observe_activity("2026-10-30", 4000, &active));
        let key = DailyKey {
            day: "2026-10-30".into(),
            group: TrafficGroup {
                app: "a.exe".into(),
                path: "a.exe".into(),
                chain: "Proxy".into(),
            },
        };
        assert_eq!(accounting.last_activity(&key), Some(3000));
        accounting.clear_history(Some("2026-10-30"));
        assert_eq!(accounting.last_activity(&key), None);
    }

    #[test]
    fn proxy_accounting_uses_only_the_final_outbound() {
        assert!(!is_proxy_chain("Microsoft / Direct / DIRECT"));
        assert!(!is_proxy_chain("REJECT"));
        assert!(!is_proxy_chain(""));
        assert!(is_proxy_chain("Direct / Japan"));
        assert!(is_proxy_chain("DIRECT / Japan"));
    }

    fn sample(id: &str, app: &str, path: &str, chain: &str, upload: u64, download: u64) -> TrafficSample {
        TrafficSample {
            id: id.into(),
            app: app.into(),
            path: path.into(),
            chain: chain.into(),
            upload,
            download,
            diagnostic: None,
        }
    }

    fn group(app: &str, path: &str, chain: &str) -> TrafficGroup {
        TrafficGroup {
            app: app.into(),
            path: path.into(),
            chain: chain.into(),
        }
    }

    fn key(day: &str, app: &str, path: &str, chain: &str) -> DailyKey {
        DailyKey {
            day: day.into(),
            group: group(app, path, chain),
        }
    }

    #[test]
    fn minute_samples_preserve_hour_and_day_totals_and_clear_by_day() {
        let mut state = TrafficAccounting::default();
        let s = |n| sample("a", "app", "app.exe", "DIRECT", n, n * 2);
        state.observe_slice("2026-10-30", "2026-10-30T10", 600, &[s(0)], 1.0);
        state.observe_slice("2026-10-30", "2026-10-30T10", 600, &[s(10)], 1.0);
        state.observe_slice("2026-10-30", "2026-10-30T10", 601, &[s(25)], 1.0);
        state.observe_slice("2026-10-31", "2026-10-31T00", 0, &[s(30)], 1.0);
        assert_eq!(state.slices().values().map(|day| day.len()).sum::<usize>(), 3);
        assert_eq!(
            state.slices()[&key("2026-10-30", "app", "app.exe", "DIRECT")][&601].upload,
            15
        );
        assert_eq!(
            state.hours()[&HourlyKey {
                hour: "2026-10-30T10".into(),
                group: group("app", "app.exe", "DIRECT")
            }]
                .upload,
            25
        );
        assert_eq!(
            state.totals()[&key("2026-10-30", "app", "app.exe", "DIRECT")].upload,
            25
        );
        state.clear_history(Some("2026-10-30"));
        assert_eq!(state.slices().len(), 1);
        assert_eq!(state.slices().keys().next().unwrap().day, "2026-10-31");
    }

    #[test]
    fn unknown_diagnostics_are_bounded_accumulate_deltas_and_clear() {
        let mut state = TrafficAccounting::default();
        state.observe("2026-10-30", &[], 1.0);
        let samples: Vec<_> = (0..35)
            .map(|n| {
                let mut value = sample(&n.to_string(), "", "", "DIRECT", 2, 3);
                value.diagnostic = Some(DiagnosticRecord {
                    destination: format!("host{n}.example:443"),
                    network: "tcp".into(),
                    reason: "process_unresolved".into(),
                    source: "127.0.0.1:1234".into(),
                    last_seen: "2026-10-30 10:00:00".into(),
                    upload: 0,
                    download: 0,
                    ..Default::default()
                });
                value
            })
            .collect();
        state.observe("2026-10-30", &samples, 1.0);
        assert_eq!(state.diagnostic_records().count(), 32);
        assert!(
            state
                .diagnostic_records()
                .any(|(_, record)| record.destination == "host34.example:443")
        );
        assert!(
            state
                .diagnostic_records()
                .all(|(_, record)| record.upload == 2 && record.download == 3)
        );
        let stored = state
            .diagnostic_records()
            .next()
            .map(|(key, record)| (key.clone(), record.clone()))
            .unwrap();
        let mut reloaded = TrafficAccounting::default();
        reloaded.restore_diagnostic(stored.0, stored.1);
        assert_eq!(reloaded.diagnostic_records().count(), 1);
        reloaded.clear_history(Some("2026-10-30"));
        assert_eq!(reloaded.diagnostic_records().count(), 0);
    }

    #[test]
    fn baseline_excludes_preexisting_bytes_and_counts_new_connections() {
        let mut state = TrafficAccounting::default();
        state.observe(
            "2026-10-30",
            &[sample("a", "browser", "browser.exe", "Proxy / Japan", 100, 500)],
            1.0,
        );
        assert!(state.totals().is_empty());
        state.observe(
            "2026-10-30",
            &[
                sample("a", "browser", "browser.exe", "Proxy / Japan", 130, 560),
                sample("b", "chat", "chat.exe", "DIRECT", 7, 42),
            ],
            2.0,
        );
        assert_eq!(
            state.totals()[&key("2026-10-30", "browser", "browser.exe", "Proxy / Japan")],
            TrafficBytes {
                upload: 30,
                download: 60
            }
        );
        assert_eq!(
            state.totals()[&key("2026-10-30", "chat", "chat.exe", "DIRECT")],
            TrafficBytes {
                upload: 7,
                download: 42
            }
        );
        assert_eq!(
            state.rates()[&group("browser", "browser.exe", "Proxy / Japan")],
            TrafficRate {
                upload: 15.0,
                download: 30.0
            }
        );
    }

    #[test]
    fn decreasing_counters_do_not_create_negative_or_duplicate_usage() {
        let mut state = TrafficAccounting::default();
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 100, 200)], 1.0);
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 2, 3)], 1.0);
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 7, 9)], 1.0);
        assert_eq!(
            state.totals()[&key("2026-10-30", "app", "a.exe", "DIRECT")],
            TrafficBytes { upload: 5, download: 6 }
        );
    }

    #[test]
    fn incremental_bytes_are_recorded_on_the_observed_local_day() {
        let mut state = TrafficAccounting::default();
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 0, 0)], 1.0);
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 10, 20)], 1.0);
        state.observe("2026-11-01", &[sample("a", "app", "a.exe", "DIRECT", 30, 40)], 1.0);
        assert_eq!(
            state.totals()[&key("2026-10-30", "app", "a.exe", "DIRECT")],
            TrafficBytes {
                upload: 10,
                download: 20
            }
        );
        assert_eq!(
            state.totals()[&key("2026-11-01", "app", "a.exe", "DIRECT")],
            TrafficBytes {
                upload: 20,
                download: 20
            }
        );
    }

    #[test]
    fn changed_identity_and_chain_attribute_only_future_bytes_to_new_group() {
        let mut state = TrafficAccounting::default();
        state.observe("2026-10-30", &[sample("a", "", "", "DIRECT", 0, 0)], 1.0);
        state.observe("2026-10-30", &[sample("a", "", "", "DIRECT", 10, 0)], 1.0);
        state.observe(
            "2026-10-30",
            &[sample("a", "app", "a.exe", "Proxy / Japan", 15, 0)],
            1.0,
        );
        assert_eq!(state.totals()[&key("2026-10-30", "", "", "DIRECT")].upload, 10);
        assert_eq!(
            state.totals()[&key("2026-10-30", "app", "a.exe", "Proxy / Japan")].upload,
            5
        );
    }

    #[test]
    fn reloading_saved_totals_does_not_recount_active_connections() {
        let mut original = TrafficAccounting::default();
        original.observe("2026-10-30", &[], 1.0);
        original.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 10, 20)], 1.0);
        let mut restored = TrafficAccounting::from_totals(original.totals().clone());
        restored.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 10, 20)], 1.0);
        assert_eq!(
            restored.totals()[&key("2026-10-30", "app", "a.exe", "DIRECT")],
            TrafficBytes {
                upload: 10,
                download: 20
            }
        );
    }

    #[test]
    fn clearing_live_rates_preserves_recorded_totals() {
        let mut state = TrafficAccounting::default();
        state.observe("2026-10-30", &[], 1.0);
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 5, 9)], 1.0);
        state.clear_rates();
        assert!(state.rates().is_empty());
        assert_eq!(state.totals()[&key("2026-10-30", "app", "a.exe", "DIRECT")].download, 9);
    }

    #[test]
    fn duration_counts_overlapping_connections_once_and_ignores_gaps() {
        let mut state = TrafficAccounting::default();
        let a = sample("a", "app", "a.exe", "DIRECT", 0, 0);
        let b = sample("b", "app", "a.exe", "DIRECT", 0, 0);
        state.observe("2026-10-30", &[a.clone(), b.clone()], 1.0);
        state.observe("2026-10-30", &[a.clone(), b], 2.0);
        let key = key("2026-10-30", "app", "a.exe", "DIRECT");
        assert_eq!(state.duration(&key), Some(2000));
        state.observe("2026-10-30", std::slice::from_ref(&a), 20.0);
        assert_eq!(state.duration(&key), Some(2000));
        state.reset_interval();
        state.observe("2026-10-30", &[a], 1.0);
        assert_eq!(state.duration(&key), Some(2000));
    }

    #[test]
    fn clearing_keeps_counter_baselines_and_other_days() {
        let mut state = TrafficAccounting::default();
        state.observe("2026-10-29", &[], 1.0);
        state.observe("2026-10-29", &[sample("a", "app", "a.exe", "DIRECT", 10, 20)], 1.0);
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 20, 30)], 1.0);
        state.clear_history(Some("2026-10-30"));
        state.observe("2026-10-30", &[sample("a", "app", "a.exe", "DIRECT", 21, 31)], 1.0);
        assert_eq!(state.totals()[&key("2026-10-29", "app", "a.exe", "DIRECT")].upload, 10);
        assert_eq!(state.totals()[&key("2026-10-30", "app", "a.exe", "DIRECT")].upload, 1);
        assert_eq!(state.duration(&key("2026-10-30", "app", "a.exe", "DIRECT")), Some(0));
        state.clear_history(None);
        assert!(state.totals().is_empty());
    }

    #[test]
    fn hourly_buckets_record_only_new_bytes_and_follow_clear_history() {
        let mut state = TrafficAccounting::default();
        let a = |upload| sample("a", "app", "a.exe", "DIRECT", upload, 0);
        state.observe_hour("2026-10-30", "2026-10-30T10", &[a(100)], 1.0);
        state.observe_hour("2026-10-30", "2026-10-30T10", &[a(110)], 1.0);
        state.observe_hour("2026-10-30", "2026-10-30T11", &[a(125)], 1.0);
        state.observe_hour("2026-10-31", "2026-10-31T00", &[a(130)], 1.0);
        let get = |hour: &str| {
            state
                .hours()
                .iter()
                .find(|(key, _)| key.hour == hour)
                .map(|(_, value)| value.upload)
        };
        assert_eq!(get("2026-10-30T10"), Some(10));
        assert_eq!(get("2026-10-30T11"), Some(15));
        assert_eq!(get("2026-10-31T00"), Some(5));
        state.clear_history(Some("2026-10-30"));
        assert_eq!(state.hours().len(), 1);
        assert!(state.hours().keys().all(|key| key.hour.starts_with("2026-10-31")));
    }
}
