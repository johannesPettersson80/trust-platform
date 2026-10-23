//! Bounded initializer-branch counts for post-run diagnosis.

use crate::bytecode::{InitializationStage, InitializerBodyKind, StorageRole};
use core::cell::{Cell, RefCell};

/// Maximum number of distinct initializer metadata/branch rows retained.
pub const INITIALIZER_BRANCH_RECORD_CAPACITY: usize = 64;

/// Exact control-flow branch selected by one initializer evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InitializerBranch {
    /// Default constant copied without allocating an initializer frame.
    LiteralDefaultFastPath = 1,
    /// Scalar intrinsic default constructed without an initializer frame.
    DirectScalarIntrinsicFastPath = 2,
    /// Constant default copied into a staged initializer result.
    StagedConstant = 3,
    /// Intrinsic default constructed into a staged initializer result.
    StagedIntrinsic = 4,
    /// Nonempty initializer body executed through the bytecode dispatcher.
    InitializerBody = 5,
}

impl InitializerBranch {
    /// Stable label for post-run reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LiteralDefaultFastPath => "literal_default_fast_path",
            Self::DirectScalarIntrinsicFastPath => "direct_scalar_intrinsic_fast_path",
            Self::StagedConstant => "staged_constant",
            Self::StagedIntrinsic => "staged_intrinsic",
            Self::InitializerBody => "initializer_body",
        }
    }
}

/// Metadata already available at the initializer decision point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializerBranchMetadata {
    /// Initializer table index.
    pub initializer_id: u32,
    /// Declaration table index, when this action targets a declaration.
    pub declaration_idx: Option<u32>,
    /// Declaration role, when the declaration metadata is available.
    pub declaration_role: Option<StorageRole>,
    /// Result type table index.
    pub result_type_id: u32,
    /// Primitive type identifier for primitive result types.
    pub primitive_id: Option<u16>,
    /// Initialization stage from the prepared entry.
    pub stage: InitializationStage,
    /// Initializer body kind from the prepared entry.
    pub body_kind: InitializerBodyKind,
    /// Encoded bytecode body length.
    pub code_length: u32,
    /// Declaration constant selected by the prepared layout, if present.
    pub default_const_idx: Option<u32>,
}

/// Count for one unique initializer metadata/branch combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializerBranchRecord {
    /// Initializer table index.
    pub initializer_id: u32,
    /// Declaration table index, when present.
    pub declaration_idx: Option<u32>,
    /// Declaration role, when available.
    pub declaration_role: Option<StorageRole>,
    /// Result type table index.
    pub result_type_id: u32,
    /// Primitive type identifier for primitive result types.
    pub primitive_id: Option<u16>,
    /// Initialization stage.
    pub stage: InitializationStage,
    /// Initializer body kind.
    pub body_kind: InitializerBodyKind,
    /// Encoded bytecode body length.
    pub code_length: u32,
    /// Declaration constant selected by the prepared layout, if present.
    pub default_const_idx: Option<u32>,
    /// Branch selected by `evaluate_initializer`.
    pub branch: InitializerBranch,
    /// Number of observed evaluations with this exact key.
    pub calls: u64,
}

impl InitializerBranchRecord {
    const EMPTY: Self = Self {
        initializer_id: 0,
        declaration_idx: None,
        declaration_role: None,
        result_type_id: 0,
        primitive_id: None,
        stage: InitializationStage::Default,
        body_kind: InitializerBodyKind::Action,
        code_length: 0,
        default_const_idx: None,
        branch: InitializerBranch::LiteralDefaultFastPath,
        calls: 0,
    };

    fn matches(self, metadata: InitializerBranchMetadata, branch: InitializerBranch) -> bool {
        self.calls != 0
            && self.initializer_id == metadata.initializer_id
            && self.declaration_idx == metadata.declaration_idx
            && self.declaration_role == metadata.declaration_role
            && self.result_type_id == metadata.result_type_id
            && self.primitive_id == metadata.primitive_id
            && self.stage == metadata.stage
            && self.body_kind == metadata.body_kind
            && self.code_length == metadata.code_length
            && self.default_const_idx == metadata.default_const_idx
            && self.branch == branch
    }

