//! Shared wire opcodes for construction and initialization.

/// STBC initialization instruction `DEFAULT_VALUE`.
pub const DEFAULT_VALUE: u8 = 0x65;

/// STBC initialization instruction `DEFAULT_TYPED`.
pub const DEFAULT_TYPED: u8 = 0x66;

/// STBC initialization instruction `COERCE_INIT_VALUE`.
pub const COERCE_INIT_VALUE: u8 = 0x67;

/// STBC initialization instruction `APPLY_INIT_VALUE`.
pub const APPLY_INIT_VALUE: u8 = 0x68;

/// STBC initialization instruction `ARRAY_NEW`.
pub const ARRAY_NEW: u8 = 0x69;

/// STBC initialization instruction `ARRAY_SET`.
pub const ARRAY_SET: u8 = 0x6A;

/// STBC initialization instruction `STRUCT_NEW`.
pub const STRUCT_NEW: u8 = 0x6B;

/// STBC initialization instruction `STRUCT_SET`.
pub const STRUCT_SET: u8 = 0x6C;
