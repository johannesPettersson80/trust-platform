use std::{
    cell::Cell, fmt, marker::PhantomData, path::PathBuf, str::FromStr, sync::Arc, time::Duration,
};

use crate::{error::AdsError, ffi::FunctionOwner};

/// Resolves the machine-wide Windows `ProgramData` directory using the trusted
/// operating-system known-folder API.
///
/// This function never reads process environment variables.
///
/// # Errors
///
/// Returns [`AdsError::UnsupportedPlatform`] outside Windows, or
/// [`AdsError::WindowsDirectory`] when Windows cannot provide an absolute
/// machine-wide path.
pub fn trusted_program_data_root() -> Result<PathBuf, AdsError> {
    #[cfg(windows)]
    {
        crate::windows_paths::trusted_program_data_root()
    }
    #[cfg(not(windows))]
    {
        Err(AdsError::UnsupportedPlatform)
    }
}

/// Six-byte AMS network identifier used by the `TwinCAT` router.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmsNetId {
    /// Net ID octets in their wire/ABI order.
    pub octets: [u8; 6],
}

impl AmsNetId {
    /// Creates an AMS network identifier from its six octets.
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
    type Err = ParseAmsNetIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut octets = [0_u8; 6];
        let mut segments = value.split('.');
        for (index, octet) in octets.iter_mut().enumerate() {
            let Some(segment) = segments.next() else {
                return Err(ParseAmsNetIdError);
            };
            *octet = segment.parse::<u8>().map_err(|_| ParseAmsNetIdError)?;
            if index == 5 && segments.next().is_some() {
                return Err(ParseAmsNetIdError);
            }
        }
        Ok(Self::new(octets))
    }
}

/// Error returned when a textual AMS Net ID is not exactly six decimal bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseAmsNetIdError;

impl fmt::Display for ParseAmsNetIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AMS Net ID must contain exactly six decimal bytes")
    }
}

impl std::error::Error for ParseAmsNetIdError {}

/// Native AMS address: a six-byte network ID plus a 16-bit logical ADS port.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmsAddress {
    /// Target or source AMS network identifier.
    pub net_id: AmsNetId,
    /// Logical ADS service port, for example PLC runtime port 851.
    pub port: u16,
}

impl AmsAddress {
    /// Creates an AMS address.
    #[must_use]
    pub const fn new(net_id: AmsNetId, port: u16) -> Self {
        Self { net_id, port }
    }
}

/// Raw ADS and device states returned by an ADS server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdsDeviceState {
    /// ADS state from the Beckhoff `ADSSTATE` value space.
    pub ads_state: u16,
    /// Device-specific state.
    pub device_state: u16,
}

/// Loaded native `TwinCAT` ADS client library.
#[derive(Clone)]
pub struct TcAdsDll {
    functions: Arc<dyn FunctionOwner>,
}

impl TcAdsDll {
    /// Loads the installed `TcAdsDll.dll` using secure Windows loader search
    /// flags and trusted `TwinCAT` installation paths.
    ///
    /// # Errors
    ///
    /// Returns [`AdsError::UnsupportedPlatform`] outside Windows, or a precise
    /// load/export error when `TwinCAT`'s native client is unavailable.
    pub fn load_installed() -> Result<Self, AdsError> {
        #[cfg(windows)]
        {
            crate::native::load_installed().map(Self::from_owner)
        }
        #[cfg(not(windows))]
        {
            Err(AdsError::UnsupportedPlatform)
        }
    }

    #[cfg(any(windows, test))]
    pub(crate) fn from_owner(functions: Arc<dyn FunctionOwner>) -> Self {
        Self { functions }
    }

    /// Opens an independent port to the local `TwinCAT` message router.
    ///
    /// # Errors
    ///
    /// Returns [`AdsError::PortOpenFailed`] when no local router accepts the
    /// port.
    pub fn open_port(&self) -> Result<AdsPort, AdsError> {
        let handle = self.functions.functions().open_port()?;
        Ok(AdsPort {
            functions: Arc::clone(&self.functions),
            handle,
            open: true,
            not_sync: PhantomData,
        })
    }
}

/// Owned local `TwinCAT` router port.
///
/// Dropping this value closes the native port exactly once. Operations require
/// mutable access so one port cannot issue overlapping synchronous calls.
pub struct AdsPort {
    functions: Arc<dyn FunctionOwner>,
    handle: i32,
    open: bool,
    not_sync: PhantomData<Cell<()>>,
}

impl AdsPort {
    /// Returns the native local ADS port handle.
    #[must_use]
    pub fn port_number(&self) -> i32 {
        self.handle
    }

    /// Returns the AMS identity assigned by the local `TwinCAT` router.
    ///
    /// # Errors
    ///
    /// Returns the native ADS error from `AdsGetLocalAddressEx`.
    pub fn local_address(&mut self) -> Result<AmsAddress, AdsError> {
        self.functions.functions().local_address(self.port_number())
    }

    /// Sets the timeout for subsequent synchronous requests on this port.
    ///
    /// # Errors
    ///
    /// Returns a size error for durations beyond `i32::MAX` milliseconds, or
    /// the native ADS error from `AdsSyncSetTimeoutEx`.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), AdsError> {
        self.functions
            .functions()
            .set_timeout(self.port_number(), timeout)
    }

    /// Reads ADS and device state from a target logical port.
    ///
    /// # Errors
    ///
    /// Returns the native ADS error from `AdsSyncReadStateReqEx`.
    pub fn read_state(&mut self, target: &AmsAddress) -> Result<AdsDeviceState, AdsError> {
        self.functions
            .functions()
            .read_state(self.port_number(), *target)
    }

    /// Reads bytes from an ADS index group and offset into `buffer`.
    ///
    /// The returned length is the number of bytes reported by `TwinCAT`.
    ///
    /// # Errors
    ///
    /// Returns a buffer-size error, native ADS error, or invalid native return
    /// length.
    pub fn read(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, AdsError> {
        self.functions.functions().read(
            self.port_number(),
            *target,
            index_group,
            index_offset,
            buffer,
        )
    }

    /// Writes raw bytes to an ADS index group and offset.
    ///
    /// # Errors
    ///
    /// Returns a buffer-size error or the native ADS error from
    /// `AdsSyncWriteReqEx`.
    pub fn write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), AdsError> {
        self.functions.functions().write(
            self.port_number(),
            *target,
            index_group,
            index_offset,
            bytes,
        )
    }

    /// Atomically writes request bytes and reads response bytes in one ADS
    /// request.
    ///
    /// # Errors
    ///
    /// Returns a buffer-size error, native ADS error, or invalid native return
    /// length.
    pub fn read_write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, AdsError> {
        self.functions.functions().read_write(
            self.port_number(),
            *target,
            index_group,
            index_offset,
            read_buffer,
            write_bytes,
        )
    }

    /// Closes this port and reports a native close error.
    ///
    /// Drop is still a safe fallback, but cannot report close failures.
    ///
    /// # Errors
    ///
    /// Returns the native ADS error from `AdsPortCloseEx`.
    pub fn close(mut self) -> Result<(), AdsError> {
        self.functions.functions().close_port(self.handle)?;
        self.open = false;
        Ok(())
    }
}

impl Drop for AdsPort {
    fn drop(&mut self) {
        if self.open {
            self.open = false;
            let _ = self.functions.functions().close_port(self.handle);
        }
    }
}
