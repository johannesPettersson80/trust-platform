//! Runtime-host lifecycle for the off-scan OpenOT persistence worker.

use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use crate::config::OpenOtPersistenceConfig;
use crate::config::OpenOtTelemetryConfig;

#[cfg(unix)]
use super::service_error::redacted_error;
use super::PersistenceError;
#[cfg(unix)]
use super::{
    OpenOtDocumentSink, OpenOtPersistenceConsumer, OpenOtPersistenceWorker,
    SharedMemoryOpenOtSource,
};

#[cfg(unix)]
type RuntimePersistenceWorker =
    OpenOtPersistenceWorker<SharedMemoryOpenOtSource, OpenOtDocumentSink>;

#[cfg(unix)]
fn open_runtime_worker(
    persistence: &OpenOtPersistenceConfig,
    bundle_root: &Path,
    source_path: &Path,
    definition: &open_ot_definition::DefinitionFile,
) -> Result<(RuntimePersistenceWorker, u32), PersistenceError> {
    let consumer = OpenOtPersistenceConsumer::new(definition.clone(), None)?;
    let mut sink = OpenOtDocumentSink::open_with_definitions(
        persistence,
        bundle_root,
        std::slice::from_ref(definition),
    )?;
    let schema_version = sink.schema_version()?;
    let source = SharedMemoryOpenOtSource::open_with_limits(
        source_path,
        persistence.batch_size,
        persistence.queue_capacity,
    )?;
    Ok((
        OpenOtPersistenceWorker::new(source, consumer, sink),
        schema_version,
    ))
}

#[cfg(unix)]
fn validate_startup_artifacts(
    config: &OpenOtTelemetryConfig,
    bundle_root: &Path,
) -> Result<
    (
        open_ot_definition::DefinitionFile,
        std::path::PathBuf,
        OpenOtPersistenceConfig,
    ),
    PersistenceError,
> {
    OpenOtDocumentSink::validate_backend_available(&config.persistence)?;
    validate_database_ca(&config.persistence, bundle_root)?;
    let definition_path = bundle_root.join("openot-definition.json");
    let definition_bytes = std::fs::read(&definition_path).map_err(|error| {
        PersistenceError::InvalidConfig(format!(
            "read compiled OpenOT definition '{}': {error}",
            definition_path.display()
        ))
    })?;
    let definition: open_ot_definition::DefinitionFile = serde_json::from_slice(&definition_bytes)
        .map_err(|error| {
            PersistenceError::InvalidConfig(format!(
                "parse compiled OpenOT definition '{}': {error}",
                definition_path.display()
            ))
        })?;
    let source_path = if config.path.is_absolute() {
        config.path.clone()
    } else {
        bundle_root.join(&config.path)
    };
    let persistence = config.persistence.clone();
    OpenOtPersistenceConsumer::new(definition.clone(), None)?;
    super::projection::LoggingProjector::new(std::iter::once(definition.clone()))?;
    SharedMemoryOpenOtSource::open_with_limits(
        &source_path,
        persistence.batch_size,
        persistence.queue_capacity,
    )?;
    Ok((definition, source_path, persistence))
}

#[cfg(unix)]
fn validate_database_ca(
    persistence: &OpenOtPersistenceConfig,
    bundle_root: &Path,
) -> Result<(), PersistenceError> {
    let (backend, configured_path) = match persistence.backend {
        Some(crate::config::OpenOtPersistenceBackend::Sqlite) => return Ok(()),
        Some(crate::config::OpenOtPersistenceBackend::PostgreSql) => (
            "postgresql",
            persistence
                .postgresql
                .as_ref()
                .and_then(|config| config.ca_cert_path.as_ref()),
        ),
        Some(crate::config::OpenOtPersistenceBackend::TimescaleDb) => (
            "timescaledb",
            persistence
                .timescaledb
                .as_ref()
                .and_then(|config| config.ca_cert_path.as_ref()),
        ),
        Some(crate::config::OpenOtPersistenceBackend::MySql) => (
            "mysql",
            persistence
                .mysql
                .as_ref()
                .and_then(|config| config.ca_cert_path.as_ref()),
        ),
        Some(crate::config::OpenOtPersistenceBackend::SqlServer) => (
            "sqlserver",
            persistence
                .sqlserver
                .as_ref()
                .and_then(|config| config.ca_cert_path.as_ref()),
        ),
        Some(crate::config::OpenOtPersistenceBackend::InfluxDb3) => (
            "influxdb3",
            persistence
                .influxdb3
                .as_ref()
                .and_then(|config| config.ca_cert_path.as_ref()),
        ),
        None => {
            return Err(PersistenceError::InvalidConfig(
                "runtime.openot.persistence.backend is required".to_string(),
            ));
        }
    };
    let configured_path = configured_path.ok_or_else(|| {
        PersistenceError::InvalidConfig(format!(
            "runtime.openot.persistence.{backend}.ca_cert_path is required"
        ))
    })?;
    let path = if configured_path.is_absolute() {
        configured_path.clone()
    } else {
        bundle_root.join(configured_path)
    };
    std::fs::read(&path).map_err(|error| {
        PersistenceError::InvalidConfig(format!(
            "read runtime.openot.persistence.{backend}.ca_cert_path '{}': {error}",
            path.display()
        ))
    })?;
    Ok(())
}

