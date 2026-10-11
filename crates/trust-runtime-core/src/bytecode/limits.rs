//! Fixed bytecode resource limits for STBC version 1.x.

/// Maximum encoded STBC container size in bytes (64 MiB).
pub const BYTECODE_MAX_CONTAINER_BYTES: usize = 64 * 1024 * 1024;
/// Maximum decoded instruction count in one module.
pub const BYTECODE_MAX_INSTRUCTIONS: usize = 1_000_000;
/// Maximum reference-table entries in one module.
pub const BYTECODE_MAX_REFERENCES: usize = 65_536;
/// Maximum local references declared by one POU.
pub const BYTECODE_MAX_LOCALS_PER_POU: usize = 65_536;
/// Maximum parameters declared by one POU.
pub const BYTECODE_MAX_PARAMETERS_PER_POU: usize = 1_024;
/// Maximum arguments carried by one native-call instruction.
pub const BYTECODE_MAX_NATIVE_ARGUMENTS: usize = 1_024;
/// Maximum nested type references while validating or materializing one constant payload.
pub const BYTECODE_MAX_CONST_NESTING: u8 = 64;

/// Limits for temporary semantic analysis, excluding the caller-owned decoded artifact.
/// Accounted bytes cover requested analysis capacities and retained qualified-name scratch.
/// Error diagnostics, allocator metadata/rounding, transient reallocation copies and native stack
/// are excluded and need profile headroom. Diagnostics preserve the original rejection reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationLimits {
    /// Maximum simultaneously accounted analysis storage in bytes.
    pub max_scratch_bytes: usize,
    /// Maximum logical work units across all POUs (visits and examined/copied slots/bytes).
    pub max_work: usize,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_scratch_bytes: 64 * 1024 * 1024,
            max_work: 64 * 1024 * 1024,
        }
    }
}

/// Deterministic analysis accounting, not a measurement of allocator overhead or WCET.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ValidationStats {
    /// Maximum simultaneously accounted analysis storage.
    pub peak_scratch_bytes: usize,
    /// Total charged analysis work units.
    pub work: usize,
}

/// Maximum records in each STBC 2.0 construction/initializer table.
pub const BYTECODE_MAX_CONSTRUCTION_RECORDS: usize = 65_536;
/// Maximum logical nodes materialized by one declaration's construction recipe.
pub const BYTECODE_MAX_CONSTRUCTION_NODES: u32 = 1_000_000;
