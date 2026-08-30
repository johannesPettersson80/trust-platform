use open_ot_document::Document;

#[cfg(test)]
use super::projection::document_identity;

/// Durable consumer position associated with a committed document batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistenceCheckpoint {
    /// OpenOT shared-memory buffer identity.
    pub buffer_id: u32,
    /// Producer run identity; cursors are comparable only within this run.
    pub run_id: u64,
    /// Absolute carriage cursor after the committed batch.
    pub cursor_abs: u64,
}

/// Ordered resolved documents and the cursor acknowledged by their commit.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistenceBatch {
    /// Documents in source delivery order.
    pub documents: Vec<Document>,
    /// Cursor that becomes durable atomically with the documents.
    pub checkpoint: PersistenceCheckpoint,
}

/// Result of an idempotent durable batch commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitOutcome {
    /// Documents newly inserted by this commit.
    pub inserted: usize,
    /// Byte-identical documents already present at the same identities.
    pub duplicated: usize,
    /// Documents durably accepted locally but not yet acknowledged by the remote backend.
    pub remote_pending: usize,
    /// Durable checkpoint after the commit.
    pub checkpoint: PersistenceCheckpoint,
}

/// Backend-neutral persistence failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceError {
    /// Temporary scaffold result before commit behavior is implemented.
    NotImplemented,
    /// One durable identity was already occupied by different canonical content.
    IdentityConflict(String),
    /// A backend could not durably commit the requested batch.
    Commit(String),
    /// A configured durable capacity bound cannot accept another batch.
    CapacityExhausted(String),
    /// A batch attempted to move an existing buffer cursor backward.
    CheckpointRegression {
        /// Existing durable cursor.
        current: u64,
        /// Cursor requested by the rejected batch.
        requested: u64,
    },
    /// The requested sink configuration is incomplete or inconsistent.
    InvalidConfig(String),
    /// A recognized backend has no adapter in this build or milestone.
    BackendUnavailable(String),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotImplemented => formatter.write_str("OpenOT persistence is not implemented"),
            Self::IdentityConflict(identity) => {
                write!(formatter, "OpenOT document identity conflict: {identity}")
            }
            Self::Commit(message) => {
                write!(formatter, "OpenOT persistence commit failed: {message}")
            }
            Self::CapacityExhausted(message) => {
                write!(
                    formatter,
                    "OpenOT persistence capacity exhausted: {message}"
                )
            }
            Self::CheckpointRegression { current, requested } => write!(
                formatter,
                "OpenOT checkpoint regressed from {current} to {requested}"
            ),
            Self::InvalidConfig(message) => {
                write!(
                    formatter,
                    "invalid OpenOT persistence configuration: {message}"
                )
            }
            Self::BackendUnavailable(message) => {
                write!(formatter, "backend_not_available: {message}")
            }
        }
    }
}

impl std::error::Error for PersistenceError {}

pub(super) fn ensure_private_parent(
    path: &std::path::Path,
    backend: &str,
) -> Result<(), PersistenceError> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    let existed = parent.exists();
    std::fs::create_dir_all(parent).map_err(|error| {
        PersistenceError::Commit(format!("{backend} create database directory: {error}"))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if !existed {
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).map_err(
                |error| {
                    PersistenceError::Commit(format!(
                        "{backend} secure database directory: {error}"
                    ))
                },
            )?;
        }
        let mode = std::fs::metadata(parent)
            .map_err(|error| {
                PersistenceError::Commit(format!("{backend} inspect database directory: {error}"))
            })?
            .permissions()
            .mode();
        if mode & 0o022 != 0 {
            return Err(PersistenceError::InvalidConfig(format!(
                "{backend} database directory must not be group/world writable"
            )));
        }
    }
    Ok(())
}

/// Atomic durable destination for ordered OpenOT document batches.
pub trait DocumentSink {
    /// Performs backend maintenance and returns required remote deliveries still pending.
    fn maintenance(&mut self) -> Result<usize, PersistenceError> {
        Ok(0)
    }

    /// Loads the last durable cursor for one carriage buffer.
    fn load_checkpoint(
        &mut self,
        _buffer_id: u32,
        _run_id: u64,
    ) -> Result<Option<PersistenceCheckpoint>, PersistenceError> {
        Err(PersistenceError::NotImplemented)
    }

    /// Commits all documents and the checkpoint together, or commits neither.
    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError>;
}

#[cfg(test)]
pub(crate) struct InMemoryDocumentSink {
    pub(crate) documents: Vec<Document>,
    pub(crate) checkpoint: Option<PersistenceCheckpoint>,
    fail_next_commit: bool,
    remote_pending: usize,
}

#[cfg(test)]
impl InMemoryDocumentSink {
    pub(crate) fn new() -> Self {
        Self {
            documents: Vec::new(),
            checkpoint: None,
            fail_next_commit: false,
            remote_pending: 0,
        }
    }

    pub(crate) fn fail_next_commit(&mut self) {
        self.fail_next_commit = true;
    }

    pub(crate) fn set_remote_pending(&mut self, remote_pending: usize) {
        self.remote_pending = remote_pending;
    }
}

#[cfg(test)]
impl DocumentSink for InMemoryDocumentSink {
    fn maintenance(&mut self) -> Result<usize, PersistenceError> {
        Ok(self.remote_pending)
    }

    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<PersistenceCheckpoint>, PersistenceError> {
        Ok(self
            .checkpoint
            .filter(|checkpoint| checkpoint.buffer_id == buffer_id && checkpoint.run_id == run_id))
    }

    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
        if self.fail_next_commit {
            self.fail_next_commit = false;
            return Err(PersistenceError::Commit(
                "injected in-memory transaction failure".to_string(),
            ));
        }
        if let Some(current) = self.checkpoint {
            if current.buffer_id == batch.checkpoint.buffer_id
                && current.run_id == batch.checkpoint.run_id
                && batch.checkpoint.cursor_abs < current.cursor_abs
            {
                return Err(PersistenceError::CheckpointRegression {
                    current: current.cursor_abs,
                    requested: batch.checkpoint.cursor_abs,
                });
            }
        }
        let mut staged = self.documents.clone();
        let mut inserted = 0;
        let mut duplicated = 0;
        for document in &batch.documents {
            let identity = document_identity(document);
            if let Some(existing) = staged
                .iter()
                .find(|existing| document_identity(existing) == identity)
            {
                if existing == document {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(format!("{identity:?}")));
            }
            staged.push(document.clone());
            inserted += 1;
        }
        self.documents = staged;
        self.checkpoint = Some(batch.checkpoint);
        Ok(CommitOutcome {
            inserted,
            duplicated,
            remote_pending: self.remote_pending,
            checkpoint: batch.checkpoint,
        })
    }
}
