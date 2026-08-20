use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use trust_ads_core::{PointQuality, PointStatus, QualityState};

use crate::value::Value;

use super::{AdsBinding, AdsConnectionState};

#[derive(Clone)]
pub(super) struct AdsSharedCache {
    inner: Arc<Mutex<AdsCacheState>>,
}

#[derive(Debug, Clone)]
struct AdsCacheState {
    state: AdsConnectionState,
    values: BTreeMap<String, Value>,
    qualities: BTreeMap<String, PointQuality>,
    pending_writes: BTreeMap<String, PendingWrite>,
    write_generations: BTreeMap<String, u64>,
}

#[derive(Debug, Clone)]
pub(super) struct PendingWrite {
    pub(super) value: Value,
    pub(super) generation: u64,
}

#[derive(Debug, Clone)]
pub(super) struct AdsCacheSnapshot {
    pub(super) values: BTreeMap<String, Value>,
    pub(super) qualities: BTreeMap<String, PointQuality>,
    pub(super) pending_writes: BTreeMap<String, Value>,
}

impl AdsSharedCache {
    pub(super) fn new(bindings: &[AdsBinding]) -> Self {
        let qualities = bindings
            .iter()
            .map(|binding| {
                (
                    binding.point.point_name.clone(),
                    PointStatus::cold_start(binding.point.point_name.clone()).quality,
                )
            })
            .collect();
        Self {
            inner: Arc::new(Mutex::new(AdsCacheState {
                state: AdsConnectionState::Disconnected,
                values: BTreeMap::new(),
                qualities,
                pending_writes: BTreeMap::new(),
                write_generations: BTreeMap::new(),
            })),
        }
    }

    pub(super) fn snapshot(&self) -> AdsCacheSnapshot {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        AdsCacheSnapshot {
            values: guard.values.clone(),
            qualities: guard.qualities.clone(),
            pending_writes: guard
                .pending_writes
                .iter()
                .map(|(point_name, pending)| (point_name.clone(), pending.value.clone()))
                .collect(),
        }
    }

    pub(super) fn state(&self) -> AdsConnectionState {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .state
    }

    pub(super) fn set_state(&self, state: AdsConnectionState) {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .state = state;
    }

    pub(super) fn set_value(&self, point_name: &str, value: Value) {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .values
            .insert(point_name.to_string(), value);
    }

    pub(super) fn set_quality(&self, point_name: &str, quality: PointQuality) {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .qualities
            .insert(point_name.to_string(), quality);
    }