#[cfg(unix)]
fn apply_worker_error(
    status: &Arc<Mutex<OpenOtPersistenceStatus>>,
    error: &PersistenceError,
    consecutive_retries: &mut u32,
    retry_max_attempts: u32,
) {
    let transient = matches!(error, PersistenceError::Commit(_));
    if transient {
        *consecutive_retries = consecutive_retries.saturating_add(1);
    }
    let retry_exhausted = transient && *consecutive_retries >= retry_max_attempts;
    if let Ok(mut status) = status.lock() {
        status.state = if transient && !retry_exhausted {
            OpenOtPersistenceState::Retrying
        } else {
            OpenOtPersistenceState::Faulted
        };
        if transient {
            status.documents_retried = status.documents_retried.saturating_add(1);
        }
        status.last_error = Some(if retry_exhausted {
            format!(
                "OpenOT persistence retry budget exhausted after {consecutive_retries} consecutive failures"
            )
        } else {
            redacted_error(error)
        });
    }
}

/// Observable lifecycle of the persistence service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOtPersistenceState {
    /// No worker exists because persistence is disabled.
    Disabled,
    /// Startup validation or backend migration is in progress.
    Starting,
    /// The worker is reachable, migrated, and caught up.
    Ready,
    /// Durable cursor trails the observed producer head and is making progress.
    CatchingUp,
    /// Persistence is running with pressure, loss, or unresolved data.
    Degraded,
    /// A transient failure is waiting for the next bounded retry.
    Retrying,
    /// Shutdown completed.
    Stopped,
    /// An unrecoverable persistence error stopped progress.
    Faulted,
}

impl OpenOtPersistenceState {
    /// Stable lowercase control/status representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::CatchingUp => "catching_up",
            Self::Degraded => "degraded",
            Self::Retrying => "retrying",
            Self::Stopped => "stopped",
            Self::Faulted => "faulted",
        }
    }
}

/// Thread-safe status snapshot projected by the runtime host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtPersistenceStatus {
    /// Current service lifecycle.
    pub state: OpenOtPersistenceState,
    /// TOML-selected backend name, absent when disabled.
    pub backend: Option<String>,
    /// Compatible truST-owned schema version, absent until a selected sink opens.
    pub schema_version: Option<u32>,
    /// Canonical documents prepared by the worker.
    pub documents_read: u64,
    /// Documents newly committed by the selected sink.
    pub documents_committed: u64,
    /// Idempotent duplicate documents observed by the sink.
    pub documents_duplicated: u64,
    /// Documents durable in a required local spool but not acknowledged remotely.
    pub remote_pending: u64,
    /// Descriptive public read-model rows newly committed by this runtime.
    pub projection_rows_committed: u64,
    /// Future event records retained with fields that could not be classified.
    pub unclassified_event_count: u64,
    /// Durable remote-delivery parts confirmed by reconciliation.
    pub reconciled_part_count: u64,
    /// Durable remote-delivery parts still awaiting reconciliation.
    pub pending_part_count: u64,
    /// Retry attempts made after transient failures.
    pub documents_retried: u64,
    /// Source-ring bytes observed beyond the last durable cursor.
    pub pending: u64,
    /// Malformed carriage records rejected by the raw reader.
    pub rejected: u64,
    /// Placeholder documents preserved without guessed meaning.
    pub unresolved: u64,
    /// Queryable loss ranges emitted by loss accounting.
    pub loss_range_count: u64,
    /// Total records represented by emitted loss ranges.
    pub lost_record_count: u64,
    /// Last durable carriage cursor.
    pub cursor_abs: u64,
    /// Most recently observed producer head.
    pub head_abs: u64,
    /// Host timestamp of the most recent successful commit.
    pub last_success_time_ns: Option<u64>,
    /// Redacted actionable error, when faulted.
    pub last_error: Option<String>,
}