    fn from_metadata(metadata: InitializerBranchMetadata, branch: InitializerBranch) -> Self {
        Self {
            initializer_id: metadata.initializer_id,
            declaration_idx: metadata.declaration_idx,
            declaration_role: metadata.declaration_role,
            result_type_id: metadata.result_type_id,
            primitive_id: metadata.primitive_id,
            stage: metadata.stage,
            body_kind: metadata.body_kind,
            code_length: metadata.code_length,
            default_const_idx: metadata.default_const_idx,
            branch,
            calls: 1,
        }
    }
}

/// Fixed-size branch totals for one sampled run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializerBranchSnapshot {
    /// Populated records occupy `records[..len]`.
    pub records: [InitializerBranchRecord; INITIALIZER_BRANCH_RECORD_CAPACITY],
    /// Number of populated records.
    pub len: usize,
    /// Events lost because the fixed record table filled or a count saturated.
    pub overflow_count: u64,
    /// Events lost because the thread-local recorder was busy or unavailable.
    pub recorder_busy: u64,
}

impl InitializerBranchSnapshot {
    const fn empty() -> Self {
        Self {
            records: [InitializerBranchRecord::EMPTY; INITIALIZER_BRANCH_RECORD_CAPACITY],
            len: 0,
            overflow_count: 0,
            recorder_busy: 0,
        }
    }

    /// Whether every observed branch fit in the recorder.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.overflow_count == 0 && self.recorder_busy == 0
    }
}

/// Whether initializer branch recording is active on this thread.
#[must_use]
pub fn initializer_branch_log_enabled() -> bool {
    match LOG_ENABLED.try_with(Cell::get) {
        Ok(enabled) => enabled,
        Err(_) => {
            note_busy();
            false
        }
    }
}

/// Clear prior branch records and enable recording on this thread.
pub fn start_initializer_branch_log() {
    let mut reset = false;
    let result = LOG.try_with(|log| {
        let Ok(mut log) = log.try_borrow_mut() else {
            return;
        };
        log.reset();
        reset = true;
    });
    if result.is_err() || !reset {
        note_busy();
        let _ = LOG_ENABLED.try_with(|enabled| enabled.set(false));
        return;
    }
    let _ = LOG_FAILURES.try_with(|failures| failures.set(0));
    let _ = LOG_ENABLED.try_with(|enabled| enabled.set(true));
}

/// Stop recording and return the bounded aggregate for post-run export.
#[must_use]
pub fn finish_initializer_branch_log() -> InitializerBranchSnapshot {
    let _ = LOG_ENABLED.try_with(|enabled| enabled.set(false));
    let mut snapshot = InitializerBranchSnapshot::empty();
    let result = LOG.try_with(|log| {
        let Ok(log) = log.try_borrow() else {
            snapshot.recorder_busy = 1;
            return;
        };
        snapshot = log.snapshot();
    });
    if result.is_err() {
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(1);
    }
    if let Ok(failures) = LOG_FAILURES.try_with(Cell::get) {
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(failures);
    }
    snapshot
}

/// Add one selected branch without allocating or formatting.
pub fn record_initializer_branch(metadata: InitializerBranchMetadata, branch: InitializerBranch) {
    if !initializer_branch_log_enabled() {
        return;
    }
    let result = LOG.try_with(|log| {
        let Ok(mut log) = log.try_borrow_mut() else {
            note_busy();
            return;
        };
        log.record(metadata, branch);
    });
    if result.is_err() {
        note_busy();
    }
}

struct Recorder {
    records: [InitializerBranchRecord; INITIALIZER_BRANCH_RECORD_CAPACITY],
    len: usize,
    overflow_count: u64,
}

impl Recorder {
    const fn new() -> Self {
        Self {
            records: [InitializerBranchRecord::EMPTY; INITIALIZER_BRANCH_RECORD_CAPACITY],
            len: 0,
            overflow_count: 0,
        }
    }

