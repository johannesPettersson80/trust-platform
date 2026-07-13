use std::{ffi::c_void, time::Duration};

#[cfg(test)]
use std::sync::Arc;

use crate::{AdsDeviceState, AdsError, AmsAddress};

pub(crate) type AdsPortOpenEx = unsafe extern "system" fn() -> i32;
pub(crate) type AdsPortCloseEx = unsafe extern "system" fn(i32) -> i32;
pub(crate) type AdsGetLocalAddressEx = unsafe extern "system" fn(i32, *mut AmsAddress) -> i32;
pub(crate) type AdsSyncSetTimeoutEx = unsafe extern "system" fn(i32, i32) -> i32;
#[rustfmt::skip]
pub(crate) type AdsSyncReadStateReqEx = unsafe extern "system" fn(i32, *const AmsAddress, *mut u16, *mut u16) -> i32;
#[rustfmt::skip]
pub(crate) type AdsSyncReadReqEx2 = unsafe extern "system" fn(i32, *const AmsAddress, u32, u32, u32, *mut c_void, *mut u32) -> i32;
#[rustfmt::skip]
pub(crate) type AdsSyncWriteReqEx = unsafe extern "system" fn(i32, *const AmsAddress, u32, u32, u32, *const c_void) -> i32;
pub(crate) type AdsSyncReadWriteReqEx2 = unsafe extern "system" fn(
    i32,
    *const AmsAddress,
    u32,
    u32,
    u32,
    *mut c_void,
    u32,
    *const c_void,
    *mut u32,
) -> i32;

#[derive(Clone, Copy)]
pub(crate) struct FunctionTable {
    pub(crate) port_open: AdsPortOpenEx,
    pub(crate) port_close: AdsPortCloseEx,
    pub(crate) get_local_address: AdsGetLocalAddressEx,
    pub(crate) set_timeout: AdsSyncSetTimeoutEx,
    pub(crate) read_state: AdsSyncReadStateReqEx,
    pub(crate) read: AdsSyncReadReqEx2,
    pub(crate) write: AdsSyncWriteReqEx,
    pub(crate) read_write: AdsSyncReadWriteReqEx2,
}

impl FunctionTable {
    pub(crate) fn open_port(&self) -> Result<i32, AdsError> {
        // SAFETY: AdsPortOpenEx has no arguments; the function pointer was
        // resolved with the exact Beckhoff ABI and its library owner is alive.
        let handle = unsafe { (self.port_open)() };
        if handle == 0 {
            Err(AdsError::PortOpenFailed)
        } else {
            Ok(handle)
        }
    }

    pub(crate) fn close_port(&self, port: i32) -> Result<(), AdsError> {
        // SAFETY: `port` came from AdsPortOpenEx and RAII calls this exactly
        // once; the function pointer's library owner is still alive.
        let code = unsafe { (self.port_close)(port) };
        check_code("AdsPortCloseEx", code)
    }

    pub(crate) fn local_address(&self, port: i32) -> Result<AmsAddress, AdsError> {
        let mut address = AmsAddress::default();
        // SAFETY: `address` is a live, correctly aligned repr(C) AmsAddress and
        // remains exclusively borrowed for the duration of the call.
        let code = unsafe { (self.get_local_address)(port, &raw mut address) };
        check_code("AdsGetLocalAddressEx", code)?;
        Ok(address)
    }

    pub(crate) fn set_timeout(&self, port: i32, timeout: Duration) -> Result<(), AdsError> {
        let millis = timeout.as_millis();
        let millis = i32::try_from(millis).map_err(|_| AdsError::TimeoutTooLarge { millis })?;
        // SAFETY: both arguments are scalar values with the documented signed
        // Windows LONG widths and the native library remains alive.
        let code = unsafe { (self.set_timeout)(port, millis) };
        check_code("AdsSyncSetTimeoutEx", code)
    }

    pub(crate) fn read_state(
        &self,
        port: i32,
        target: AmsAddress,
    ) -> Result<AdsDeviceState, AdsError> {
        let mut ads_state = 0_u16;
        let mut device_state = 0_u16;
        // SAFETY: all pointers reference live, correctly aligned repr(C) input
        // or u16 output values for the complete synchronous call.
        let code = unsafe {
            (self.read_state)(
                port,
                &raw const target,
                &raw mut ads_state,
                &raw mut device_state,
            )
        };
        check_code("AdsSyncReadStateReqEx", code)?;
        Ok(AdsDeviceState {
            ads_state,
            device_state,
        })
    }

