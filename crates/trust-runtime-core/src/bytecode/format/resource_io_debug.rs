use super::*;

/// Configuration resources encoded in the artifact.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceMeta {
    /// Resources in configuration order.
    pub resources: Vec<ResourceEntry>,
}

/// Process-image sizes and task configuration for one resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEntry {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Input process-image size in bytes.
    pub inputs_size: u32,
    /// Output process-image size in bytes.
    pub outputs_size: u32,
    /// Marker-memory process-image size in bytes.
    pub memory_size: u32,
    /// Configured tasks in declaration order.
    pub tasks: Vec<TaskEntry>,
}

/// Configured task timing, priority and bound programs or FB references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEntry {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Task priority; smaller numeric values execute first.
    pub priority: u32,
    /// Periodic task interval in nanoseconds.
    pub interval_nanos: i64,
    /// Optional single-event variable name index.
    pub single_name_idx: Option<u32>,
    /// Program name indices in execution order.
    pub program_name_idx: Vec<u32>,
    /// Function-block instance reference indices.
    pub fb_ref_idx: Vec<u32>,
}

/// Process-image address bindings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IoMap {
    /// Address bindings in wire order.
    pub bindings: Vec<IoBinding>,
}

/// A textual I/O address bound to a typed storage reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoBinding {
    /// Textual direct-address index in STRING_TABLE.
    pub address_str_idx: u32,
    /// Reference index in REF_TABLE.
    pub ref_idx: u32,
    /// Type index in TYPE_TABLE.
    pub type_id: Option<u32>,
}

/// Source locations indexed by POU and bytecode position.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugMap {
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<DebugEntry>,
}

/// One source mapping; offsets are absolute POU_BODIES byte positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugEntry {
    /// Referenced POU identifier.
    pub pou_id: u32,
    /// Absolute byte offset within POU_BODIES.
    pub code_offset: u32,
    /// Source filename index in the applicable debug string table.
    pub file_idx: u32,
    /// Source line number.
    pub line: u32,
    /// Source column number.
    pub column: u32,
    /// Wire record kind.
    pub kind: u8,
}