/// Owned host service handle. Dropping it requests bounded shutdown.
pub struct OpenOtPersistenceService {
    stop: Arc<AtomicBool>,
    drain_expired: Arc<AtomicBool>,
    shutdown_deadline: Arc<Mutex<Option<std::time::Instant>>>,
    status: Arc<Mutex<OpenOtPersistenceStatus>>,
    thread: Option<JoinHandle<()>>,
    shutdown_timeout: Duration,
}

impl OpenOtPersistenceService {
    /// Starts the configured worker, or returns `None` without side effects when disabled.
    #[cfg(unix)]
    pub fn start(
        config: &OpenOtTelemetryConfig,
        bundle_root: &Path,
    ) -> Result<Option<Self>, PersistenceError> {
        if !config.persistence.enabled {
            return Ok(None);
        }
        let (definition, source_path, persistence_config) =
            validate_startup_artifacts(config, bundle_root)?;
        // Definition parsing and shared-memory carriage availability are local
        // bundle artifacts and therefore remain synchronous startup checks.
        // Opening or migrating the selected database belongs to the supervised
        // worker below so a remote outage cannot stop PLC startup.
        let reconnect_bundle_root = bundle_root.to_path_buf();
        let reconnect_source_path = source_path.clone();
        let reconnect_definition = definition.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let drain_expired = Arc::new(AtomicBool::new(false));
        let shutdown_deadline = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(OpenOtPersistenceStatus {
            state: OpenOtPersistenceState::Starting,
            backend: config
                .persistence
                .backend
                .map(|backend| backend.as_str().to_string()),
            schema_version: None,
            documents_read: 0,
            documents_committed: 0,
            documents_duplicated: 0,
            remote_pending: 0,
            projection_rows_committed: 0,
            unclassified_event_count: 0,
            reconciled_part_count: 0,
            pending_part_count: 0,
            documents_retried: 0,
            pending: 0,
            rejected: 0,
            unresolved: 0,
            loss_range_count: 0,
            lost_record_count: 0,
            cursor_abs: 0,
            head_abs: 0,
            last_success_time_ns: None,
            last_error: None,
        }));
        let worker_stop = Arc::clone(&stop);
        let worker_drain_expired = Arc::clone(&drain_expired);
        let worker_shutdown_deadline = Arc::clone(&shutdown_deadline);
        let worker_status = Arc::clone(&status);
        let interval = Duration::from_millis(config.persistence.flush_interval_ms);
        let retry_initial = Duration::from_millis(config.persistence.retry_initial_ms);
        let retry_max = Duration::from_millis(config.persistence.retry_max_ms);
        let retry_multiplier = u32::from(config.persistence.retry_multiplier);
        let retry_max_attempts = config.persistence.retry_max_attempts;
        let thread = std::thread::Builder::new()
            .name("trust-openot-persistence".to_string())
            .spawn(move || {
                let mut worker = None;
                let mut retry_delay = retry_initial;
                let mut consecutive_retries = 0u32;
                let mut reconnect_totals = ReconnectTotals::default();
                let mut drain_target = None;
                loop {
                    let requested = worker_stop.load(Ordering::Acquire);
                    let deadline = worker_shutdown_deadline
                        .lock()
                        .ok()
                        .and_then(|deadline| *deadline);
                    if requested && deadline.is_some_and(|limit| std::time::Instant::now() >= limit)
                    {
                        worker_drain_expired.store(true, Ordering::Release);
                        break;
                    }
                    if worker.is_none() {
                        match open_runtime_worker(
                            &persistence_config,
                            &reconnect_bundle_root,
                            &reconnect_source_path,
                            &reconnect_definition,
                        ) {
                            Ok((reconnected, schema_version)) => {
                                worker = Some(reconnected);
                                if let Ok(mut status) = worker_status.lock() {
                                    status.schema_version = Some(schema_version);
                                }
                                continue;
                            }
                            Err(error) => {
                                apply_worker_error(
                                    &worker_status,
                                    &error,
                                    &mut consecutive_retries,
                                    retry_max_attempts,
                                );
                                if !matches!(error, PersistenceError::Commit(_))
                                    || consecutive_retries >= retry_max_attempts
                                {
                                    break;
                                }
                                std::thread::park_timeout(retry_delay);
                                retry_delay = retry_delay
                                    .checked_mul(retry_multiplier)
                                    .unwrap_or(retry_max)
                                    .min(retry_max);
                                continue;
                            }
                        }
                    }
                    match worker
                        .as_mut()
                        .expect("worker reconnected above")
                        .run_once(unix_time_ns())
                    {
                        Ok(outcome) => {
                            retry_delay = retry_initial;
                            consecutive_retries = 0;
                            let snapshot =
                                worker.as_ref().expect("successful worker pass").status();
                            if let Ok(mut status) = worker_status.lock() {
                                apply_worker_snapshot(
                                    &mut status,
                                    snapshot,
                                    &reconnect_totals,
                                    outcome.is_some(),
                                );
                            }
                            if requested {
                                let target = *drain_target.get_or_insert(snapshot.head_abs);
                                if snapshot.cursor_abs >= target
                                    && snapshot.remote_pending == 0
                                    && snapshot.pending_part_count == 0
                                {
                                    break;
                                }
                                continue;
                            }
                        }
                        Err(error) => {
                            let transient = matches!(error, PersistenceError::Commit(_));
                            apply_worker_error(
                                &worker_status,
                                &error,
                                &mut consecutive_retries,
                                retry_max_attempts,
                            );
                            if !transient || consecutive_retries >= retry_max_attempts {
                                break;
                            }
                            if let Some(failed_worker) = worker.as_ref() {
                                reconnect_totals.accumulate(failed_worker.status());
                            }
                            worker = None;
                            std::thread::park_timeout(retry_delay);
                            retry_delay = retry_delay
                                .checked_mul(retry_multiplier)
                                .unwrap_or(retry_max)
                                .min(retry_max);
                            continue;
                        }
                    }
                    std::thread::park_timeout(interval);
                }
            })
            .map_err(|error| {
                PersistenceError::Commit(format!("spawn OpenOT persistence worker: {error}"))
            })?;
        Ok(Some(Self {
            stop,
            drain_expired,
            shutdown_deadline,
            status,
            thread: Some(thread),
            shutdown_timeout: Duration::from_millis(config.persistence.shutdown_timeout_ms),
        }))
    }

    /// Rejects enabled persistence on hosts without the OpenOT shared-memory transport.
    #[cfg(not(unix))]
    pub fn start(
        config: &OpenOtTelemetryConfig,
        _bundle_root: &Path,
    ) -> Result<Option<Self>, PersistenceError> {
        if config.persistence.enabled {
            return Err(PersistenceError::BackendUnavailable(
                "OpenOT persistence requires the Unix shared-memory transport in this build"
                    .to_string(),
            ));
        }
        Ok(None)
    }

    /// Returns a point-in-time status snapshot.
    #[must_use]
    pub fn status(&self) -> OpenOtPersistenceStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or(OpenOtPersistenceStatus {
                state: OpenOtPersistenceState::Faulted,
                backend: None,
                schema_version: None,
                documents_read: 0,
                documents_committed: 0,
                documents_duplicated: 0,
                remote_pending: 0,
                projection_rows_committed: 0,
                unclassified_event_count: 0,
                reconciled_part_count: 0,
                pending_part_count: 0,
                documents_retried: 0,
                pending: 0,
                rejected: 0,
                unresolved: 0,
                loss_range_count: 0,
                lost_record_count: 0,
                cursor_abs: 0,
                head_abs: 0,
                last_success_time_ns: None,
                last_error: Some("persistence status lock poisoned".to_string()),
            })
    }

    /// Shares the live status projection with the runtime control boundary.
    #[must_use]
    pub fn status_handle(&self) -> Arc<Mutex<OpenOtPersistenceStatus>> {
        Arc::clone(&self.status)
    }

    /// Requests shutdown and waits for the worker thread to exit.
    pub fn shutdown(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        let deadline = std::time::Instant::now() + self.shutdown_timeout;
        if let Ok(mut requested_deadline) = self.shutdown_deadline.lock() {
            *requested_deadline = Some(deadline);
        }
        self.stop.store(true, Ordering::Release);
        let mut timed_out = false;
        thread.thread().unpark();
        while !thread.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        if thread.is_finished() {
            let _ = thread.join();
        } else {
            timed_out = true;
        }
        if let Ok(mut status) = self.status.lock() {
            if timed_out || self.drain_expired.load(Ordering::Acquire) {
                status.state = OpenOtPersistenceState::Faulted;
                status.last_error = Some(format!(
                    "OpenOT persistence shutdown timed out with {} source bytes and {} remote documents pending",
                    status.pending, status.remote_pending
                ));
            } else {
                status.state = OpenOtPersistenceState::Stopped;
            }
        }
    }
}

