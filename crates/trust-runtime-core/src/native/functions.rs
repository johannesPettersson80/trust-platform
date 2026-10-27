//! The kernel's functions a compiled program calls (specification 44 §8.2): every kernel reads the
//! real a text spells with the runtime's own reader, so that `STRING_TO_REAL` gives what it gives
//! in the bytecode engine, on every device.

use trust_plc_abi::kernel::{read_real, with_debugger, RealReader};
use trust_plc_abi::{FaultCode, KernelApi};

/// The runtime's reader of reals (specification 07 §2.8).
pub struct RuntimeReals;

impl RealReader for RuntimeReals {
    fn read(text: &[u8]) -> Result<f64, FaultCode> {
        core::str::from_utf8(text)
            .ok()
            .and_then(|text| crate::value::float_text::parse(text).ok())
            .ok_or(FaultCode::TypeMismatch)
    }
}

/// The functions of the kernel (specification 44 §8.2, §13): a stop goes to the debugger of the
/// call, when the program's cycle gave it one, no write is queued, a text reads as the runtime
/// reads it, and no wall clock is read; a platform with a wall clock adds its own
/// (`trust_plc_abi::kernel::with_wall_clock`).
pub const KERNEL_API: KernelApi = with_debugger(read_real::<RuntimeReals>);

/// [`KERNEL_API`] where a static is needed.
pub static KERNEL_FUNCTIONS: KernelApi = KERNEL_API;
