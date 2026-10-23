use core::fmt;
use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    detail: String,
    code: Option<i32>,
}

impl Error {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            code: None,
        }
    }

    fn call(operation: &'static str, code: i32) -> Self {
        Self {
            detail: format!(
                "{operation} returned ADS error {code} (0x{code:X}): {}",
                ads_error_description(code)
            ),
            code: Some(code),
        }
    }

    #[must_use]
    pub fn code(&self) -> Option<i32> {
        self.code
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for Error {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmsNetId {
    pub octets: [u8; 6],
}

impl AmsNetId {
    #[must_use]
    pub const fn new(octets: [u8; 6]) -> Self {
        Self { octets }
    }
}

impl fmt::Display for AmsNetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{}.{}.{}.{}.{}",
            self.octets[0],
            self.octets[1],
            self.octets[2],
            self.octets[3],
            self.octets[4],
            self.octets[5]
        )
    }
}

impl FromStr for AmsNetId {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut octets = [0_u8; 6];
        let mut segments = value.split('.');
        for octet in &mut octets {
            let segment = segments
                .next()
                .ok_or_else(|| Error::new("AMS Net ID must contain exactly six decimal bytes"))?;
            *octet = segment
                .parse::<u8>()
                .map_err(|_| Error::new("AMS Net ID must contain exactly six decimal bytes"))?;
        }
        if segments.next().is_some() {
            return Err(Error::new(
                "AMS Net ID must contain exactly six decimal bytes",
            ));
        }
        validate_net_id(octets).map(Self::new)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmsAddress {
    pub net_id: AmsNetId,
    pub port: u16,
}

impl AmsAddress {
    #[must_use]
    pub const fn new(net_id: AmsNetId, port: u16) -> Self {
        Self { net_id, port }
    }
}

const _: [(); 8] = [(); std::mem::size_of::<AmsAddress>()];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdsDeviceState {
    pub ads_state: u16,
    pub device_state: u16,
}

type AdsPortOpenEx = unsafe extern "system" fn() -> i32;
type AdsPortCloseEx = unsafe extern "system" fn(i32) -> i32;
type AdsGetLocalAddressEx = unsafe extern "system" fn(i32, *mut AmsAddress) -> i32;
type AdsSyncSetTimeoutEx = unsafe extern "system" fn(i32, i32) -> i32;
type AdsSyncReadStateReqEx =
    unsafe extern "system" fn(i32, *const AmsAddress, *mut u16, *mut u16) -> i32;
type AdsSyncReadReqEx2 =
    unsafe extern "system" fn(i32, *const AmsAddress, u32, u32, u32, *mut c_void, *mut u32) -> i32;
type AdsSyncWriteReqEx =
    unsafe extern "system" fn(i32, *const AmsAddress, u32, u32, u32, *const c_void) -> i32;
type AdsSyncReadWriteReqEx2 = unsafe extern "system" fn(
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
struct FunctionTable {
    open: AdsPortOpenEx,
    close: AdsPortCloseEx,
    get_local: AdsGetLocalAddressEx,
    set_timeout: AdsSyncSetTimeoutEx,
    read_state: AdsSyncReadStateReqEx,
    read: AdsSyncReadReqEx2,
    write: AdsSyncWriteReqEx,
    read_write: AdsSyncReadWriteReqEx2,
}

trait FunctionOwner: Send + Sync {
    fn functions(&self) -> &FunctionTable;
}

#[cfg(target_os = "windows")]
struct NativeOwner {
    functions: FunctionTable,
    _library: libloading::Library,
}

#[cfg(target_os = "windows")]
impl FunctionOwner for NativeOwner {
    fn functions(&self) -> &FunctionTable {
        &self.functions
    }
}

#[derive(Clone)]
pub struct TcAdsDll {
    functions: Arc<dyn FunctionOwner>,
}

impl TcAdsDll {
    pub fn load_installed() -> Result<Self, Error> {
        #[cfg(target_os = "windows")]
        {
            use libloading::{Library, Symbol};

            // SAFETY: every symbol uses the documented TcAdsApi.h ABI. NativeOwner owns the
            // library for at least as long as the copied function pointers can be called.
            unsafe {
                let library = Library::new("TcAdsDll.dll")
                    .map_err(|error| Error::new(format!("load TcAdsDll.dll: {error}")))?;
                fn load<T: Copy>(
                    library: &Library,
                    symbol: &'static [u8],
                    name: &'static str,
                ) -> Result<T, Error> {
                    // SAFETY: callers supply the exact extern-system signature for `name`.
                    unsafe {
                        let value: Symbol<'_, T> = library
                            .get(symbol)
                            .map_err(|error| Error::new(format!("load {name}: {error}")))?;
                        Ok(*value)
                    }
                }
                let functions = FunctionTable {
                    open: load(&library, b"AdsPortOpenEx\0", "AdsPortOpenEx")?,
                    close: load(&library, b"AdsPortCloseEx\0", "AdsPortCloseEx")?,
                    get_local: load(&library, b"AdsGetLocalAddressEx\0", "AdsGetLocalAddressEx")?,
                    set_timeout: load(&library, b"AdsSyncSetTimeoutEx\0", "AdsSyncSetTimeoutEx")?,
                    read_state: load(
                        &library,
                        b"AdsSyncReadStateReqEx\0",
                        "AdsSyncReadStateReqEx",
                    )?,
                    read: load(&library, b"AdsSyncReadReqEx2\0", "AdsSyncReadReqEx2")?,
                    write: load(&library, b"AdsSyncWriteReqEx\0", "AdsSyncWriteReqEx")?,
                    read_write: load(
                        &library,
                        b"AdsSyncReadWriteReqEx2\0",
                        "AdsSyncReadWriteReqEx2",
                    )?,
                };
                return Ok(Self {
                    functions: Arc::new(NativeOwner {
                        functions,
                        _library: library,
                    }),
                });
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(Error::new(
                "the TwinCAT ADS native API is only available on Windows",
            ))
        }
    }

    pub fn open_port(&self) -> Result<AdsPort, Error> {
        // SAFETY: the function pointer has the documented no-argument ABI and its owner lives in
        // the returned AdsPort.
        let handle = unsafe { (self.functions.functions().open)() };
        if handle == 0 {
            return Err(Error::new("AdsPortOpenEx returned port 0"));
        }
        Ok(AdsPort {
            functions: Arc::clone(&self.functions),
            handle,
            open: true,
            not_sync: PhantomData,
        })
    }

    #[cfg(test)]
    fn from_functions(functions: FunctionTable) -> Self {
        struct StaticOwner(FunctionTable);
        impl FunctionOwner for StaticOwner {
            fn functions(&self) -> &FunctionTable {
                &self.0
            }
        }
        Self {
            functions: Arc::new(StaticOwner(functions)),
        }
    }
}

pub struct AdsPort {
    functions: Arc<dyn FunctionOwner>,
    handle: i32,
    open: bool,
    not_sync: PhantomData<Cell<()>>,
}

impl AdsPort {
    #[must_use]
    pub fn port_number(&self) -> i32 {
        self.handle
    }

    pub fn local_address(&mut self) -> Result<AmsAddress, Error> {
        let mut address = AmsAddress::default();
        // SAFETY: address is an initialized 8-byte repr(C) output buffer.
        let code = unsafe { (self.functions.functions().get_local)(self.handle, &mut address) };
        check_code("AdsGetLocalAddressEx", code)?;
        validate_net_id(address.net_id.octets)?;
        Ok(address)
    }

    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        let millis = i32::try_from(timeout.as_millis())
            .map_err(|_| Error::new("ADS timeout exceeds the native i32 millisecond limit"))?;
        // SAFETY: both arguments use the documented scalar ABI.
        let code = unsafe { (self.functions.functions().set_timeout)(self.handle, millis) };
        check_code("AdsSyncSetTimeoutEx", code)
    }

    pub fn read_state(&mut self, target: &AmsAddress) -> Result<AdsDeviceState, Error> {
        let mut ads_state = 0_u16;
        let mut device_state = 0_u16;
        // SAFETY: target and both state outputs remain valid for the synchronous call.
        let code = unsafe {
            (self.functions.functions().read_state)(
                self.handle,
                target,
                &mut ads_state,
                &mut device_state,
            )
        };
        check_code("AdsSyncReadStateReqEx", code)?;
        Ok(AdsDeviceState {
            ads_state,
            device_state,
        })
    }

    pub fn read(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let length = checked_length(buffer.len())?;
        let mut returned = 0_u32;
        // SAFETY: buffer and returned are valid for the complete synchronous read.
        let code = unsafe {
            (self.functions.functions().read)(
                self.handle,
                target,
                index_group,
                index_offset,
                length,
                buffer.as_mut_ptr().cast(),
                &mut returned,
            )
        };
        check_code("AdsSyncReadReqEx2", code)?;
        checked_returned(returned, buffer.len())
    }

    pub fn write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let length = checked_length(bytes.len())?;
        // SAFETY: bytes remains valid for the complete synchronous write.
        let code = unsafe {
            (self.functions.functions().write)(
                self.handle,
                target,
                index_group,
                index_offset,
                length,
                bytes.as_ptr().cast(),
            )
        };
        check_code("AdsSyncWriteReqEx", code)
    }

    pub fn read_write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, Error> {
        let read_length = checked_length(read_buffer.len())?;
        let write_length = checked_length(write_bytes.len())?;
        let mut returned = 0_u32;
        // SAFETY: both byte ranges and returned remain valid for the synchronous call.
        let code = unsafe {
            (self.functions.functions().read_write)(
                self.handle,
                target,
                index_group,
                index_offset,
                read_length,
                read_buffer.as_mut_ptr().cast(),
                write_length,
                write_bytes.as_ptr().cast(),
                &mut returned,
            )
        };
        check_code("AdsSyncReadWriteReqEx2", code)?;
        checked_returned(returned, read_buffer.len())
    }

    pub fn close(mut self) -> Result<(), Error> {
        // SAFETY: this handle came from AdsPortOpenEx and is closed exactly once.
        self.open = false;
        let code = unsafe { (self.functions.functions().close)(self.handle) };
        check_code("AdsPortCloseEx", code)
    }
}

impl Drop for AdsPort {
    fn drop(&mut self) {
        if self.open {
            self.open = false;
            // SAFETY: this is the single best-effort close for the owned native port.
            let _ = unsafe { (self.functions.functions().close)(self.handle) };
        }
    }
}

pub fn local_net_id() -> Result<[u8; 6], Error> {
    let library = TcAdsDll::load_installed()?;
    let mut port = library.open_port()?;
    let address = port.local_address()?;
    port.close()?;
    Ok(address.net_id.octets)
}

fn validate_net_id(net_id: [u8; 6]) -> Result<[u8; 6], Error> {
    if net_id.iter().all(|byte| *byte == 0) {
        Err(Error::new(
            "AdsGetLocalAddressEx returned an empty AMS Net ID",
        ))
    } else {
        Ok(net_id)
    }
}

fn check_code(operation: &'static str, code: i32) -> Result<(), Error> {
    if code == 0 {
        Ok(())
    } else {
        Err(Error::call(operation, code))
    }
}

fn checked_length(length: usize) -> Result<u32, Error> {
    u32::try_from(length).map_err(|_| Error::new("ADS buffer exceeds the native u32 limit"))
}

fn checked_returned(returned: u32, capacity: usize) -> Result<usize, Error> {
    let returned = usize::try_from(returned).unwrap_or(usize::MAX);
    if returned <= capacity {
        Ok(returned)
    } else {
        Err(Error::new(format!(
            "native ADS read reported {returned} bytes for a {capacity}-byte buffer"
        )))
    }
}

const fn ads_error_description(code: i32) -> &'static str {
    match code {
        0x006 => "target ADS port was not found",
        0x007 => "target computer was not found; verify the AMS route",
        0x008 => "unknown ADS command",
        0x00B => "unknown AMS command",
        0x00D => "ADS port is not connected",
        0x012 => "ADS port is disabled",
        0x015 => "AMS synchronous request timed out",
        0x018 => "invalid AMS port",
        0x01B => "ADS target host is unreachable",
        0x505 => "ADS router is not initialized",
        0x507 => "ADS router port is not registered",
        0x509 => "ADS router port is invalid",
        0x50A => "ADS router is not active",
        0x50C => "ADS router fragment timed out",
        0x50D => "ADS router port was removed",
        0x701 => "ADS service is not supported by the target",
        0x702 => "invalid ADS index group",
        0x710 => "ADS symbol was not found",
        0x719 | 0x745 => "ADS request timed out because the target did not respond",
        _ => "unclassified native ADS failure",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    static CLOSES: AtomicUsize = AtomicUsize::new(0);
    static NATIVE_TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "system" fn open() -> i32 {
        32_981
    }

    unsafe extern "system" fn close(_port: i32) -> i32 {
        CLOSES.fetch_add(1, Ordering::SeqCst);
        0
    }

    unsafe extern "system" fn local(_port: i32, address: *mut AmsAddress) -> i32 {
        // SAFETY: the test calls through the same initialized output contract as TcAdsDll.
        unsafe {
            *address = AmsAddress::new(AmsNetId::new([100, 67, 6, 217, 1, 1]), 32_981);
        }
        0
    }

    unsafe extern "system" fn timeout(_port: i32, _millis: i32) -> i32 {
        0
    }

    unsafe extern "system" fn state(
        _port: i32,
        target: *const AmsAddress,
        ads_state: *mut u16,
        device_state: *mut u16,
    ) -> i32 {
        // SAFETY: the fake receives valid pointers from AdsPort::read_state.
        unsafe {
            assert_eq!((*target).port, 301);
            *ads_state = 5;
            *device_state = 7;
        }
        0
    }

    unsafe extern "system" fn read(
        _port: i32,
        _target: *const AmsAddress,
        _group: u32,
        _offset: u32,
        length: u32,
        data: *mut c_void,
        returned: *mut u32,
    ) -> i32 {
        // SAFETY: the fake receives a buffer of `length` bytes and one u32 output.
        unsafe {
            std::ptr::write_bytes(data, 0xA5, length as usize);
            *returned = length;
        }
        0
    }

    unsafe extern "system" fn write(
        _port: i32,
        _target: *const AmsAddress,
        _group: u32,
        _offset: u32,
        _length: u32,
        _data: *const c_void,
    ) -> i32 {
        0
    }

    unsafe extern "system" fn read_write(
        _port: i32,
        _target: *const AmsAddress,
        _group: u32,
        _offset: u32,
        read_length: u32,
        read_data: *mut c_void,
        _write_length: u32,
        _write_data: *const c_void,
        returned: *mut u32,
    ) -> i32 {
        // SAFETY: the fake receives a read buffer and returned output from AdsPort::read_write.
        unsafe {
            std::ptr::write_bytes(read_data, 0x5A, read_length as usize);
            *returned = read_length;
        }
        0
    }

    fn library() -> TcAdsDll {
        TcAdsDll::from_functions(FunctionTable {
            open,
            close,
            get_local: local,
            set_timeout: timeout,
            read_state: state,
            read,
            write,
            read_write,
        })
    }

    #[test]
    fn native_port_preserves_target_port_and_closes_once() {
        let _guard = NATIVE_TEST_LOCK.lock().expect("native test lock");
        CLOSES.store(0, Ordering::SeqCst);
        let mut port = library().open_port().expect("open fake native port");
        assert_eq!(
            port.local_address()
                .expect("local address")
                .net_id
                .to_string(),
            "100.67.6.217.1.1"
        );
        let target = AmsAddress::new(AmsNetId::new([100, 67, 6, 217, 1, 1]), 301);
        assert_eq!(port.read_state(&target).expect("read state").ads_state, 5);
        port.close().expect("close fake native port");
        assert_eq!(CLOSES.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn native_read_and_read_write_validate_returned_lengths() {
        let _guard = NATIVE_TEST_LOCK.lock().expect("native test lock");
        let target = AmsAddress::new(AmsNetId::new([100, 67, 6, 217, 1, 1]), 851);
        let mut port = library().open_port().expect("open fake native port");
        let mut read_buffer = [0_u8; 4];
        assert_eq!(port.read(&target, 1, 2, &mut read_buffer), Ok(4));
        assert_eq!(read_buffer, [0xA5; 4]);
        assert_eq!(
            port.read_write(&target, 3, 4, &mut read_buffer, b"symbol\0"),
            Ok(4)
        );
        assert_eq!(read_buffer, [0x5A; 4]);
    }

    #[test]
    fn native_net_id_parser_requires_six_nonzero_octets() {
        assert_eq!(
            "100.67.6.217.1.1"
                .parse::<AmsNetId>()
                .expect("valid net id")
                .octets,
            [100, 67, 6, 217, 1, 1]
        );
        assert!("100.67.6.217.1".parse::<AmsNetId>().is_err());
        assert!("0.0.0.0.0.0".parse::<AmsNetId>().is_err());
    }
}
