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