    pub(super) fn revoke_good_authority(&self, point_names: &[&str], detail: &str) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        for point_name in point_names {
            let Some(quality) = guard.qualities.get_mut(*point_name) else {
                continue;
            };
            if quality.state == QualityState::Good {
                quality.mark_stale(detail);
            }
        }
    }

    pub(super) fn queue_write(&self, point_name: String, value: Value) -> u64 {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let generation = next_generation(&mut guard, point_name.as_str());
        guard
            .pending_writes
            .insert(point_name.clone(), PendingWrite { value, generation });
        guard
            .qualities
            .insert(point_name, PointQuality::stale("ADS write pending"));
        generation
    }

    pub(super) fn pending_write_batch(
        &self,
    ) -> (BTreeMap<String, PendingWrite>, BTreeMap<String, u64>) {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        (
            guard.pending_writes.clone(),
            guard.write_generations.clone(),
        )
    }

    pub(super) fn reject_write(&self, point_name: &str, now_ms: u64, detail: String) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        next_generation(&mut guard, point_name);
        guard.pending_writes.remove(point_name);
        guard
            .qualities
            .insert(point_name.to_string(), PointQuality::error(now_ms, detail));
    }

    pub(super) fn ack_write_if_current(
        &self,
        point_name: &str,
        generation: u64,
        now_ms: u64,
    ) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        if !pending_generation_matches(&guard, point_name, generation) {
            return false;
        }
        guard.pending_writes.remove(point_name);
        guard
            .qualities
            .insert(point_name.to_string(), PointQuality::good(now_ms));
        true
    }

    pub(super) fn fail_write_if_current(
        &self,
        point_name: &str,
        generation: u64,
        quality: PointQuality,
    ) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        if !pending_generation_matches(&guard, point_name, generation) {
            return false;
        }
        guard.pending_writes.remove(point_name);
        guard.qualities.insert(point_name.to_string(), quality);
        true
    }

    pub(super) fn mark_reconnecting(&self, now_ms: u64, reconnect_backoff_ms: u64, detail: String) {
        self.project_reconnecting(now_ms, reconnect_backoff_ms, detail, None);
    }

    pub(super) fn mark_reconnecting_preserving(
        &self,
        now_ms: u64,
        reconnect_backoff_ms: u64,
        detail: String,
        write_generation_baseline: &BTreeMap<String, u64>,
    ) {
        self.project_reconnecting(
            now_ms,
            reconnect_backoff_ms,
            detail,
            Some(write_generation_baseline),
        );
    }

    fn project_reconnecting(
        &self,
        now_ms: u64,
        reconnect_backoff_ms: u64,
        detail: String,
        write_generation_baseline: Option<&BTreeMap<String, u64>>,
    ) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.state = AdsConnectionState::Reconnecting;
        let detail = format!(
            "{detail}; retry after {} ms",
            now_ms.saturating_add(reconnect_backoff_ms)
        );
        let protected = write_generation_baseline
            .map(|baseline| changed_write_points(&guard, baseline))
            .unwrap_or_default();
        for (point_name, quality) in &mut guard.qualities {
            if protected.contains(point_name) {
                continue;
            }
            quality.mark_stale(detail.clone());
        }
    }

    pub(super) fn mark_faulted(&self, now_ms: u64, detail: String) {
        self.project_faulted(now_ms, detail, None);
    }

    pub(super) fn mark_faulted_preserving(
        &self,
        now_ms: u64,
        detail: String,
        write_generation_baseline: &BTreeMap<String, u64>,
    ) {
        self.project_faulted(now_ms, detail, Some(write_generation_baseline));
    }

    fn project_faulted(
        &self,
        now_ms: u64,
        detail: String,
        write_generation_baseline: Option<&BTreeMap<String, u64>>,
    ) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.state = AdsConnectionState::Faulted;
        let protected = write_generation_baseline
            .map(|baseline| changed_write_points(&guard, baseline))
            .unwrap_or_default();
        for (point_name, quality) in &mut guard.qualities {
            if protected.contains(point_name) {
                continue;
            }
            quality.mark_error(now_ms, detail.clone());
        }
    }

    pub(super) fn statuses(&self) -> Vec<PointStatus> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .qualities
            .iter()
            .map(|(point_name, quality)| PointStatus {
                point_name: point_name.clone(),
                quality: quality.clone(),
            })
            .collect()
    }

    pub(super) fn status(&self, point_name: &str) -> Option<PointStatus> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .qualities
            .get(point_name)
            .cloned()
            .map(|quality| PointStatus {
                point_name: point_name.to_string(),
                quality,
            })
    }
}

fn next_generation(state: &mut AdsCacheState, point_name: &str) -> u64 {
    let generation = state
        .write_generations
        .get(point_name)
        .copied()
        .unwrap_or(0)
        .wrapping_add(1);
    state
        .write_generations
        .insert(point_name.to_string(), generation);
    generation
}

fn pending_generation_matches(state: &AdsCacheState, point_name: &str, generation: u64) -> bool {
    state
        .pending_writes
        .get(point_name)
        .is_some_and(|pending| pending.generation == generation)
}

fn changed_write_points(
    state: &AdsCacheState,
    write_generation_baseline: &BTreeMap<String, u64>,
) -> BTreeSet<String> {
    state
        .write_generations
        .iter()
        .filter(|&(point_name, current)| {
            write_generation_baseline.get(point_name.as_str()).copied() != Some(*current)
        })
        .map(|(point_name, _)| point_name.clone())
        .collect()
}
