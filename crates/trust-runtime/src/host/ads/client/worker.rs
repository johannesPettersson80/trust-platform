use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use trust_ads_core::{PointQuality, QualityState, UpdateMode};

use crate::value::Value;

use super::super::descriptors::{point_reads, point_writes};
use super::super::transport::{
    AdsHandleRequest, AdsResolvedHandle, AdsSubscribeRequest, AdsSubscription, AdsTransport,
    AdsWriteRequest,
};
use super::cache::AdsSharedCache;
use super::{validate_remote_symbols, AdsBinding, AdsBridgeError, AdsConnectionState};

const DEFAULT_WORKER_TICK_INTERVAL: Duration = Duration::from_millis(20);

pub struct AdsConnectionWorker<T> {
    transport: T,
    bindings: Vec<AdsBinding>,
    shared: AdsSharedCache,
    handles: BTreeMap<String, AdsResolvedHandle>,
    subscriptions: BTreeMap<String, AdsSubscription>,
    symbol_version: Option<u32>,
    reconnect_backoff_ms: u64,
    next_reconnect_after_ms: Option<u64>,
    symbol_version_check_interval_ms: u64,
    next_symbol_version_check_ms: Option<u64>,
}

impl<T: AdsTransport> AdsConnectionWorker<T> {
    pub(super) fn new(
        transport: T,
        bindings: Vec<AdsBinding>,
        shared: AdsSharedCache,
        reconnect_backoff_ms: u64,
        symbol_version_check_interval_ms: u64,
    ) -> Self {
        Self {
            transport,
            bindings,
            shared,
            handles: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            symbol_version: None,
            reconnect_backoff_ms,
            next_reconnect_after_ms: None,
            symbol_version_check_interval_ms,
            next_symbol_version_check_ms: None,
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        match self.shared.state() {
            AdsConnectionState::Disconnected | AdsConnectionState::Connecting => {
                self.connect(now_ms)?;
            }
            AdsConnectionState::Reconnecting => {
                if self
                    .next_reconnect_after_ms
                    .is_some_and(|retry_at| now_ms < retry_at)
                {
                    return Ok(());
                }
                let _ = self.transport.disconnect();
                self.connect(now_ms)?;
            }
            AdsConnectionState::Faulted => return Ok(()),
            AdsConnectionState::Connected => {}
        }

        if let Err(error) = self.poll_connected(now_ms) {
            self.handle_runtime_error(now_ms, &error);
            return Err(error);
        }
        Ok(())
    }

    pub fn connect(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        self.shared.set_state(AdsConnectionState::Connecting);
        let result = self
            .transport
            .connect()
            .map_err(AdsBridgeError::from)
            .and_then(|()| self.validate_online_and_resolve_handles(now_ms));
        match result {
            Ok(()) => {
                self.shared.set_state(AdsConnectionState::Connected);
                self.next_reconnect_after_ms = None;
                self.next_symbol_version_check_ms =
                    Some(now_ms.saturating_add(self.symbol_version_check_interval_ms));
                Ok(())
            }
            Err(error) => {
                self.handle_runtime_error(now_ms, &error);
                Err(error)
            }
        }
    }

    pub fn mark_reconnecting(&mut self, now_ms: u64, detail: impl Into<String>) {
        self.shared
            .mark_reconnecting(now_ms, self.reconnect_backoff_ms, detail.into());
        self.next_reconnect_after_ms = Some(now_ms.saturating_add(self.reconnect_backoff_ms));
    }

    pub fn spawn(self, tick_interval: Duration) -> Result<AdsWorkerThread, AdsBridgeError>
    where
        T: Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_ref = Arc::clone(&stop);
        let interval = if tick_interval.is_zero() {
            DEFAULT_WORKER_TICK_INTERVAL
        } else {
            tick_interval
        };
        let join = thread::Builder::new()
            .name("trust-ads-worker".to_string())
            .spawn(move || {
                let mut worker = self;
                while !stop_ref.load(Ordering::SeqCst) {
                    let _ = worker.tick(now_ms());
                    thread::sleep(interval);
                }
                let _ = worker.transport.disconnect();
            })
            .map_err(|err| {
                AdsBridgeError::transport(format!("failed to spawn ADS worker thread: {err}"))
            })?;
        Ok(AdsWorkerThread {
            stop,
            join: Some(join),
        })
    }

