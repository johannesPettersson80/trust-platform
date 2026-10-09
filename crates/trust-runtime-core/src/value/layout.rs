//! Value slot accounting for preparation and target build evidence.

use super::Value;

/// Bytes occupied by a value slot on the compiled target, excluding owned data.
pub const VALUE_SIZE_BYTES: usize = core::mem::size_of::<Value>();
/// Alignment of a value slot on the compiled target.
pub const VALUE_ALIGN_BYTES: usize = core::mem::align_of::<Value>();

// A1's slot budgets are not limits on application storage or heap allocations.
// F401/C6 builds must stop rather than silently growing the planned slot cost.
#[cfg(target_pointer_width = "32")]
const _: () = assert!(VALUE_SIZE_BYTES <= 32 && VALUE_ALIGN_BYTES <= 8);

#[cfg(target_pointer_width = "64")]
const _: () = assert!(VALUE_SIZE_BYTES <= 48 && VALUE_ALIGN_BYTES <= 8);