    pub(crate) fn read(
        &self,
        port: i32,
        target: AmsAddress,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, AdsError> {
        let length = checked_length("AdsSyncReadReqEx2", buffer.len())?;
        let mut returned = 0_u32;
        let data = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr().cast::<c_void>()
        };
        // SAFETY: `target`, `returned`, and the optional `data` range are live
        // and correctly aligned; `length` equals the Rust buffer length.
        let code = unsafe {
            (self.read)(
                port,
                &raw const target,
                index_group,
                index_offset,
                length,
                data,
                &raw mut returned,
            )
        };
        check_code("AdsSyncReadReqEx2", code)?;
        checked_returned("AdsSyncReadReqEx2", returned, buffer.len())
    }

    pub(crate) fn write(
        &self,
        port: i32,
        target: AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), AdsError> {
        let length = checked_length("AdsSyncWriteReqEx", bytes.len())?;
        let data = if bytes.is_empty() {
            std::ptr::null()
        } else {
            bytes.as_ptr().cast::<c_void>()
        };
        // SAFETY: `target` and the optional `data` range remain live for the
        // synchronous call; `length` equals the immutable Rust slice length.
        let code = unsafe {
            (self.write)(
                port,
                &raw const target,
                index_group,
                index_offset,
                length,
                data,
            )
        };
        check_code("AdsSyncWriteReqEx", code)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn read_write(
        &self,
        port: i32,
        target: AmsAddress,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, AdsError> {
        let read_length = checked_length("AdsSyncReadWriteReqEx2", read_buffer.len())?;
        let write_length = checked_length("AdsSyncReadWriteReqEx2", write_bytes.len())?;
        let read_data = if read_buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            read_buffer.as_mut_ptr().cast::<c_void>()
        };
        let write_data = if write_bytes.is_empty() {
            std::ptr::null()
        } else {
            write_bytes.as_ptr().cast::<c_void>()
        };
        let mut returned = 0_u32;
        // SAFETY: all pointers reference live repr(C) values or byte ranges for
        // the complete synchronous call, and both lengths match their slices.
        let code = unsafe {
            (self.read_write)(
                port,
                &raw const target,
                index_group,
                index_offset,
                read_length,
                read_data,
                write_length,
                write_data,
                &raw mut returned,
            )
        };
        check_code("AdsSyncReadWriteReqEx2", code)?;
        checked_returned("AdsSyncReadWriteReqEx2", returned, read_buffer.len())
    }
}

pub(crate) trait FunctionOwner: Send + Sync {
    fn functions(&self) -> &FunctionTable;
}

#[cfg(test)]
pub(crate) struct StaticFunctionOwner(pub(crate) FunctionTable);

#[cfg(test)]
impl FunctionOwner for StaticFunctionOwner {
    fn functions(&self) -> &FunctionTable {
        &self.0
    }
}

#[cfg(test)]
pub(crate) fn test_owner(functions: FunctionTable) -> Arc<dyn FunctionOwner> {
    Arc::new(StaticFunctionOwner(functions))
}

fn check_code(operation: &'static str, code: i32) -> Result<(), AdsError> {
    if code == 0 {
        Ok(())
    } else {
        Err(AdsError::native_call(operation, code))
    }
}

fn checked_length(operation: &'static str, length: usize) -> Result<u32, AdsError> {
    u32::try_from(length).map_err(|_| AdsError::BufferTooLarge { operation, length })
}

fn checked_returned(
    operation: &'static str,
    returned: u32,
    capacity: usize,
) -> Result<usize, AdsError> {
    let reported = returned;
    let returned = usize::try_from(reported).unwrap_or(usize::MAX);
    if returned <= capacity {
        Ok(returned)
    } else {
        Err(AdsError::InvalidReadLength {
            operation,
            reported,
            capacity,
        })
    }
}

#[cfg(test)]
mod length_tests {
    use super::checked_length;
    use crate::AdsError;

    #[test]
    fn rejects_lengths_above_the_native_u32_abi() {
        if usize::BITS > u32::BITS {
            let length = usize::try_from(u64::from(u32::MAX) + 1).expect("64-bit usize");
            assert_eq!(
                checked_length("read", length),
                Err(AdsError::BufferTooLarge {
                    operation: "read",
                    length,
                })
            );
        }
    }
}
