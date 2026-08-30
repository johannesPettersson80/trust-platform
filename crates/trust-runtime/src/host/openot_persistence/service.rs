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
) -> Result<RuntimePersistenceWorker, PersistenceError> {
    let consumer = OpenOtPersistenceConsumer::new(definition.clone(), None)?;
    let sink = OpenOtDocumentSink::open_with_definitions(
        persistence,
        bundle_root,
        std::slice::from_ref(definition),
    )?;
    let source = SharedMemoryOpenOtSource::open_with_limits(
        source_path,
        persistence.batch_size,
        persistence.queue_capacity,
    )?;
    Ok(OpenOtPersistenceWorker::new(source, consumer, sink))
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
    /// Compatible truST-owned schema version, absent when disabled.
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
        let definition_path = bundle_root.join("openot-definition.json");
        let definition_bytes = std::fs::read(&definition_path).map_err(|error| {
            PersistenceError::InvalidConfig(format!(
                "read compiled OpenOT definition '{}': {error}",
                definition_path.display()
            ))
        })?;
        let definition: open_ot_definition::DefinitionFile =
            serde_json::from_slice(&definition_bytes).map_err(|error| {
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
        let persistence_config = config.persistence.clone();
        let reconnect_bundle_root = bundle_root.to_path_buf();
        let reconnect_source_path = source_path.clone();
        let reconnect_definition = definition.clone();
        let initial_worker =
            open_runtime_worker(&persistence_config, bundle_root, &source_path, &definition)?;
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(OpenOtPersistenceStatus {
            state: OpenOtPersistenceState::Starting,
            backend: config
                .persistence
                .backend
                .map(|backend| backend.as_str().to_string()),
            schema_version: Some(3),
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
        let worker_status = Arc::clone(&status);
        let interval = Duration::from_millis(config.persistence.flush_interval_ms);
        let retry_initial = Duration::from_millis(config.persistence.retry_initial_ms);
        let retry_max = Duration::from_millis(config.persistence.retry_max_ms);
        let retry_multiplier = u32::from(config.persistence.retry_multiplier);
        let retry_max_attempts = config.persistence.retry_max_attempts;
        let thread = std::thread::Builder::new()
            .name("trust-openot-persistence".to_string())
            .spawn(move || {
                let mut worker = Some(initial_worker);
                let mut retry_delay = retry_initial;
                let mut consecutive_retries = 0u32;
                let mut committed_before_reconnect = 0u64;
                let mut duplicated_before_reconnect = 0u64;
                let mut projection_rows_before_reconnect = 0u64;
                let mut unclassified_before_reconnect = 0u64;
                let mut reconciled_parts_before_reconnect = 0u64;
                while !worker_stop.load(Ordering::Acquire) {
                    if worker.is_none() {
                        match open_runtime_worker(
                            &persistence_config,
                            &reconnect_bundle_root,
                            &reconnect_source_path,
                            &reconnect_definition,
                        ) {
                            Ok(reconnected) => {
                                worker = Some(reconnected);
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
                        Ok(_) => {
                            retry_delay = retry_initial;
                            consecutive_retries = 0;
                            let snapshot =
                                worker.as_ref().expect("successful worker pass").status();
                            if let Ok(mut status) = worker_status.lock() {
                                apply_worker_snapshot(
                                    &mut status,
                                    snapshot,
                                    committed_before_reconnect,
                                    duplicated_before_reconnect,
                                    projection_rows_before_reconnect,
                                    unclassified_before_reconnect,
                                    reconciled_parts_before_reconnect,
                                );
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
                                committed_before_reconnect = committed_before_reconnect
                                    .saturating_add(failed_worker.status().documents_committed);
                                duplicated_before_reconnect = duplicated_before_reconnect
                                    .saturating_add(failed_worker.status().documents_duplicated);
                                projection_rows_before_reconnect = projection_rows_before_reconnect
                                    .saturating_add(
                                        failed_worker.status().projection_rows_committed,
                                    );
                                unclassified_before_reconnect = unclassified_before_reconnect
                                    .saturating_add(
                                        failed_worker.status().unclassified_event_count,
                                    );
                                reconciled_parts_before_reconnect =
                                    reconciled_parts_before_reconnect.saturating_add(
                                        failed_worker.status().reconciled_part_count,
                                    );
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
        self.stop.store(true, Ordering::Release);
        let mut timed_out = false;
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let deadline = std::time::Instant::now() + self.shutdown_timeout;
            while !thread.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            if thread.is_finished() {
                let _ = thread.join();
            } else {
                timed_out = true;
            }
        }
        if let Ok(mut status) = self.status.lock() {
            if timed_out {
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
fn apply_worker_snapshot(
    status: &mut OpenOtPersistenceStatus,
    snapshot: &super::worker::OpenOtPersistenceWorkerStatus,
    committed_before_reconnect: u64,
    duplicated_before_reconnect: u64,
    projection_rows_before_reconnect: u64,
    unclassified_before_reconnect: u64,
    reconciled_parts_before_reconnect: u64,
) {
    status.documents_read = snapshot.documents_read;
    status.documents_committed =
        committed_before_reconnect.saturating_add(snapshot.documents_committed);
    status.documents_duplicated =
        duplicated_before_reconnect.saturating_add(snapshot.documents_duplicated);
    status.remote_pending = snapshot.remote_pending;
    status.projection_rows_committed =
        projection_rows_before_reconnect.saturating_add(snapshot.projection_rows_committed);
    status.unclassified_event_count =
        unclassified_before_reconnect.saturating_add(snapshot.unclassified_event_count);
    status.reconciled_part_count =
        reconciled_parts_before_reconnect.saturating_add(snapshot.reconciled_part_count);
    status.pending_part_count = snapshot.pending_part_count;
    status.rejected = snapshot.rejected;
    status.unresolved = snapshot.unresolved;
    status.loss_range_count = snapshot.loss_range_count;
    status.lost_record_count = snapshot.lost_record_count;
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
    status.last_success_time_ns = Some(unix_time_ns());
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
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use open_ot_carriage::registry::{
        EVENT_HEARTBEAT, EVENT_VALUE_CHANGED, KEY_NEW_VALUE, KEY_VALUE_ID, TY_BOOL, TY_UDINT,
    };
    use open_ot_carriage::wire::{Record, Slot};
    use open_ot_definition::sample_definition;
    use open_ot_shm::SharedRecordPublisher;

    use super::*;
    use crate::config::{
        OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtSqlitePersistenceConfig,
    };
    #[cfg(feature = "openot-real-database-tests")]
    use crate::config::{OpenOtPersistenceTlsMode, OpenOtPostgreSqlPersistenceConfig};

    #[test]
    fn operator_status_redacts_backend_secrets_and_sensitive_paths() {
        let secret = "password=plant-secret token=operator-token /private/customer/history.db";
        for error in [
            PersistenceError::InvalidConfig(secret.to_string()),
            PersistenceError::Commit(secret.to_string()),
            PersistenceError::CapacityExhausted(secret.to_string()),
        ] {
            let projected = redacted_error(&error);
            assert!(!projected.contains("plant-secret"));
            assert!(!projected.contains("operator-token"));
            assert!(!projected.contains("/private/customer"));
        }
    }

    #[test]
    fn enabled_service_persists_shared_memory_record_off_thread_and_stops() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-openot-persistence-service-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("root");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .expect("secure root");
        }
        let shm_path = root.join("openot.shm");
        let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
        publisher
            .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
            .expect("publish");
        std::fs::write(
            root.join("openot-definition.json"),
            serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
        )
        .expect("write definition");
        let mut config = OpenOtTelemetryConfig {
            enabled: true,
            path: std::path::PathBuf::from("openot.shm"),
            ..OpenOtTelemetryConfig::default()
        };
        config.persistence = OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            flush_interval_ms: 10,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: std::path::PathBuf::from("openot.sqlite3"),
            }),
            ..OpenOtPersistenceConfig::default()
        };

        let mut service = OpenOtPersistenceService::start(&config, &root)
            .expect("start service")
            .expect("enabled service");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while service.status().documents_committed < 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(service.status().documents_committed, 1);
        assert_eq!(service.status().state, OpenOtPersistenceState::Ready);
        service.shutdown();
        assert_eq!(service.status().state, OpenOtPersistenceState::Stopped);
        assert!(root.join("openot.sqlite3").is_file());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn service_supplies_compiled_definition_to_typed_database_projection() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-logging-service-definition-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("root");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure root");
        let definition = sample_definition();
        std::fs::write(
            root.join("openot-definition.json"),
            serde_json::to_vec_pretty(&definition).expect("serialize definition"),
        )
        .expect("write definition");
        let shm_path = root.join("openot.shm");
        let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
        let mut value = Record::new(11, 1, 0, 66, EVENT_VALUE_CHANGED);
        value.slots = vec![
            Slot::new(KEY_VALUE_ID, TY_UDINT, 2003u32.to_le_bytes()),
            Slot::new(KEY_NEW_VALUE, TY_BOOL, [1]),
        ];
        publisher
            .append_record(&value)
            .expect("publish typed value");
        let config = OpenOtTelemetryConfig {
            enabled: true,
            path: "openot.shm".into(),
            persistence: OpenOtPersistenceConfig {
                enabled: true,
                backend: Some(OpenOtPersistenceBackend::Sqlite),
                flush_interval_ms: 10,
                retry_max_attempts: 2,
                sqlite: Some(OpenOtSqlitePersistenceConfig {
                    path: "trust-logging.sqlite3".into(),
                }),
                ..OpenOtPersistenceConfig::default()
            },
            ..OpenOtTelemetryConfig::default()
        };

        let mut service = OpenOtPersistenceService::start(&config, &root)
            .expect("start typed logging service")
            .expect("enabled typed logging service");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while service.status().documents_committed < 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let status = service.status();
        service.shutdown();
        assert_eq!(
            status.documents_committed, 1,
            "typed persistence status: {status:?}"
        );
        let database = rusqlite::Connection::open(root.join("trust-logging.sqlite3"))
            .expect("inspect typed logging database");
        let stored: (String, bool) = database
            .query_row(
                "SELECT value_name,boolean_value FROM logged_values",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query service-projected value");
        assert_eq!(stored, ("Enabled".to_string(), true));
        let projection_rows: i64 = database
            .query_row(
                "SELECT (SELECT COUNT(*) FROM event_log) + (SELECT COUNT(*) FROM logged_values)",
                [],
                |row| row.get(0),
            )
            .expect("count event and value projections");
        assert_eq!(status.projection_rows_committed, projection_rows as u64);
        assert_eq!(status.projection_rows_committed, 2);
        assert_eq!(status.unclassified_event_count, 0);
        assert_eq!(status.reconciled_part_count, 0);
        assert_eq!(status.pending_part_count, 0);
        drop(database);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn sqlite_service_restart_uses_durable_checkpoint_and_catches_up_once() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-openot-sqlite-service-restart-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .expect("secure root");
        }
        let shm_path = root.join("openot.shm");
        let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
        publisher
            .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
            .expect("publish baseline");
        std::fs::write(
            root.join("openot-definition.json"),
            serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
        )
        .expect("write definition");
        let mut config = OpenOtTelemetryConfig {
            enabled: true,
            path: std::path::PathBuf::from("openot.shm"),
            ..OpenOtTelemetryConfig::default()
        };
        config.persistence = OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            flush_interval_ms: 10,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: "history/openot.sqlite3".into(),
            }),
            ..OpenOtPersistenceConfig::default()
        };

        let mut first = OpenOtPersistenceService::start(&config, &root)
            .expect("start first service")
            .expect("enabled first service");
        let first_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while first.status().documents_committed < 1 && std::time::Instant::now() < first_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(first.status().documents_committed, 1);
        let baseline_cursor = first.status().cursor_abs;
        first.shutdown();

        publisher
            .append_record(&Record::new(12, 1, 1, 7, EVENT_HEARTBEAT))
            .expect("publish while stopped 1");
        publisher
            .append_record(&Record::new(13, 1, 2, 7, EVENT_HEARTBEAT))
            .expect("publish while stopped 2");
        let mut second = OpenOtPersistenceService::start(&config, &root)
            .expect("restart service")
            .expect("enabled restarted service");
        let second_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while (second.status().state != OpenOtPersistenceState::Ready
            || second.status().cursor_abs <= baseline_cursor)
            && std::time::Instant::now() < second_deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        let second_status = second.status();
        assert_eq!(second_status.state, OpenOtPersistenceState::Ready);
        assert_eq!(second_status.cursor_abs, second_status.head_abs);
        assert_eq!(second_status.documents_committed, 2);
        second.shutdown();

        let database = rusqlite::Connection::open(root.join("history/openot.sqlite3"))
            .expect("inspect restart database");
        let events: i64 = database
            .query_row(
                "SELECT COUNT(*) FROM logging_records WHERE document_kind='event'",
                [],
                |row| row.get(0),
            )
            .expect("event count");
        assert_eq!(events, 3);
        drop(database);

        let mut third = OpenOtPersistenceService::start(&config, &root)
            .expect("start caught-up service")
            .expect("enabled caught-up service");
        let third_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while third.status().state != OpenOtPersistenceState::Ready
            && std::time::Instant::now() < third_deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(third.status().documents_committed, 0);
        assert_eq!(third.status().cursor_abs, third.status().head_abs);
        third.shutdown();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn shutdown_does_not_wait_past_the_configured_deadline_for_a_stuck_backend_call() {
        let status = Arc::new(Mutex::new(OpenOtPersistenceStatus {
            state: OpenOtPersistenceState::Ready,
            backend: Some("sqlite".to_string()),
            schema_version: Some(2),
            documents_read: 0,
            documents_committed: 0,
            documents_duplicated: 0,
            remote_pending: 0,
            projection_rows_committed: 0,
            unclassified_event_count: 0,
            reconciled_part_count: 0,
            pending_part_count: 0,
            documents_retried: 0,
            pending: 4,
            rejected: 0,
            unresolved: 0,
            loss_range_count: 0,
            lost_record_count: 0,
            cursor_abs: 0,
            head_abs: 4,
            last_success_time_ns: None,
            last_error: None,
        }));
        let mut service = OpenOtPersistenceService {
            stop: Arc::new(AtomicBool::new(false)),
            status,
            thread: Some(std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
            })),
            shutdown_timeout: Duration::from_millis(20),
        };
        let started = std::time::Instant::now();

        service.shutdown();

        assert!(started.elapsed() < Duration::from_millis(200));
        let status = service.status();
        assert_eq!(status.state, OpenOtPersistenceState::Faulted);
        assert_eq!(status.pending, 4);
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[test]
    fn postgresql_service_reconnects_and_catches_up_after_real_server_restart() {
        struct RestartGuard(String);
        impl Drop for RestartGuard {
            fn drop(&mut self) {
                let _ = std::process::Command::new("docker")
                    .args(["start", self.0.as_str()])
                    .status();
            }
        }

        let container = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CONTAINER")
            .expect("real PostgreSQL container name");
        let ca = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA").expect("PostgreSQL CA");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-openot-postgresql-reconnect-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("root");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .expect("secure root");
        }
        let shm_path = root.join("openot.shm");
        let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
        publisher
            .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
            .expect("publish baseline");
        std::fs::write(
            root.join("openot-definition.json"),
            serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
        )
        .expect("write definition");
        let mut config = OpenOtTelemetryConfig {
            enabled: true,
            path: std::path::PathBuf::from("openot.shm"),
            ..OpenOtTelemetryConfig::default()
        };
        config.persistence = OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::PostgreSql),
            flush_interval_ms: 10,
            retry_initial_ms: 20,
            retry_max_ms: 20,
            retry_multiplier: 1,
            retry_max_attempts: 200,
            postgresql: Some(OpenOtPostgreSqlPersistenceConfig {
                connection_url_env: "TRUST_TEST_OPENOT_POSTGRES_URL".into(),
                schema: format!("openot_reconnect_{}_{}", std::process::id(), stamp).into(),
                tls: OpenOtPersistenceTlsMode::Require,
                ca_cert_path: Some(ca.into()),
            }),
            ..OpenOtPersistenceConfig::default()
        };
        let mut service = OpenOtPersistenceService::start(&config, &root)
            .expect("start PostgreSQL service")
            .expect("enabled service");
        wait_for_status(&service, Duration::from_secs(5), |status| {
            status.state == OpenOtPersistenceState::Ready && status.cursor_abs > 0
        });

        let stopped = std::process::Command::new("docker")
            .args(["stop", container.as_str()])
            .status()
            .expect("stop real PostgreSQL");
        assert!(stopped.success());
        let _restart_guard = RestartGuard(container.clone());
        publisher
            .append_record(&Record::new(11, 2, 0, 7, EVENT_HEARTBEAT))
            .expect("publish during outage");
        wait_for_status(&service, Duration::from_secs(5), |status| {
            status.state == OpenOtPersistenceState::Retrying
        });
        let started = std::process::Command::new("docker")
            .args(["start", container.as_str()])
            .status()
            .expect("restart real PostgreSQL");
        assert!(started.success());

        wait_for_status(&service, Duration::from_secs(15), |status| {
            status.state == OpenOtPersistenceState::Ready
                && status.cursor_abs == status.head_abs
                && status.cursor_abs > 64
        });
        assert_eq!(
            service.status().documents_committed,
            2,
            "reconnection must preserve cumulative service counters"
        );
        service.shutdown();
        std::fs::remove_dir_all(root).ok();
    }

    #[cfg(feature = "openot-real-database-tests")]
    fn wait_for_status(
        service: &OpenOtPersistenceService,
        timeout: Duration,
        predicate: impl Fn(&OpenOtPersistenceStatus) -> bool,
    ) {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            let status = service.status();
            if predicate(&status) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "OpenOT persistence status deadline expired: {:#?}",
            service.status()
        );
    }

    #[test]
    fn unresolved_document_keeps_caught_up_service_degraded() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-openot-persistence-degraded-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("root");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .expect("secure root");
        }
        let shm_path = root.join("openot.shm");
        let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
        publisher
            .append_record(&Record::new(11, 1, 0, 7, 0xFFFF))
            .expect("publish unresolved event");
        std::fs::write(
            root.join("openot-definition.json"),
            serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
        )
        .expect("write definition");
        let mut config = OpenOtTelemetryConfig {
            enabled: true,
            path: std::path::PathBuf::from("openot.shm"),
            ..OpenOtTelemetryConfig::default()
        };
        config.persistence = OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            flush_interval_ms: 10,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: std::path::PathBuf::from("openot.sqlite3"),
            }),
            ..OpenOtPersistenceConfig::default()
        };
        let mut service = OpenOtPersistenceService::start(&config, &root)
            .expect("start service")
            .expect("enabled service");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while service.status().documents_committed < 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(service.status().unresolved, 1);
        assert_eq!(service.status().state, OpenOtPersistenceState::Degraded);
        service.shutdown();
        std::fs::remove_dir_all(root).ok();
    }
}