#[cfg(unix)]
#[derive(Default)]
struct ReconnectTotals {
    documents_read: u64,
    documents_committed: u64,
    documents_duplicated: u64,
    projection_rows_committed: u64,
    unclassified_event_count: u64,
    reconciled_part_count: u64,
    rejected: u64,
    unresolved: u64,
    loss_range_count: u64,
    lost_record_count: u64,
}

#[cfg(unix)]
impl ReconnectTotals {
    fn accumulate(&mut self, status: &super::worker::OpenOtPersistenceWorkerStatus) {
        self.documents_read = self.documents_read.saturating_add(status.documents_read);
        self.documents_committed = self
            .documents_committed
            .saturating_add(status.documents_committed);
        self.documents_duplicated = self
            .documents_duplicated
            .saturating_add(status.documents_duplicated);
        self.projection_rows_committed = self
            .projection_rows_committed
            .saturating_add(status.projection_rows_committed);
        self.unclassified_event_count = self
            .unclassified_event_count
            .saturating_add(status.unclassified_event_count);
        self.reconciled_part_count = self
            .reconciled_part_count
            .saturating_add(status.reconciled_part_count);
        self.rejected = self.rejected.saturating_add(status.rejected);
        self.unresolved = self.unresolved.saturating_add(status.unresolved);
        self.loss_range_count = self
            .loss_range_count
            .saturating_add(status.loss_range_count);
        self.lost_record_count = self
            .lost_record_count
            .saturating_add(status.lost_record_count);
    }
}