    pub fn handle_for_point(&self, point_name: &str) -> Option<u32> {
        self.handles.get(point_name).map(|handle| handle.handle)
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn subscription_for_point(&self, point_name: &str) -> Option<u32> {
        self.subscriptions
            .get(point_name)
            .map(|subscription| subscription.subscription_id)
    }

    fn poll_connected(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        self.refresh_handles_if_symbol_version_due(now_ms)?;
        self.drain_notifications(now_ms)?;
        self.poll_reads(now_ms)?;
        self.publish_pending_writes(now_ms)?;
        Ok(())
    }

    fn refresh_handles_if_symbol_version_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, AdsBridgeError> {
        if self
            .next_symbol_version_check_ms
            .is_some_and(|due| now_ms < due)
        {
            return Ok(false);
        }
        self.next_symbol_version_check_ms =
            Some(now_ms.saturating_add(self.symbol_version_check_interval_ms));
        let current = self.transport.symbol_version()?;
        if self.symbol_version == Some(current) {
            return Ok(false);
        }
        self.transport.disconnect().map_err(AdsBridgeError::from)?;
        self.transport.connect().map_err(AdsBridgeError::from)?;
        self.validate_online_and_resolve_handles(now_ms)?;
        Ok(true)
    }

    fn poll_reads(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        let handles = self
            .bindings
            .iter()
            .filter(|binding| {
                point_reads(binding.point.access) && binding.point.mode != UpdateMode::Notify
            })
            .map(|binding| self.handle_for_binding(binding))
            .collect::<Result<Vec<_>, _>>()?;
        if handles.is_empty() {
            return Ok(());
        }
        for read in self.transport.sumup_read(&handles)? {
            let (value, quality) = validate_ingress_sample(
                read.point_name.as_str(),
                read.value,
                read.quality,
                now_ms,
                "read",
            );
            if quality.state == QualityState::Good {
                if let Some(value) = value {
                    self.shared.set_value(read.point_name.as_str(), value);
                }
            }
            self.shared.set_quality(read.point_name.as_str(), quality);
        }
        Ok(())
    }

    fn drain_notifications(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        for sample in self.transport.drain_notifications()? {
            let (value, quality) = validate_ingress_sample(
                sample.point_name.as_str(),
                sample.value,
                sample.quality,
                now_ms,
                "notification",
            );
            if quality.state == QualityState::Good {
                if let Some(value) = value {
                    self.shared.set_value(sample.point_name.as_str(), value);
                }
            }
            self.shared.set_quality(sample.point_name.as_str(), quality);
        }
        Ok(())
    }

    fn publish_pending_writes(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        let pending = self.shared.pending_writes();
        if pending.is_empty() {
            return Ok(());
        }
        let binding_index = self.binding_index();
        let mut writes = Vec::new();
        let mut point_names = Vec::new();
        for (point_name, value) in pending {
            let Some(binding) = binding_index.get(point_name.as_str()) else {
                self.shared.set_quality(
                    point_name.as_str(),
                    PointQuality::error(now_ms, format!("unknown ADS output '{point_name}'")),
                );
                continue;
            };
            if !point_writes(binding.point.access) {
                self.shared.set_quality(
                    point_name.as_str(),
                    PointQuality::error(
                        now_ms,
                        format!("ADS point '{point_name}' is not writable"),
                    ),
                );
                self.shared.ack_write(point_name.as_str());
                continue;
            }
            writes.push(AdsWriteRequest {
                handle: self.handle_for_binding(binding)?,
                value,
            });
            point_names.push(point_name);
        }
        if writes.is_empty() {
            return Ok(());
        }
        let qualities = self.transport.sumup_write(&writes)?;
        if qualities.len() != point_names.len() {
            return Err(AdsBridgeError::validation(format!(
                "ADS write returned {} qualities for {} writes",
                qualities.len(),
                point_names.len()
            )));
        }
        for (point_name, quality) in point_names.into_iter().zip(qualities) {
            let quality = normalize_write_quality(quality, now_ms);
            if quality.state == QualityState::Good {
                self.shared.ack_write(point_name.as_str());
            }
            self.shared.set_quality(point_name.as_str(), quality);
        }
        Ok(())
    }

    fn validate_online_and_resolve_handles(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        let symbols = self.transport.upload_symbol_table()?;
        validate_remote_symbols(&self.bindings, &symbols)?;
        let requests = self
            .bindings
            .iter()
            .map(|binding| AdsHandleRequest {
                point_name: binding.point.point_name.clone(),
                address: binding.point.address.clone(),
                data_type: binding.point.data_type.clone(),
            })
            .collect::<Vec<_>>();
        let handles = self.transport.resolve_handles(&requests)?;
        let mut resolved = BTreeMap::new();
        for handle in handles {
            resolved.insert(handle.point_name.clone(), handle);
        }
        for binding in &self.bindings {
            if !resolved.contains_key(binding.point.point_name.as_str()) {
                return Err(AdsBridgeError::validation(format!(
                    "ADS handle resolution did not return point '{}'",
                    binding.point.point_name
                )));
            }
        }
        self.handles = resolved;
        self.subscribe_notify_points()?;
        self.symbol_version = Some(self.transport.symbol_version()?);
        self.next_symbol_version_check_ms =
            Some(now_ms.saturating_add(self.symbol_version_check_interval_ms));
        Ok(())
    }

    fn subscribe_notify_points(&mut self) -> Result<(), AdsBridgeError> {
        self.subscriptions.clear();
        let requests = self
            .bindings
            .iter()
            .filter(|binding| {
                point_reads(binding.point.access) && binding.point.mode == UpdateMode::Notify
            })
            .map(|binding| {
                Ok((
                    binding.point.point_name.clone(),
                    self.handle_for_binding(binding)?,
                    binding.point.mode,
                    binding.point.notification_mode,
                ))
            })
            .collect::<Result<Vec<_>, AdsBridgeError>>()?;
        for (point_name, handle, mode, notification_mode) in requests {
            let subscription = self.transport.subscribe(AdsSubscribeRequest {
                handle,
                mode,
                notification_mode,
            })?;
            self.subscriptions.insert(point_name, subscription);
        }
        Ok(())
    }

    fn handle_for_binding(
        &self,
        binding: &AdsBinding,
    ) -> Result<AdsResolvedHandle, AdsBridgeError> {
        self.handles
            .get(binding.point.point_name.as_str())
            .cloned()
            .ok_or_else(|| {
                AdsBridgeError::validation(format!(
                    "ADS point '{}' does not have a resolved handle",
                    binding.point.point_name
                ))
            })
    }

    fn binding_index(&self) -> BTreeMap<&str, &AdsBinding> {
        self.bindings
            .iter()
            .map(|binding| (binding.point.point_name.as_str(), binding))
            .collect()
    }

    fn handle_runtime_error(&mut self, now_ms: u64, error: &AdsBridgeError) {
        if error.is_transport() {
            self.mark_reconnecting(now_ms, error.to_string());
        } else {
            self.shared.mark_faulted(now_ms, error.to_string());
        }
    }
}

fn validate_ingress_sample(
    point_name: &str,
    value: Option<Value>,
    quality: PointQuality,
    now_ms: u64,
    source: &str,
) -> (Option<Value>, PointQuality) {
    if quality.state != QualityState::Good {
        return (value, quality);
    }
    let Some(value) = value else {
        return (
            None,
            PointQuality::error(
                now_ms,
                format!("ADS {source} '{point_name}' returned good quality without a value"),
            ),
        );
    };
    if contains_non_finite_float(&value) {
        return (
            None,
            PointQuality::error(
                now_ms,
                format!("ADS {source} '{point_name}' contains a non-finite REAL/LREAL value"),
            ),
        );
    }
    (Some(value), quality)
}

fn contains_non_finite_float(value: &Value) -> bool {
    match value {
        Value::Real(value) => !value.is_finite(),
        Value::LReal(value) => !value.is_finite(),
        Value::Array(array) => array.elements().iter().any(contains_non_finite_float),
        _ => false,
    }
}

pub struct AdsWorkerThread {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl AdsWorkerThread {
    pub fn shutdown(mut self) -> Result<(), AdsBridgeError> {
        self.request_stop();
        self.join_worker()
    }

    fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    fn join_worker(&mut self) -> Result<(), AdsBridgeError> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        join.join()
            .map_err(|_| AdsBridgeError::validation("ADS worker thread panicked"))
    }
}

impl Drop for AdsWorkerThread {
    fn drop(&mut self) {
        self.request_stop();
        let _ = self.join_worker();
    }
}

fn normalize_write_quality(quality: PointQuality, now_ms: u64) -> PointQuality {
    if quality.last_update_ms.is_some() {
        quality
    } else if quality.state == QualityState::Good {
        PointQuality::good(now_ms)
    } else {
        PointQuality::error(
            now_ms,
            quality
                .detail
                .unwrap_or_else(|| "ADS write failed without detail".to_string()),
        )
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
