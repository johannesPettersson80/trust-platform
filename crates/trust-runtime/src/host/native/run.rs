//! A compiled program on this computer (specification 44 §10.3): its image admitted, its module's
//! code copied into executable memory, and the kernel's program over it.

use std::sync::Arc;

use trust_plc_abi::kernel::{wall_clock, with_wall_clock, Entry, Executable, WallClock};
use trust_plc_abi::{FaultCode, KernelApi};
use trust_runtime_core::native::program::{
    admit, AdmittedImage, Device, ModuleCode, NativeProgram,
};
use trust_runtime_core::native::KERNEL_API;

use super::NativeError;

/// The computer's wall clock, which `CURRENT_DT` reads (specification 07, "Runtime Clock Sources"):
/// `SystemTime` as UTC Unix milliseconds, a positive fraction cut off, neither the runtime's nor a
/// simulation's clock.
struct HostClock;

impl WallClock for HostClock {
    fn unix_millis() -> Result<i64, FaultCode> {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| FaultCode::Overflow)?;
        i64::try_from(since.as_millis()).map_err(|_| FaultCode::Overflow)
    }
}

/// The kernel's functions on a computer: the core's, with the computer's wall clock.
static HOSTED_FUNCTIONS: KernelApi = with_wall_clock(&KERNEL_API, wall_clock::<HostClock>);

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
