//! A compiled program on this computer (specification 44 §10.3): its image admitted, its module's
//! code copied into executable memory, and the kernel's program over it.

use trust_plc_abi::kernel::{Entry, Executable};
use trust_runtime_core::native::program::{admit, Device, ModuleCode, NativeProgram};
use trust_runtime_core::native::KERNEL_FUNCTIONS;

use super::NativeError;

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
    NativeProgram::new(image, HostCode(code), device, &KERNEL_FUNCTIONS)
        .map_err(|refusal| NativeError::Image(refusal.to_string()))
}