#[cfg(unix)]
fn apply_worker_snapshot(
    status: &mut OpenOtPersistenceStatus,
    snapshot: &super::worker::OpenOtPersistenceWorkerStatus,
    reconnect: &ReconnectTotals,
    committed_this_pass: bool,
) {
    status.documents_read = reconnect
        .documents_read
        .saturating_add(snapshot.documents_read);
    status.documents_committed = reconnect
        .documents_committed
        .saturating_add(snapshot.documents_committed);
    status.documents_duplicated = reconnect
        .documents_duplicated
        .saturating_add(snapshot.documents_duplicated);
    status.remote_pending = snapshot.remote_pending;
    status.projection_rows_committed = reconnect
        .projection_rows_committed
        .saturating_add(snapshot.projection_rows_committed);
    status.unclassified_event_count = reconnect
        .unclassified_event_count
        .saturating_add(snapshot.unclassified_event_count);
    status.reconciled_part_count = reconnect
        .reconciled_part_count
        .saturating_add(snapshot.reconciled_part_count);
    status.pending_part_count = snapshot.pending_part_count;
    status.rejected = reconnect.rejected.saturating_add(snapshot.rejected);
    status.unresolved = reconnect.unresolved.saturating_add(snapshot.unresolved);
    status.loss_range_count = reconnect
        .loss_range_count
        .saturating_add(snapshot.loss_range_count);
    status.lost_record_count = reconnect
        .lost_record_count
        .saturating_add(snapshot.lost_record_count);
    status.cursor_abs = snapshot.cursor_abs;
    status.head_abs = snapshot.head_abs;
    status.pending = snapshot.head_abs.saturating_sub(snapshot.cursor_abs);
    status.state = if status.unresolved > 0 || status.loss_range_count > 0 {
        OpenOtPersistenceState::Degraded
    } else if status.pending > 0 || status.remote_pending > 0 {
        OpenOtPersistenceState::CatchingUp
    } else {
        OpenOtPersistenceState::Ready
    };
    if committed_this_pass {
        status.last_success_time_ns = Some(unix_time_ns());
    }
    status.last_error = None;
}

impl Drop for OpenOtPersistenceService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(unix)]
fn unix_time_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(all(test, unix))]
#[path = "service_tests.rs"]
mod tests;
