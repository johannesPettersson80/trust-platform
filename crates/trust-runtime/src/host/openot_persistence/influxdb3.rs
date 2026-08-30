use std::{fs, path::Path};

use rusqlite::{params, Connection, OptionalExtension};
use ureq::{
    tls::{Certificate, RootCerts, TlsConfig},
    Agent,
};

use super::projection::document_row;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

const SPOOL_SCHEMA_VERSION: u32 = 2;

/// InfluxDB 3 HTTP adapter with a mandatory durable SQLite delivery spool.
pub struct InfluxDb3DocumentSink {
    spool: Connection,
    max_bytes: u64,
    agent: Agent,
    host: String,
    token: String,
    database: String,
    #[cfg(feature = "openot-real-database-tests")]
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
        if !host.starts_with("https://") {
            return Err(PersistenceError::InvalidConfig(
                "InfluxDB 3 host must use https://".to_string(),
            ));
        }
        validate_database(database)?;
        super::contracts::ensure_private_parent(spool_path, "InfluxDB 3 spool")?;
        let ca_pem = fs::read(ca_cert_path).map_err(|e| influx_error("read CA certificate", e))?;
        let agent = tls_agent(&ca_pem)?;
        let spool = Connection::open(spool_path)
            .map_err(|e| PersistenceError::Commit(format!("InfluxDB 3 open spool: {e}")))?;
        migrate_spool(&spool)?;
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
            #[cfg(feature = "openot-real-database-tests")]
            query_ca_pem: ca_pem,
        };
        sink.authorized_get("/health")?;
        Ok(sink)
    }

    /// Attempts delivery of all accepted but unacknowledged spool entries.
    pub fn flush_pending(&mut self) -> Result<usize, PersistenceError> {
        let mut statement = self
            .spool
            .prepare("SELECT identity_key,line_protocol FROM openot_spool WHERE delivered=0 ORDER BY spool_id")
            .map_err(spool_error("prepare pending delivery"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
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
            .map(|(_, line)| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let url = format!(
            "{}/api/v3/write_lp?db={}&precision=nanosecond",
            self.host,
            urlencoding::encode(&self.database)
        );
        self.agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Content-Type", "text/plain; charset=utf-8")
            .send(body)
            .map_err(http_error("write WAL-synchronous line protocol"))?;
        let transaction = self
            .spool
            .transaction()
            .map_err(spool_error("begin delivery acknowledgement"))?;
        for (identity, _) in &rows {
            transaction
                .execute(
                    "UPDATE openot_spool SET delivered=1 WHERE identity_key=?1",
                    [identity],
                )
                .map_err(spool_error("mark delivered"))?;
        }
        transaction
            .commit()
            .map_err(spool_error("commit delivery acknowledgement"))?;
        Ok(rows.len())
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
            "SELECT COUNT(*) AS count FROM openot_documents WHERE identity_key LIKE '%:{run_id:016x}:%'"
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
            .map_err(|e| influx_error("read query response", e))?;
        let rows: Vec<serde_json::Value> = serde_json::from_str(&body)
            .map_err(|e| PersistenceError::Commit(format!("InfluxDB 3 decode query: {e}")))?;
        rows.first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                PersistenceError::Commit("InfluxDB 3 count query omitted count".to_string())
            })
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
            "SELECT canonical_json FROM openot_documents WHERE {predicate} ORDER BY identity_key /* visibility-probe-{probe} */"
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
            .map_err(|error| influx_error("read canonical query response", error))?;
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
                "SELECT COUNT(*) FROM openot_spool WHERE delivered=0",
                [],
                |row| row.get(0),
            )
            .map_err(spool_error("count pending entries"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn pending_count(&self) -> Result<i64, PersistenceError> {
        self.query_pending_count()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn spool_logical_bytes(&self) -> Result<u64, PersistenceError> {
        logical_spool_bytes(&self.spool)
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn downgrade_checkpoint_to_v1_for_test(&self) -> Result<(), PersistenceError> {
        self.spool
            .execute_batch(
                "DELETE FROM openot_checkpoint;
                 ALTER TABLE openot_checkpoint DROP COLUMN run_id;
                 UPDATE openot_schema SET version=1 WHERE singleton=1;",
            )
            .map_err(spool_error("seed spool schema v1"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(&self, version: u32) -> Result<(), PersistenceError> {
        self.spool
            .execute(
                "UPDATE openot_schema SET version=?1 WHERE singleton=1",
                [version],
            )
            .map(|_| ())
            .map_err(spool_error("seed spool schema version"))
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
            .map_err(|e| influx_error("read response", e))
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

    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        let stored = self
            .spool
            .query_row(
                "SELECT buffer_id,run_id,cursor_abs FROM openot_checkpoint WHERE singleton=1",
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
                "SELECT buffer_id,run_id,cursor_abs FROM openot_checkpoint WHERE singleton=1",
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
        for document in &batch.documents {
            let row = document_row(document)?;
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT canonical_json FROM openot_spool WHERE identity_key=?1",
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
                return Err(PersistenceError::IdentityConflict(row.identity_key));
            }
            let line = line_protocol(&row)?;
            transaction.execute(
                "INSERT INTO openot_spool(identity_key,canonical_json,line_protocol,delivered) VALUES(?1,?2,?3,0)",
                params![row.identity_key, row.canonical_json, line],
            ).map_err(spool_error("insert spool document"))?;
            inserted += 1;
        }
        transaction.execute(
            "INSERT INTO openot_checkpoint(singleton,buffer_id,run_id,cursor_abs) VALUES(1,?1,?2,?3)
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
        Ok(CommitOutcome {
            inserted,
            duplicated,
            remote_pending,
            checkpoint: batch.checkpoint,
        })
    }
}

fn migrate_spool(connection: &Connection) -> Result<(), PersistenceError> {
    connection.execute_batch(&format!(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
         CREATE TABLE IF NOT EXISTS openot_schema(singleton INTEGER PRIMARY KEY CHECK(singleton=1),version INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS openot_spool(spool_id INTEGER PRIMARY KEY AUTOINCREMENT,identity_key TEXT NOT NULL UNIQUE,canonical_json TEXT NOT NULL,line_protocol TEXT NOT NULL,delivered INTEGER NOT NULL CHECK(delivered IN(0,1)));
         CREATE INDEX IF NOT EXISTS openot_spool_pending ON openot_spool(delivered,spool_id);
         CREATE TABLE IF NOT EXISTS openot_checkpoint(singleton INTEGER PRIMARY KEY CHECK(singleton=1),buffer_id INTEGER NOT NULL,run_id BLOB NOT NULL CHECK(length(run_id)=8),cursor_abs BLOB NOT NULL CHECK(length(cursor_abs)=8));
         INSERT OR IGNORE INTO openot_schema(singleton,version) VALUES(1,{SPOOL_SCHEMA_VERSION});"
    )).map_err(spool_error("migrate spool"))?;
    let version: u32 = connection
        .query_row(
            "SELECT version FROM openot_schema WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .map_err(spool_error("read spool schema version"))?;
    if version > SPOOL_SCHEMA_VERSION {
        return Err(PersistenceError::Commit(format!(
            "InfluxDB 3 spool schema {version} is newer than supported {SPOOL_SCHEMA_VERSION}"
        )));
    }
    if version == 1 {
        connection
            .execute_batch(
                "ALTER TABLE openot_checkpoint ADD COLUMN run_id BLOB NOT NULL DEFAULT X'0000000000000000' CHECK(length(run_id)=8);
                 UPDATE openot_schema SET version=2 WHERE singleton=1;",
            )
            .map_err(spool_error("migrate spool schema 1 to 2"))?;
    }
    Ok(())
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

fn line_protocol(row: &super::projection::DocumentRow) -> Result<String, PersistenceError> {
    let timestamp = decode_u64(&row.receive_time_ns, "receive time")?;
    let timestamp = i64::try_from(timestamp).map_err(|_| {
        PersistenceError::Commit("InfluxDB 3 timestamp exceeds signed nanosecond range".to_string())
    })?;
    Ok(format!(
        "openot_documents,identity_key={},document_kind={} canonical_json=\"{}\",buffer_id={}u,source_id={}u {}",
        escape_tag(&row.identity_key), escape_tag(row.document_kind), escape_field(&row.canonical_json), row.buffer_id, row.source_id, timestamp
    ))
}

fn escape_tag(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}
fn escape_field(value: &str) -> String {
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
    move |error| PersistenceError::Commit(format!("InfluxDB 3 {context}: {error}"))
}
fn influx_error(context: &'static str, error: impl std::fmt::Display) -> PersistenceError {
    PersistenceError::Commit(format!("InfluxDB 3 {context}: {error}"))
}
