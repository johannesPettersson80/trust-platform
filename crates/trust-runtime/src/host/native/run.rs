//! A compiled program on this computer (specification 44 §10.3): its image admitted, its module's
//! code copied into executable memory, and the kernel's program over it.

use std::sync::{Arc, OnceLock};

use trust_plc_abi::kernel::{
    clock, wall_clock, with_clock, with_wall_clock, Entry, Executable, KernelClock, WallClock,
};
use trust_plc_abi::{FaultCode, KernelApi};
use trust_runtime_core::native::program::{
    admit, AdmittedImage, Device, ModuleCode, NativeProgram,
};
use trust_runtime_core::native::KERNEL_API;

use super::NativeError;
use crate::scheduler::SteppedClock;

/// The clock of a simulator for machine tests, when this process is one (specification 11 §6.8.1,
/// "Simulator for machine tests"): `CURRENT_DT` then reads its logical time from 1 January 1970
/// UTC, so that every run of a test reads the same date and time.
static SIMULATED_WALL_CLOCK: OnceLock<SteppedClock> = OnceLock::new();

/// Makes `CURRENT_DT` read the logical time of `clock` in this process, from 1 January 1970 UTC:
/// `trust-runtime simulate` calls it once, before its program starts. A later call keeps the first
/// clock.
pub fn read_wall_clock_from(clock: &SteppedClock) {
    let _ = SIMULATED_WALL_CLOCK.set(clock.clone());
}

/// The computer's wall clock, which `CURRENT_DT` reads (specification 07, "Runtime Clock Sources"):
/// `SystemTime` as UTC Unix milliseconds, a positive fraction cut off, neither the runtime's nor a
/// simulation's clock; in a simulator's process, its logical time (`read_wall_clock_from`).
struct HostClock;

impl WallClock for HostClock {
    fn unix_millis() -> Result<i64, FaultCode> {
        // A replay gives the build the trace's readings (specification 48 RU-60).
        if let Some(millis) = trust_runtime_core::trace::readings::played() {
            return Ok(millis);
        }
        let millis = if let Some(clock) = SIMULATED_WALL_CLOCK.get() {
            clock.now().as_nanos() / 1_000_000
        } else {
            let since = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| FaultCode::Overflow)?;
            i64::try_from(since.as_millis()).map_err(|_| FaultCode::Overflow)?
        };
        // A cycle trace being recorded on this thread keeps the reading (specification 48 RU-59).
        trust_runtime_core::trace::readings::record(millis);
        Ok(millis)
    }
}

/// The kernel's monotonic clock on a computer (specification 44 §8.2), which a debug image reads
/// around each call of a Rust POU (specification 48 RU-49): `Instant`, from the first reading of
/// the process, which no change of the wall clock moves.
struct HostMonotonic;

impl KernelClock for HostMonotonic {
    fn nanos() -> i64 {
        static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let origin = *ORIGIN.get_or_init(std::time::Instant::now);
        i64::try_from(origin.elapsed().as_nanos()).unwrap_or(i64::MAX)
    }
}

/// The kernel's functions on a computer: the core's, with the computer's wall clock and monotonic
/// clock.
static HOSTED_FUNCTIONS: KernelApi = with_clock(
    &with_wall_clock(&KERNEL_API, wall_clock::<HostClock>),
    clock::<HostMonotonic>,
);

/// A module's code in this process's executable memory.
pub struct HostCode(Executable);

impl ModuleCode for HostCode {
    fn entry(&self, offset: u32) -> Option<Entry> {
        self.0.entry(offset)
    }
}

/// The program of the image `bytes`, admitted on `device` and with its module mapped executable.
/// It starts with `cold_start` or `warm_start`.
///
/// # Errors
///
/// [`NativeError::Image`] with the device's refusal, or the mapping's error.
pub fn load<'i>(
    bytes: &'i [u8],
    device: &Device,
) -> Result<NativeProgram<'i, HostCode>, NativeError> {
    let image = admit(bytes, device).map_err(|refusal| NativeError::Image(refusal.to_string()))?;
    let (_, module) = image.module();
    let code = Executable::load(module)
        .map_err(|error| NativeError::Image(format!("the module cannot be mapped: {error}")))?;
    NativeProgram::new(image, HostCode(code), device, &HOSTED_FUNCTIONS)
        .map_err(|refusal| NativeError::Image(refusal.to_string()))
}

/// The program of the image `bytes`, admitted on `device`, which keeps the bytes, with its module
/// copied into executable memory: the program a computer's resource runs, for as long as it likes
/// (specification 44 §8.8). It starts with `cold_start` or `warm_start`.
///
/// # Errors
///
/// [`NativeError::Image`] with the device's refusal, or the mapping's error.
pub fn load_kept(
    bytes: Vec<u8>,
    device: &Device,
) -> Result<NativeProgram<'static, HostCode>, NativeError> {
    let image = Arc::new(
        AdmittedImage::admit(bytes.into_boxed_slice(), device)
            .map_err(|refusal| NativeError::Image(refusal.to_string()))?,
    );
    let code = Executable::load(image.image().module().1)
        .map_err(|error| NativeError::Image(format!("the module cannot be mapped: {error}")))?;
    NativeProgram::keeping(image, HostCode(code), device, &HOSTED_FUNCTIONS)
        .map_err(|refusal| NativeError::Image(refusal.to_string()))
}
