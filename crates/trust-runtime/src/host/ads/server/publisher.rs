//! ADS server value publication from runtime snapshots.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use trust_ads_core::{ads_bytes_from_value, PointQuality, SymbolDescriptor};
use trust_ads_server::{AdsErrorCode, AdsServerError, Clock, ValueIo};

use crate::debug::DebugSnapshot;

use super::contracts::AdsServerRuntimeConfig;
use super::symbols::global_name_for_server_symbol;

/// Reads current runtime values for ADS server requests.
#[derive(Clone)]
pub struct AdsServerValuePublisher {
    config: AdsServerRuntimeConfig,
    snapshot_provider: Arc<dyn Fn() -> Option<TimedDebugSnapshot> + Send + Sync>,
    clock_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl AdsServerValuePublisher {
    /// Creates a publisher from the runtime snapshot provider.
    #[must_use]
    pub fn new(
        config: AdsServerRuntimeConfig,
        snapshot_provider: Arc<dyn Fn() -> Option<DebugSnapshot> + Send + Sync>,
    ) -> Self {
        let timed_provider =
            Arc::new(move || snapshot_provider().map(TimedDebugSnapshot::captured_now));
        Self::new_with_timed_provider(config, timed_provider, Arc::new(now_ms))
    }

    #[must_use]
    pub(super) fn new_with_timed_provider(
        config: AdsServerRuntimeConfig,
        snapshot_provider: Arc<dyn Fn() -> Option<TimedDebugSnapshot> + Send + Sync>,
        clock_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            config,
            snapshot_provider,
            clock_ms,
        }
    }

    fn quality_for_snapshot(&self, captured_wall_ms: u64) -> PointQuality {
        let now_ms = (self.clock_ms)();
        let age_ms = now_ms.saturating_sub(captured_wall_ms);
        let stale_after_ms = self.config.read_timeout_ms.saturating_mul(2).max(1);
        if age_ms > stale_after_ms {
            PointQuality::stale_at(
                captured_wall_ms,
                format!("runtime snapshot is stale ({age_ms} ms old)"),
            )
        } else {
            PointQuality::good(captured_wall_ms)
        }
    }
}

impl ValueIo for AdsServerValuePublisher {
    fn read(&self, symbol: &SymbolDescriptor) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
        let timed_snapshot = (self.snapshot_provider)().ok_or_else(|| {
            AdsServerError::device(AdsErrorCode::NotReady, "runtime snapshot unavailable")
        })?;
        let snapshot = &timed_snapshot.snapshot;
        let global_name = global_name_for_server_symbol(&self.config, symbol.name.as_str())
            .ok_or_else(|| {
                AdsServerError::device(
                    AdsErrorCode::NotFound,
                    format!("ADS symbol '{}' is not a runtime global", symbol.name),
                )
            })?;
        let value = snapshot.storage.get_global(global_name).ok_or_else(|| {
            AdsServerError::device(
                AdsErrorCode::NotFound,
                format!("runtime global '{global_name}' is unavailable"),
            )
        })?;
        let bytes = ads_bytes_from_value(&symbol.data_type, value).map_err(|err| {
            AdsServerError::device(
                AdsErrorCode::InvalidData,
                format!("failed to encode ADS symbol '{}': {err}", symbol.name),
            )
        })?;
        if bytes.len() != symbol.byte_size as usize {
            return Err(AdsServerError::device(
                AdsErrorCode::InvalidSize,
                format!(
                    "encoded ADS symbol '{}' has {} bytes, descriptor expects {}",
                    symbol.name,
                    bytes.len(),
                    symbol.byte_size
                ),
            ));
        }
        Ok((
            bytes,
            self.quality_for_snapshot(timed_snapshot.captured_wall_ms),
        ))
    }
}

impl Clock for AdsServerValuePublisher {
    fn now_ms(&self) -> u64 {
        (self.clock_ms)()
    }
}

#[derive(Clone)]
pub(super) struct TimedDebugSnapshot {
    snapshot: DebugSnapshot,
    captured_wall_ms: u64,
}

impl TimedDebugSnapshot {
    fn captured_now(snapshot: DebugSnapshot) -> Self {
        Self {
            snapshot,
            captured_wall_ms: now_ms(),
        }
    }

    #[cfg(test)]
    pub(super) fn captured_at(snapshot: DebugSnapshot, captured_wall_ms: u64) -> Self {
        Self {
            snapshot,
            captured_wall_ms,
        }
    }
}

/// Returns current Unix time in milliseconds, saturated to `u64`.
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
