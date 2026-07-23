use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex};
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

#[cfg(test)]
mod tests;

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

struct ConnectionCandidate {
    handles: BTreeMap<String, AdsResolvedHandle>,
    subscriptions: BTreeMap<String, AdsSubscription>,
    symbol_version: u32,
}

struct WorkerPollError {
    error: AdsBridgeError,
    write_generation_baseline: Option<BTreeMap<String, u64>>,
}

impl WorkerPollError {
    fn plain(error: AdsBridgeError) -> Self {
        Self {
            error,
            write_generation_baseline: None,
        }
    }

    fn after_write(
        error: AdsBridgeError,
        write_generation_baseline: BTreeMap<String, u64>,
    ) -> Self {
        Self {
            error,
            write_generation_baseline: Some(write_generation_baseline),
        }
    }
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

        if let Err(failure) = self.poll_connected(now_ms) {
            self.handle_runtime_error_preserving(
                now_ms,
                &failure.error,
                failure.write_generation_baseline.as_ref(),
            );
            return Err(failure.error);
        }
        Ok(())
    }

    pub fn connect(&mut self, now_ms: u64) -> Result<(), AdsBridgeError> {
        self.shared.set_state(AdsConnectionState::Connecting);
        let result = self
            .transport
            .connect()
            .map_err(AdsBridgeError::from)
            .and_then(|()| self.build_connection_candidate());
        match result {
            Ok(candidate) => {
                self.publish_candidate(candidate, now_ms);
                self.shared.set_state(AdsConnectionState::Connected);
                self.next_reconnect_after_ms = None;
                Ok(())
            }
            Err(error) => {
                self.handle_runtime_error(now_ms, &error);
                Err(error)
            }
        }
    }

    pub fn mark_reconnecting(&mut self, now_ms: u64, detail: impl Into<String>) {
        let _ = self.transport.disconnect();
        self.invalidate_local_correlation();
        self.shared
            .mark_reconnecting(now_ms, self.reconnect_backoff_ms, detail.into());
        self.next_reconnect_after_ms = Some(now_ms.saturating_add(self.reconnect_backoff_ms));
    }

    fn mark_reconnecting_preserving(
        &mut self,
        now_ms: u64,
        detail: String,
        write_generation_baseline: &BTreeMap<String, u64>,
    ) {
        let _ = self.transport.disconnect();
        self.invalidate_local_correlation();
        self.shared.mark_reconnecting_preserving(
            now_ms,
            self.reconnect_backoff_ms,
            detail,
            write_generation_baseline,
        );
        self.next_reconnect_after_ms = Some(now_ms.saturating_add(self.reconnect_backoff_ms));
    }

    pub fn spawn(self, tick_interval: Duration) -> Result<AdsWorkerThread, AdsBridgeError>
    where
        T: Send + 'static,
    {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
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
                loop {
                    if stop_requested(stop_ref.as_ref()) {
                        break;
                    }
                    let _ = worker.tick(now_ms());
                    if wait_for_stop(stop_ref.as_ref(), interval) {
                        break;
                    }
                }
                worker.finish_shutdown();
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

    fn poll_connected(&mut self, now_ms: u64) -> Result<(), WorkerPollError> {
        self.refresh_handles_if_symbol_version_due(now_ms)
            .map_err(WorkerPollError::plain)?;
        self.drain_notifications(now_ms)
            .map_err(WorkerPollError::plain)?;
        self.poll_reads(now_ms).map_err(WorkerPollError::plain)?;
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
        self.invalidate_local_correlation();
        self.mark_input_authority_stale("ADS symbol version changed; refreshing correlation");
        self.transport.disconnect().map_err(AdsBridgeError::from)?;
        self.transport.connect().map_err(AdsBridgeError::from)?;
        let candidate = self.build_connection_candidate()?;
        self.publish_candidate(candidate, now_ms);
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
        let reads = self.transport.sumup_read(&handles)?;
        if reads.len() != handles.len() {
            return Err(AdsBridgeError::validation(format!(
                "ADS read returned {} results for {} handles",
                reads.len(),
                handles.len()
            )));
        }
        for (handle, read) in handles.iter().zip(&reads) {
            if read.point_name != handle.point_name {
                return Err(AdsBridgeError::validation(format!(
                    "ADS read result '{}' does not match requested point '{}'",
                    read.point_name, handle.point_name
                )));
            }
        }
        for read in reads {
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
            let Some(subscription) = self.subscriptions.get(sample.point_name.as_str()) else {
                return Err(AdsBridgeError::validation(format!(
                    "ADS notification point '{}' has no active subscription",
                    sample.point_name
                )));
            };
            if subscription.subscription_id != sample.subscription_id {
                return Err(AdsBridgeError::validation(format!(
                    "ADS notification point '{}' returned subscription {} but active subscription is {}",
                    sample.point_name, sample.subscription_id, subscription.subscription_id
                )));
            }
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

    fn publish_pending_writes(&mut self, now_ms: u64) -> Result<(), WorkerPollError> {
        let (pending, write_generation_baseline) = self.shared.pending_write_batch();
        if pending.is_empty() {
            return Ok(());
        }
        let binding_index = self.binding_index();
        let mut writes = Vec::new();
        let mut point_names = Vec::new();
        for (point_name, pending) in pending {
            let Some(binding) = binding_index.get(point_name.as_str()) else {
                self.shared.fail_write_if_current(
                    point_name.as_str(),
                    pending.generation,
                    PointQuality::error(now_ms, format!("unknown ADS output '{point_name}'")),
                );
                continue;
            };
            if !point_writes(binding.point.access) {
                self.shared.fail_write_if_current(
                    point_name.as_str(),
                    pending.generation,
                    PointQuality::error(
                        now_ms,
                        format!("ADS point '{point_name}' is not writable"),
                    ),
                );
                continue;
            }
            writes.push(AdsWriteRequest {
                handle: self
                    .handle_for_binding(binding)
                    .map_err(WorkerPollError::plain)?,
                value: pending.value,
            });
            point_names.push((point_name, pending.generation));
        }
        if writes.is_empty() {
            return Ok(());
        }
        let qualities = self.transport.sumup_write(&writes).map_err(|error| {
            WorkerPollError::after_write(
                AdsBridgeError::from(error),
                write_generation_baseline.clone(),
            )
        })?;
        if qualities.len() != point_names.len() {
            return Err(WorkerPollError::after_write(
                AdsBridgeError::validation(format!(
                    "ADS write returned {} qualities for {} writes",
                    qualities.len(),
                    point_names.len()
                )),
                write_generation_baseline,
            ));
        }
        for ((point_name, generation), quality) in point_names.into_iter().zip(qualities) {
            let quality = normalize_write_quality(quality, now_ms);
            if quality.state == QualityState::Good {
                let acknowledged_at = quality.last_update_ms.unwrap_or(now_ms);
                self.shared
                    .ack_write_if_current(point_name.as_str(), generation, acknowledged_at);
            } else {
                self.shared
                    .fail_write_if_current(point_name.as_str(), generation, quality);
            }
        }
        Ok(())
    }

    fn build_connection_candidate(&mut self) -> Result<ConnectionCandidate, AdsBridgeError> {
        let symbol_version_before = self.transport.symbol_version()?;
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
        let responses = self.transport.resolve_handles(&requests)?;
        let handles = validate_resolved_handles(&requests, responses)?;
        let subscriptions = self.build_subscription_candidate(&handles)?;
        let symbol_version_after = self.transport.symbol_version()?;
        if symbol_version_before != symbol_version_after {
            return Err(AdsBridgeError::validation(format!(
                "ADS symbol version changed during candidate construction: {symbol_version_before} != {symbol_version_after}"
            )));
        }
        Ok(ConnectionCandidate {
            handles,
            subscriptions,
            symbol_version: symbol_version_after,
        })
    }

    fn build_subscription_candidate(
        &mut self,
        handles: &BTreeMap<String, AdsResolvedHandle>,
    ) -> Result<BTreeMap<String, AdsSubscription>, AdsBridgeError> {
        let requests = self
            .bindings
            .iter()
            .filter(|binding| {
                point_reads(binding.point.access) && binding.point.mode == UpdateMode::Notify
            })
            .map(|binding| {
                Ok((
                    binding.point.point_name.clone(),
                    handle_for_binding_in(handles, binding)?,
                    binding.point.mode,
                    binding.point.notification_mode,
                ))
            })
            .collect::<Result<Vec<_>, AdsBridgeError>>()?;
        let mut subscriptions = BTreeMap::new();
        let mut subscription_ids = BTreeSet::new();
        for (point_name, handle, mode, notification_mode) in requests {
            let subscription = self.transport.subscribe(AdsSubscribeRequest {
                handle,
                mode,
                notification_mode,
            })?;
            if subscription.point_name != point_name {
                return Err(AdsBridgeError::validation(format!(
                    "ADS subscription response '{}' does not match requested point '{point_name}'",
                    subscription.point_name
                )));
            }
            if !subscription_ids.insert(subscription.subscription_id) {
                return Err(AdsBridgeError::validation(format!(
                    "ADS subscription ID {} is active for more than one point",
                    subscription.subscription_id
                )));
            }
            subscriptions.insert(point_name, subscription);
        }
        Ok(subscriptions)
    }

    fn publish_candidate(&mut self, candidate: ConnectionCandidate, now_ms: u64) {
        self.handles = candidate.handles;
        self.subscriptions = candidate.subscriptions;
        self.symbol_version = Some(candidate.symbol_version);
        self.next_symbol_version_check_ms =
            Some(now_ms.saturating_add(self.symbol_version_check_interval_ms));
    }

    fn invalidate_local_correlation(&mut self) {
        self.handles.clear();
        self.subscriptions.clear();
        self.symbol_version = None;
        self.next_symbol_version_check_ms = None;
    }

    fn mark_input_authority_stale(&self, detail: &str) {
        let point_names = self
            .bindings
            .iter()
            .filter(|binding| point_reads(binding.point.access))
            .map(|binding| binding.point.point_name.as_str())
            .collect::<Vec<_>>();
        self.shared.revoke_good_authority(&point_names, detail);
    }

    fn finish_shutdown(&mut self) {
        self.invalidate_local_correlation();
        self.next_reconnect_after_ms = None;
        self.mark_input_authority_stale("ADS worker stopped; cached input is not authoritative");
        self.shared.set_state(AdsConnectionState::Disconnected);
        let _ = self.transport.disconnect();
    }

    fn handle_for_binding(
        &self,
        binding: &AdsBinding,
    ) -> Result<AdsResolvedHandle, AdsBridgeError> {
        handle_for_binding_in(&self.handles, binding)
    }

    fn binding_index(&self) -> BTreeMap<&str, &AdsBinding> {
        self.bindings
            .iter()
            .map(|binding| (binding.point.point_name.as_str(), binding))
            .collect()
    }

    fn handle_runtime_error(&mut self, now_ms: u64, error: &AdsBridgeError) {
        self.handle_runtime_error_preserving(now_ms, error, None);
    }

    fn handle_runtime_error_preserving(
        &mut self,
        now_ms: u64,
        error: &AdsBridgeError,
        write_generation_baseline: Option<&BTreeMap<String, u64>>,
    ) {
        if error.is_transport() {
            if let Some(write_generation_baseline) = write_generation_baseline {
                self.mark_reconnecting_preserving(
                    now_ms,
                    error.to_string(),
                    write_generation_baseline,
                );
            } else {
                self.mark_reconnecting(now_ms, error.to_string());
            }
        } else {
            let _ = self.transport.disconnect();
            self.invalidate_local_correlation();
            self.next_reconnect_after_ms = None;
            if let Some(write_generation_baseline) = write_generation_baseline {
                self.shared.mark_faulted_preserving(
                    now_ms,
                    error.to_string(),
                    write_generation_baseline,
                );
            } else {
                self.shared.mark_faulted(now_ms, error.to_string());
            }
        }
    }
}

fn validate_resolved_handles(
    requests: &[AdsHandleRequest],
    responses: Vec<AdsResolvedHandle>,
) -> Result<BTreeMap<String, AdsResolvedHandle>, AdsBridgeError> {
    if responses.len() != requests.len() {
        return Err(AdsBridgeError::validation(format!(
            "ADS handle resolution returned {} responses for {} requests",
            responses.len(),
            requests.len()
        )));
    }
    let requested = requests
        .iter()
        .map(|request| (request.point_name.as_str(), request))
        .collect::<BTreeMap<_, _>>();
    let mut resolved = BTreeMap::new();
    for response in responses {
        let Some(request) = requested.get(response.point_name.as_str()) else {
            return Err(AdsBridgeError::validation(format!(
                "ADS handle resolution returned unexpected point '{}'",
                response.point_name
            )));
        };
        if response.address != request.address {
            return Err(AdsBridgeError::validation(format!(
                "ADS handle resolution returned the wrong address for point '{}'",
                response.point_name
            )));
        }
        if response.data_type != request.data_type {
            return Err(AdsBridgeError::validation(format!(
                "ADS handle resolution returned the wrong descriptor for point '{}'",
                response.point_name
            )));
        }
        let point_name = response.point_name.clone();
        if resolved.insert(point_name.clone(), response).is_some() {
            return Err(AdsBridgeError::validation(format!(
                "ADS handle resolution returned point '{point_name}' more than once"
            )));
        }
    }
    Ok(resolved)
}

fn handle_for_binding_in(
    handles: &BTreeMap<String, AdsResolvedHandle>,
    binding: &AdsBinding,
) -> Result<AdsResolvedHandle, AdsBridgeError> {
    handles
        .get(binding.point.point_name.as_str())
        .cloned()
        .ok_or_else(|| {
            AdsBridgeError::validation(format!(
                "ADS point '{}' does not have a resolved handle",
                binding.point.point_name
            ))
        })
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
    if super::contains_non_finite_float(&value) {
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

pub struct AdsWorkerThread {
    stop: Arc<(Mutex<bool>, Condvar)>,
    join: Option<thread::JoinHandle<()>>,
}

impl AdsWorkerThread {
    pub fn shutdown(mut self) -> Result<(), AdsBridgeError> {
        self.request_stop();
        self.join_worker()
    }

    fn request_stop(&self) {
        let (stopped, wake) = self.stop.as_ref();
        *stopped.lock().unwrap_or_else(|err| err.into_inner()) = true;
        wake.notify_all();
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
    if quality.state == QualityState::Good {
        return PointQuality::good(quality.last_update_ms.unwrap_or(now_ms));
    }
    let detail = quality
        .detail
        .unwrap_or_else(|| "failed without detail".to_string());
    let detail = if detail.starts_with("ADS write failed: ") {
        detail
    } else {
        format!("ADS write failed: {detail}")
    };
    PointQuality::error(quality.last_update_ms.unwrap_or(now_ms), detail)
}

fn stop_requested(stop: &(Mutex<bool>, Condvar)) -> bool {
    *stop.0.lock().unwrap_or_else(|err| err.into_inner())
}

fn wait_for_stop(stop: &(Mutex<bool>, Condvar), interval: Duration) -> bool {
    let stopped = stop.0.lock().unwrap_or_else(|err| err.into_inner());
    if *stopped {
        return true;
    }
    let (stopped, _) = stop
        .1
        .wait_timeout_while(stopped, interval, |stopped| !*stopped)
        .unwrap_or_else(|err| err.into_inner());
    *stopped
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