    fn reset(&mut self) {
        self.records.fill(InitializerBranchRecord::EMPTY);
        self.len = 0;
        self.overflow_count = 0;
    }

    fn record(&mut self, metadata: InitializerBranchMetadata, branch: InitializerBranch) {
        if let Some(index) = self.records[..self.len]
            .iter_mut()
            .position(|record| record.matches(metadata, branch))
        {
            if let Some(calls) = self.records[index].calls.checked_add(1) {
                self.records[index].calls = calls;
            } else {
                self.overflow_count = self.overflow_count.saturating_add(1);
            }
            return;
        }
        if self.len == INITIALIZER_BRANCH_RECORD_CAPACITY {
            self.overflow_count = self.overflow_count.saturating_add(1);
            return;
        }
        self.records[self.len] = InitializerBranchRecord::from_metadata(metadata, branch);
        self.len += 1;
    }

    fn snapshot(&self) -> InitializerBranchSnapshot {
        InitializerBranchSnapshot {
            records: self.records,
            len: self.len,
            overflow_count: self.overflow_count,
            recorder_busy: 0,
        }
    }
}

fn note_busy() {
    let _ = LOG_FAILURES.try_with(|failures| {
        failures.set(failures.get().saturating_add(1));
    });
}

thread_local! {
    static LOG: RefCell<Recorder> = const { RefCell::new(Recorder::new()) };
    static LOG_ENABLED: Cell<bool> = const { Cell::new(false) };
    static LOG_FAILURES: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(initializer_id: u32) -> InitializerBranchMetadata {
        InitializerBranchMetadata {
            initializer_id,
            declaration_idx: Some(initializer_id + 10),
            declaration_role: Some(StorageRole::Return),
            result_type_id: 7,
            primitive_id: Some(7),
            stage: InitializationStage::Default,
            body_kind: InitializerBodyKind::Action,
            code_length: 0,
            default_const_idx: None,
        }
    }

    #[test]
    fn initializer_branch_log_aggregates_equal_metadata_without_timing() {
        start_initializer_branch_log();
        let metadata = metadata(3);
        record_initializer_branch(metadata, InitializerBranch::DirectScalarIntrinsicFastPath);
        record_initializer_branch(metadata, InitializerBranch::DirectScalarIntrinsicFastPath);
        let snapshot = finish_initializer_branch_log();

        assert!(snapshot.is_valid());
        assert_eq!(snapshot.len, 1);
        let record = snapshot.records[0];
        assert_eq!(record.initializer_id, 3);
        assert_eq!(record.declaration_idx, Some(13));
        assert_eq!(record.declaration_role, Some(StorageRole::Return));
        assert_eq!(record.result_type_id, 7);
        assert_eq!(record.primitive_id, Some(7));
        assert_eq!(record.stage, InitializationStage::Default);
        assert_eq!(record.body_kind, InitializerBodyKind::Action);
        assert_eq!(record.code_length, 0);
        assert_eq!(record.default_const_idx, None);
        assert_eq!(
            record.branch,
            InitializerBranch::DirectScalarIntrinsicFastPath
        );
        assert_eq!(record.calls, 2);
    }

    #[test]
    fn initializer_branch_log_overflow_invalidates_snapshot_without_panicking() {
        start_initializer_branch_log();
        for initializer_id in 0..65 {
            record_initializer_branch(metadata(initializer_id), InitializerBranch::InitializerBody);
        }
        let snapshot = finish_initializer_branch_log();

        assert_eq!(snapshot.len, 64);
        assert_eq!(snapshot.overflow_count, 1);
        assert!(!snapshot.is_valid());
    }

    #[test]
    fn initializer_branch_log_is_disabled_after_finish() {
        start_initializer_branch_log();
        let first = finish_initializer_branch_log();
        assert!(first.is_valid());
        record_initializer_branch(metadata(1), InitializerBranch::LiteralDefaultFastPath);
        let second = finish_initializer_branch_log();
        assert!(second.is_valid());
        assert_eq!(second.len, 0);
    }
}
