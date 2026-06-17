use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use trust_ads_core::{PointQuality, PointStatus};

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
    pending_writes: BTreeMap<String, Value>,
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
            })),
        }
    }

    pub(super) fn snapshot(&self) -> AdsCacheSnapshot {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        AdsCacheSnapshot {
            values: guard.values.clone(),
            qualities: guard.qualities.clone(),
            pending_writes: guard.pending_writes.clone(),
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

    pub(super) fn queue_write(&self, point_name: String, value: Value) {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pending_writes
            .insert(point_name, value);
    }

    pub(super) fn pending_writes(&self) -> BTreeMap<String, Value> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pending_writes
            .clone()
    }

    pub(super) fn ack_write(&self, point_name: &str) {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pending_writes
            .remove(point_name);
    }

    pub(super) fn mark_reconnecting(&self, now_ms: u64, reconnect_backoff_ms: u64, detail: String) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.state = AdsConnectionState::Reconnecting;
        let detail = format!(
            "{detail}; retry after {} ms",
            now_ms.saturating_add(reconnect_backoff_ms)
        );
        for quality in guard.qualities.values_mut() {
            quality.mark_stale(detail.clone());
        }
    }

    pub(super) fn mark_faulted(&self, now_ms: u64, detail: String) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.state = AdsConnectionState::Faulted;
        for quality in guard.qualities.values_mut() {
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
