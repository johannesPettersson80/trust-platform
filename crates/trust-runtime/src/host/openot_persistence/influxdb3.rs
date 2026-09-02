use std::{fs, path::Path};

use rusqlite::{params, Connection, OptionalExtension};
use ureq::{
    tls::{Certificate, RootCerts, TlsConfig},
    Agent,
};

use super::contracts::LOGGING_SCHEMA_GENERATION;
use super::projection::LoggingProjector;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

const MAX_DELIVERY_PARTS_PER_PASS: i64 = 2_048;
const RECONCILIATION_POINTS_PER_QUERY: usize = 32;

/// InfluxDB 3 HTTP adapter with a mandatory durable SQLite delivery spool.
pub struct InfluxDb3DocumentSink {
    spool: Connection,
    max_bytes: u64,
    agent: Agent,
    host: String,
    token: String,
    database: String,
    projector: LoggingProjector,
    query_ca_pem: Vec<u8>,
}

impl std::fmt::Debug for InfluxDb3DocumentSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InfluxDb3DocumentSink")
            .field("host", &self.host)
            .field("database", &self.database)
            .field("token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl InfluxDb3DocumentSink {
    /// Returns the compatible truST-owned durable spool schema version.
    pub fn schema_version(&self) -> Result<u32, PersistenceError> {
        self.spool
            .query_row(
                "SELECT version FROM logging_schema WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .map_err(spool_error("read spool schema version"))
    }

    /// Opens the durable spool and CA-authenticated InfluxDB 3 connection.
    pub fn open(
        host: &str,
        token: &str,
        database: &str,
        spool_path: &Path,
        ca_cert_path: &Path,
    ) -> Result<Self, PersistenceError> {
        Self::open_bounded(host, token, database, spool_path, ca_cert_path, u64::MAX)
    }

    /// Opens the adapter with an explicit durable-spool byte ceiling.
    pub fn open_bounded(
        host: &str,
        token: &str,
        database: &str,
        spool_path: &Path,
        ca_cert_path: &Path,
        max_bytes: u64,
    ) -> Result<Self, PersistenceError> {
        Self::open_bounded_with_definitions(
            host,
            token,
            database,
            spool_path,
            ca_cert_path,
            max_bytes,
            Vec::new(),
        )
    }

    #[doc(hidden)]
    pub fn open_bounded_with_definitions(
        host: &str,
        token: &str,
        database: &str,
        spool_path: &Path,
        ca_cert_path: &Path,
        max_bytes: u64,
        definitions: Vec<open_ot_definition::DefinitionFile>,
    ) -> Result<Self, PersistenceError> {
        if !host.starts_with("https://") {
            return Err(PersistenceError::InvalidConfig(
                "InfluxDB 3 host must use https://".to_string(),
            ));
        }
        validate_database(database)?;
        super::contracts::ensure_private_parent(spool_path, "InfluxDB 3 spool")?;
        let ca_pem = fs::read(ca_cert_path).map_err(|e| influx_error("read CA certificate", e))?;
        let agent = tls_agent(&ca_pem)?;
        let projector = LoggingProjector::new(definitions)?;
        let spool = Connection::open(spool_path)
            .map_err(|e| PersistenceError::Commit(format!("InfluxDB 3 open spool: {e}")))?;
        initialize_spool_schema(&spool)?;
        if logical_spool_bytes(&spool)? > max_bytes {
            return Err(PersistenceError::InvalidConfig(format!(
                "InfluxDB 3 spool schema exceeds configured max_bytes {max_bytes}"
            )));
        }
        let sink = Self {
            spool,
            max_bytes,
            agent,
            host: host.trim_end_matches('/').to_string(),
            token: token.to_string(),
            database: database.to_string(),
            projector,
            query_ca_pem: ca_pem,
        };
        sink.authorized_get("/health")?;
        Ok(sink)
    }

    /// Attempts delivery of all accepted but unacknowledged spool entries.
    pub fn flush_pending(&mut self) -> Result<usize, PersistenceError> {
        let mut statement = self
            .spool
            .prepare("SELECT document_identity,part_id,line_protocol FROM logging_delivery_spool WHERE delivered=0 ORDER BY document_identity,part_ordinal LIMIT ?1")
            .map_err(spool_error("prepare pending delivery"))?;
        let rows = statement
            .query_map([MAX_DELIVERY_PARTS_PER_PASS], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(spool_error("read pending delivery"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(spool_error("decode pending delivery"))?;
        drop(statement);
        if rows.is_empty() {
            return Ok(0);
        }
        let body = rows
            .iter()
            .map(|(_, _, line)| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let url = format!(
            "{}/api/v3/write_lp?db={}&precision=nanosecond&accept_partial=false&no_sync=false",
            self.host,
            urlencoding::encode(&self.database)
        );
        self.agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Content-Type", "text/plain; charset=utf-8")
            .send(body)
            .map_err(http_error("write WAL-synchronous line protocol"))?;
        let verified = self.reconcile_remote_parts(&rows)?;
        let transaction = self
            .spool
            .transaction()
            .map_err(spool_error("begin delivery acknowledgement"))?;
        for part_id in &verified {
            transaction
                .execute(
                    "UPDATE logging_delivery_spool SET delivered=1 WHERE part_id=?1",
                    [part_id],
                )
                .map_err(spool_error("mark delivered"))?;
        }
        transaction
            .commit()
            .map_err(spool_error("commit delivery acknowledgement"))?;
        if verified.len() != rows.len() {
            return Err(PersistenceError::Commit(format!(
                "InfluxDB 3 reconciliation found {} of {} expected delivery parts",
                verified.len(),
                rows.len()
            )));
        }
        Ok(verified.len())
    }

    fn reconcile_remote_parts(
        &self,
        rows: &[(String, String, String)],
    ) -> Result<Vec<String>, PersistenceError> {
        let agent = tls_agent(&self.query_ca_pem)?;
        let mut groups = std::collections::BTreeMap::<String, Vec<(String, String)>>::new();
        for (_, part_id, line) in rows {
            let (series, remainder) = line.split_once(' ').ok_or_else(|| {
                PersistenceError::Commit("InfluxDB 3 spooled line omitted fields".into())
            })?;
            let measurement = series.split(',').next().unwrap_or_default();
            let identity = series
                .split(',')
                .skip(1)
                .find_map(|tag| {
                    tag.strip_prefix("record_id=")
                        .or_else(|| tag.strip_prefix("identity_key="))
                })
                .ok_or_else(|| {
                    PersistenceError::Commit("InfluxDB 3 spooled line omitted identity tag".into())
                })?;
            let identity_column = if series.contains(",record_id=") {
                "record_id"
            } else {
                "identity_key"
            };
            let timestamp = remainder
                .rsplit_once(' ')
                .map(|(_, time)| time)
                .ok_or_else(|| {
                    PersistenceError::Commit("InfluxDB 3 spooled line omitted timestamp".into())
                })?;
            groups.entry(measurement.to_string()).or_default().push((
                part_id.clone(),
                format!(
                    "({identity_column}='{}' AND time=to_timestamp_nanos({timestamp}))",
                    identity.replace('\'', "''")
                ),
            ));
        }
        let mut verified = Vec::with_capacity(rows.len());
        for (measurement, points) in groups {
            for chunk in points.chunks(RECONCILIATION_POINTS_PER_QUERY) {
                let query = format!(
                    "SELECT COUNT(*) AS count FROM {measurement} WHERE {}",
                    chunk
                        .iter()
                        .map(|(_, predicate)| predicate.as_str())
                        .collect::<Vec<_>>()
                        .join(" OR ")
                );
                let url = format!(
                    "{}/api/v3/query_sql?db={}&q={}&format=json",
                    self.host,
                    urlencoding::encode(&self.database),
                    urlencoding::encode(&query)
                );
                let mut response = agent
                    .get(&url)
                    .header("Authorization", &format!("Bearer {}", self.token))
                    .call()
                    .map_err(http_error("reconcile delivery measurement"))?;
                let body = response
                    .body_mut()
                    .read_to_string()
                    .map_err(|error| transport_error("read reconciliation query", error))?;
                let result: Vec<serde_json::Value> =
                    serde_json::from_str(&body).map_err(|error| {
                        PersistenceError::Commit(format!(
                            "InfluxDB 3 decode reconciliation query: {error}"
                        ))
                    })?;
                let count = result
                    .first()
                    .and_then(|row| row.get("count"))
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                if count == i64::try_from(chunk.len()).unwrap_or(i64::MAX) {
                    verified.extend(chunk.iter().map(|(part_id, _)| part_id.clone()));
                }
            }
        }
        Ok(verified)
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn server_version(&self) -> Result<String, PersistenceError> {
        let body = self.authorized_get("/ping")?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| PersistenceError::Commit(format!("InfluxDB 3 decode ping: {e}")))?;
        value
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| PersistenceError::Commit("InfluxDB 3 ping omitted version".to_string()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn remote_document_count_for_run(
        &self,
        run_id: u64,
    ) -> Result<i64, PersistenceError> {
        let query = format!(
            "SELECT COUNT(*) AS count FROM logging_records WHERE identity_key LIKE '%:{run_id:016x}:%'"
        );
        let url = format!(
            "{}/api/v3/query_sql?db={}&q={}&format=json",
            self.host,
            urlencoding::encode(&self.database),
            urlencoding::encode(&query)
        );
        // Query through a fresh TLS client. InfluxDB 3 can keep the writer's
        // persistent connection on a pre-write query snapshot even after the
        // WAL-synchronous acknowledgement; external readers use a separate
        // connection and are the visibility boundary this proof exercises.
        let query_agent = tls_agent(&self.query_ca_pem)?;
        let mut response = query_agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(http_error("query documents"))?;
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| transport_error("read query response", error))?;
        let rows: Vec<serde_json::Value> = serde_json::from_str(&body)
            .map_err(|e| PersistenceError::Commit(format!("InfluxDB 3 decode query: {e}")))?;
        rows.first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                PersistenceError::Commit("InfluxDB 3 count query omitted count".to_string())
            })
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn remote_measurement_count_for_run(
        &self,
        measurement: &str,
        run_id: u64,
    ) -> Result<i64, PersistenceError> {
        if !matches!(
            measurement,
            "event_log"
                | "logged_values"
                | "alarm_history"
                | "message_log"
                | "state_history"
                | "batch_history"
                | "recipe_history"
                | "material_additions"
                | "operator_activity"
                | "audit_log"
                | "electronic_signatures"
                | "system_events"
                | "data_loss"
                | "unresolved_records"
        ) {
            return Err(PersistenceError::InvalidConfig(
                "unsupported InfluxDB test measurement".into(),
            ));
        }
        let query = format!("SELECT COUNT(*) AS count FROM {measurement} WHERE run_id='{run_id}'");
        let url = format!(
            "{}/api/v3/query_sql?db={}&q={}&format=json",
            self.host,
            urlencoding::encode(&self.database),
            urlencoding::encode(&query)
        );
        let query_agent = tls_agent(&self.query_ca_pem)?;
        let mut response = query_agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(http_error("query typed measurement"))?;
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| transport_error("read typed measurement query", error))?;
        let rows: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|error| {
            PersistenceError::Commit(format!("InfluxDB 3 decode typed query: {error}"))
        })?;
        rows.first()
            .and_then(|row| row.get("count"))
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| PersistenceError::Commit("InfluxDB 3 typed query omitted count".into()))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn remote_canonical_jsons_for_run(
        &self,
        run_id: u64,
    ) -> Result<Vec<String>, PersistenceError> {
        self.remote_canonical_jsons_where(&format!("identity_key LIKE '%:{run_id:016x}:%'"))
    }

    /// Queries one run at one receive timestamp for deterministic real-product verification.
    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn remote_canonical_jsons_for_run_at(
        &self,
        run_id: u64,
        receive_time_ns: u64,
    ) -> Result<Vec<String>, PersistenceError> {
        self.remote_canonical_jsons_where(&format!(
            "identity_key LIKE '%:{run_id:016x}:%' AND time = to_timestamp_nanos({receive_time_ns})"
        ))
    }

    /// Queries the deterministic fixture receive timestamp across its sources.
    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn remote_canonical_jsons_at(
        &self,
        receive_time_ns: u64,
    ) -> Result<Vec<String>, PersistenceError> {
        self.remote_canonical_jsons_where(&format!("time = to_timestamp_nanos({receive_time_ns})"))
    }

    #[cfg(feature = "openot-real-database-tests")]
    fn remote_canonical_jsons_where(
        &self,
        predicate: &str,
    ) -> Result<Vec<String>, PersistenceError> {
        // A WAL-synchronous write can become query-visible after the first
        // probe. Keep visibility probes distinct so neither an intermediary
        // nor the query endpoint can reuse the initial empty GET response.
        let probe = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let query = format!(
            "SELECT canonical_json FROM logging_records WHERE {predicate} ORDER BY identity_key /* visibility-probe-{probe} */"
        );
        let url = format!(
            "{}/api/v3/query_sql?db={}&q={}&format=json",
            self.host,
            urlencoding::encode(&self.database),
            urlencoding::encode(&query)
        );
        let query_agent = tls_agent(&self.query_ca_pem)?;
        let mut response = query_agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(http_error("query canonical documents"))?;
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| transport_error("read canonical query response", error))?;
        let rows: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|error| {
            PersistenceError::Commit(format!("InfluxDB 3 decode canonical query: {error}"))
        })?;
        rows.into_iter()
            .map(|row| {
                row.get("canonical_json")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| {
                        PersistenceError::Commit(
                            "InfluxDB 3 canonical query omitted canonical_json".to_string(),
                        )
                    })
            })
            .collect()
    }

    fn query_pending_count(&self) -> Result<i64, PersistenceError> {
        self.spool
            .query_row(
                "SELECT COUNT(DISTINCT document_identity) FROM logging_delivery_spool WHERE delivered=0",
                [],
                |row| row.get(0),
            )
            .map_err(spool_error("count pending entries"))
    }

    fn query_pending_part_count(&self) -> Result<i64, PersistenceError> {
        self.spool
            .query_row(
                "SELECT COUNT(*) FROM logging_delivery_spool WHERE delivered=0",
                [],
                |row| row.get(0),
            )
            .map_err(spool_error("count pending delivery parts"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn pending_count(&self) -> Result<i64, PersistenceError> {
        self.query_pending_count()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn pending_part_count(&self) -> Result<i64, PersistenceError> {
        let exists: i64 = self
            .spool
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='logging_delivery_spool'",
                [],
                |row| row.get(0),
            )
            .map_err(spool_error("inspect delivery-part schema"))?;
        if exists == 0 {
            return Ok(0);
        }
        self.query_pending_part_count()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn internal_name_counts(&self) -> Result<(i64, i64), PersistenceError> {
        let logging = self.spool.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('logging_schema','logging_records','logging_checkpoint','logging_delivery_spool')",
            [], |row| row.get(0),
        ).map_err(spool_error("inspect logging spool names"))?;
        let legacy = self.spool.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('openot_schema','openot_spool','openot_checkpoint')",
            [], |row| row.get(0),
        ).map_err(spool_error("inspect legacy spool names"))?;
        Ok((logging, legacy))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn spool_logical_bytes(&self) -> Result<u64, PersistenceError> {
        logical_spool_bytes(&self.spool)
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(&self, version: u32) -> Result<(), PersistenceError> {
        let catalog_fingerprint = spool_catalog_fingerprint(&self.spool)?;
        self.spool
            .execute(
                "INSERT INTO logging_schema(singleton,version,catalog_fingerprint) VALUES (1,?1,?2)
             ON CONFLICT(singleton) DO UPDATE SET version=excluded.version,catalog_fingerprint=excluded.catalog_fingerprint",
                rusqlite::params![version, catalog_fingerprint],
            )
            .and_then(|_| self.spool.pragma_update(None, "user_version", version))
            .map_err(spool_error("seed spool schema version"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn remove_schema_marker_for_test(&self) -> Result<(), PersistenceError> {
        self.spool
            .execute("DELETE FROM logging_schema WHERE singleton=1", [])
            .and_then(|_| self.spool.pragma_update(None, "user_version", 0))
            .map_err(spool_error("remove spool schema generation marker"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_host_for_test(&mut self, host: &str) {
        self.host = host.to_string();
    }

    fn authorized_get(&self, path: &str) -> Result<String, PersistenceError> {
        let mut response = self
            .agent
            .get(format!("{}{}", self.host, path))
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(http_error("authorized request"))?;
        response
            .body_mut()
            .read_to_string()
            .map_err(|error| transport_error("read response", error))
    }
}

fn tls_agent(ca_pem: &[u8]) -> Result<Agent, PersistenceError> {
    let ca = Certificate::from_pem(ca_pem)
        .map_err(|e| PersistenceError::Commit(format!("InfluxDB 3 parse CA certificate: {e}")))?;
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::new_with_certs(&[ca]))
        .build();
    Ok(Agent::config_builder().tls_config(tls).build().into())
}

impl DocumentSink for InfluxDb3DocumentSink {
    fn maintenance(&mut self) -> Result<usize, PersistenceError> {
        self.flush_pending()?;
        Ok(usize::try_from(self.query_pending_count()?).unwrap_or(usize::MAX))
    }

    fn maintenance_status(&mut self) -> Result<super::MaintenanceOutcome, PersistenceError> {
        let before = usize::try_from(self.query_pending_part_count()?).unwrap_or(usize::MAX);
        self.flush_pending()?;
        let pending_parts = usize::try_from(self.query_pending_part_count()?).unwrap_or(usize::MAX);
        Ok(super::MaintenanceOutcome {
            remote_pending: usize::try_from(self.query_pending_count()?).unwrap_or(usize::MAX),
            reconciled_parts: before.saturating_sub(pending_parts),
            pending_parts,
        })
    }

    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        let stored = self
            .spool
            .query_row(
                "SELECT buffer_id,run_id,cursor_abs FROM logging_checkpoint WHERE singleton=1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(spool_error("read spool checkpoint"))?;
        let Some((stored_buffer, stored_run, cursor)) = stored else {
            return Ok(None);
        };
        let stored_run = decode_u64(&stored_run, "spool checkpoint run")?;
        if stored_buffer != buffer_id || stored_run != run_id {
            return Ok(None);
        }
        let cursor_abs = decode_u64(&cursor, "spool checkpoint")?;
        Ok(Some(super::PersistenceCheckpoint {
            buffer_id,
            run_id,
            cursor_abs,
        }))
    }

    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
        let transaction = self
            .spool
            .transaction()
            .map_err(spool_error("begin spool transaction"))?;
        if let Some((buffer, run, cursor)) = transaction
            .query_row(
                "SELECT buffer_id,run_id,cursor_abs FROM logging_checkpoint WHERE singleton=1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(spool_error("read spool checkpoint"))?
        {
            let current = decode_u64(&cursor, "spool checkpoint")?;
            let current_run = decode_u64(&run, "spool checkpoint run")?;
            if buffer == batch.checkpoint.buffer_id
                && current_run == batch.checkpoint.run_id
                && batch.checkpoint.cursor_abs < current
            {
                return Err(PersistenceError::CheckpointRegression {
                    current,
                    requested: batch.checkpoint.cursor_abs,
                });
            }
        }
        let mut inserted = 0;
        let mut duplicated = 0;
        let mut projection_rows_committed = 0;
        let mut unclassified_events = 0;
        let mut unresolved_documents = 0;
        let mut loss_ranges = 0;
        let mut lost_records = 0u64;
        for document in &batch.documents {
            let projected = self.projector.project(document)?;
            let projected_row_count = projected.public_row_count();
            let has_unclassified_event = projected.has_unclassified_event();
            let special_counts = projected.loss_and_unresolved_counts()?;
            let row = &projected.canonical;
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT canonical_json FROM logging_records WHERE identity_key=?1",
                    [&row.identity_key],
                    |db_row| db_row.get(0),
                )
                .optional()
                .map_err(spool_error("check spool identity"))?;
            if let Some(existing) = existing {
                if existing == row.canonical_json {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(row.identity_key.clone()));
            }
            let line = super::influxdb3_read_model::line_protocol(&projected)?;
            transaction.execute(
                "INSERT INTO logging_records(identity_key,canonical_json,line_protocol,delivered) VALUES(?1,?2,?3,0)",
                params![&row.identity_key, &row.canonical_json, &line],
            ).map_err(spool_error("insert spool document"))?;
            for (ordinal, part) in line.lines().enumerate() {
                let part_id = format!("{}:{ordinal}", row.identity_key);
                transaction.execute(
                    "INSERT INTO logging_delivery_spool(part_id,document_identity,part_ordinal,line_protocol,delivered) VALUES(?1,?2,?3,?4,0)",
                    params![part_id, &row.identity_key, ordinal as u32, part],
                ).map_err(spool_error("insert durable delivery part"))?;
            }
            inserted += 1;
            projection_rows_committed += projected_row_count;
            unclassified_events += usize::from(has_unclassified_event);
            unresolved_documents += special_counts.0;
            loss_ranges += special_counts.1;
            lost_records = lost_records.saturating_add(special_counts.2);
        }
        transaction.execute(
            "INSERT INTO logging_checkpoint(singleton,buffer_id,run_id,cursor_abs) VALUES(1,?1,?2,?3)
             ON CONFLICT(singleton) DO UPDATE SET buffer_id=excluded.buffer_id,run_id=excluded.run_id,cursor_abs=excluded.cursor_abs",
            params![batch.checkpoint.buffer_id, batch.checkpoint.run_id.to_be_bytes().as_slice(), batch.checkpoint.cursor_abs.to_be_bytes().as_slice()],
        ).map_err(spool_error("write spool checkpoint"))?;
        if logical_spool_bytes(&transaction)? > self.max_bytes {
            return Err(PersistenceError::CapacityExhausted(format!(
                "InfluxDB 3 spool reached configured max_bytes {}",
                self.max_bytes
            )));
        }
        transaction
            .commit()
            .map_err(spool_error("commit spool acceptance"))?;
        let remote_pending = usize::try_from(self.query_pending_count()?).unwrap_or(usize::MAX);
        let pending_parts = usize::try_from(self.query_pending_part_count()?).unwrap_or(usize::MAX);
        Ok(CommitOutcome {
            inserted,
            duplicated,
            remote_pending,
            projection_rows_committed,
            unclassified_events,
            unresolved_documents,
            loss_ranges,
            lost_records,
            pending_parts,
            checkpoint: batch.checkpoint,
        })
    }
}

fn initialize_spool_schema(connection: &Connection) -> Result<(), PersistenceError> {
    let version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(spool_error("read spool schema generation"))?;
    if version == LOGGING_SCHEMA_GENERATION {
        let marker: (u32, String) = connection
            .query_row("SELECT version,catalog_fingerprint FROM logging_schema WHERE singleton=1", [], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|_| PersistenceError::Commit(
                "InfluxDB 3 incompatible pre-release spool schema; back up and recreate the development spool".into(),
            ))?;
        if marker.0 != LOGGING_SCHEMA_GENERATION {
            return Err(PersistenceError::Commit(format!(
                "InfluxDB 3 incompatible pre-release spool generation {}; back up and recreate the development spool",
                marker.0
            )));
        }
        validate_spool_schema(connection)?;
        if spool_catalog_fingerprint(connection)? != marker.1 {
            return Err(PersistenceError::Commit(
                "InfluxDB 3 incompatible pre-release spool schema: the generation-1 catalog definition changed; back up and recreate the development spool".into(),
            ));
        }
        return apply_and_verify_spool_pragmas(connection);
    }
    if version != 0 {
        return Err(PersistenceError::Commit(format!(
            "InfluxDB 3 incompatible pre-release spool generation {version}; back up and recreate the development spool"
        )));
    }
    let occupied: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND (substr(name,1,8)='logging_' OR name IN ('openot_schema','openot_spool','openot_checkpoint')) LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(spool_error("inspect spool schema ownership"))?;
    if let Some(object) = occupied {
        return Err(PersistenceError::Commit(format!(
            "InfluxDB 3 incompatible pre-release spool object {object}; back up and recreate the development spool"
        )));
    }
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         PRAGMA foreign_keys=ON;
         CREATE TABLE logging_schema(singleton INTEGER PRIMARY KEY CHECK(singleton=1),version INTEGER NOT NULL CHECK(version=1),catalog_fingerprint TEXT NOT NULL);
         INSERT INTO logging_schema(singleton,version,catalog_fingerprint) VALUES(1,1,'');
         CREATE TABLE logging_records(spool_id INTEGER PRIMARY KEY AUTOINCREMENT,identity_key TEXT NOT NULL UNIQUE,canonical_json TEXT NOT NULL,line_protocol TEXT NOT NULL,delivered INTEGER NOT NULL CHECK(delivered IN(0,1)));
         CREATE TABLE logging_delivery_spool(part_id TEXT PRIMARY KEY,document_identity TEXT NOT NULL,part_ordinal INTEGER NOT NULL,line_protocol TEXT NOT NULL,delivered INTEGER NOT NULL CHECK(delivered IN(0,1)),UNIQUE(document_identity,part_ordinal),FOREIGN KEY(document_identity) REFERENCES logging_records(identity_key));
         CREATE INDEX logging_delivery_spool_pending ON logging_delivery_spool(delivered,document_identity,part_ordinal);
         CREATE TABLE logging_checkpoint(singleton INTEGER PRIMARY KEY CHECK(singleton=1),buffer_id INTEGER NOT NULL,run_id BLOB NOT NULL CHECK(length(run_id)=8),cursor_abs BLOB NOT NULL CHECK(length(cursor_abs)=8));
         PRAGMA user_version=1;"
    ).map_err(spool_error("initialize generation-1 spool"))?;
    let catalog_fingerprint = spool_catalog_fingerprint(connection)?;
    connection
        .execute(
            "UPDATE logging_schema SET catalog_fingerprint=?1 WHERE singleton=1",
            [&catalog_fingerprint],
        )
        .map_err(spool_error("record generation-1 spool catalog fingerprint"))?;
    connection
        .execute_batch("COMMIT;")
        .map_err(spool_error("commit generation-1 spool"))?;
    validate_spool_schema(connection)?;
    apply_and_verify_spool_pragmas(connection)
}

fn apply_and_verify_spool_pragmas(connection: &Connection) -> Result<(), PersistenceError> {
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .map_err(spool_error("apply connection-local durability settings"))?;
    let foreign_keys: u32 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(spool_error("verify foreign-key mode"))?;
    let synchronous: u32 = connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .map_err(spool_error("verify synchronous mode"))?;
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(spool_error("verify journal mode"))?;
    if foreign_keys != 1 || synchronous != 2 || !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(PersistenceError::Commit(format!(
            "InfluxDB 3 spool durability settings were not applied: foreign_keys={foreign_keys}, synchronous={synchronous}, journal_mode={journal_mode}"
        )));
    }
    Ok(())
}

fn validate_spool_schema(connection: &Connection) -> Result<(), PersistenceError> {
    for object in [
        "logging_schema",
        "logging_records",
        "logging_delivery_spool",
        "logging_checkpoint",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [object],
                |row| row.get(0),
            )
            .map_err(spool_error("validate spool schema object"))?;
        if !exists {
            return Err(PersistenceError::Commit(format!(
                "InfluxDB 3 incompatible pre-release spool schema: required object {object} is missing; back up and recreate the development spool"
            )));
        }
    }
    Ok(())
}

fn spool_catalog_fingerprint(connection: &Connection) -> Result<String, PersistenceError> {
    let mut statement = connection
        .prepare(
            "SELECT type || '|' || name || '|' || tbl_name || '|' || COALESCE(sql,'') \
             FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND \
             (name LIKE 'logging_%' OR tbl_name LIKE 'logging_%')",
        )
        .map_err(spool_error("prepare spool catalog fingerprint"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(spool_error("read spool catalog fingerprint"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(spool_error("decode spool catalog fingerprint"))?;
    Ok(super::schema_contract::fingerprint(rows))
}

fn logical_spool_bytes(connection: &Connection) -> Result<u64, PersistenceError> {
    let page_count: i64 = connection
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .map_err(spool_error("read spool page count"))?;
    let page_size: i64 = connection
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(spool_error("read spool page size"))?;
    let page_count = u64::try_from(page_count)
        .map_err(|_| PersistenceError::Commit("InfluxDB 3 spool page count is negative".into()))?;
    let page_size = u64::try_from(page_size)
        .map_err(|_| PersistenceError::Commit("InfluxDB 3 spool page size is negative".into()))?;
    Ok(page_count.saturating_mul(page_size))
}

pub(super) fn canonical_line_protocol(
    row: &super::projection::DocumentRow,
) -> Result<String, PersistenceError> {
    let timestamp = decode_u64(&row.receive_time_ns, "receive time")?;
    let timestamp = i64::try_from(timestamp).map_err(|_| {
        PersistenceError::Commit("InfluxDB 3 timestamp exceeds signed nanosecond range".to_string())
    })?;
    Ok(format!(
        "logging_records,identity_key={},document_kind={} canonical_json=\"{}\",buffer_id={}u,source_id={}u {}",
        escape_tag(&row.identity_key), escape_tag(row.document_kind), escape_field(&row.canonical_json), row.buffer_id, row.source_id, timestamp
    ))
}

pub(super) fn escape_tag(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}
pub(super) fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn validate_database(value: &str) -> Result<(), PersistenceError> {
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c == '_' || c == '-' || c.is_ascii_alphanumeric())
    {
        Ok(())
    } else {
        Err(PersistenceError::InvalidConfig(
            "InfluxDB 3 database must contain only ASCII letters, digits, '_' or '-'".to_string(),
        ))
    }
}

fn decode_u64(bytes: &[u8], context: &str) -> Result<u64, PersistenceError> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        PersistenceError::Commit(format!(
            "InfluxDB 3 {context} is not an 8-byte unsigned value"
        ))
    })?;
    Ok(u64::from_be_bytes(bytes))
}

fn spool_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> PersistenceError {
    move |error| PersistenceError::Commit(format!("InfluxDB 3 {context}: {error}"))
}
fn http_error(context: &'static str) -> impl FnOnce(ureq::Error) -> PersistenceError {
    move |error| {
        let retryable = matches!(
            &error,
            ureq::Error::Io(_)
                | ureq::Error::Timeout(_)
                | ureq::Error::HostNotFound
                | ureq::Error::ConnectionFailed
                | ureq::Error::ConnectProxyFailed(_)
        );
        let message = format!("InfluxDB 3 {context}: {error}");
        if retryable {
            PersistenceError::Connection(message)
        } else {
            PersistenceError::Commit(message)
        }
    }
}
fn influx_error(context: &'static str, error: impl std::fmt::Display) -> PersistenceError {
    PersistenceError::Commit(format!("InfluxDB 3 {context}: {error}"))
}

fn transport_error(context: &'static str, error: impl std::fmt::Display) -> PersistenceError {
    PersistenceError::Connection(format!("InfluxDB 3 {context}: {error}"))
}

#[cfg(test)]
#[path = "influxdb3_tests.rs"]
mod tests;
