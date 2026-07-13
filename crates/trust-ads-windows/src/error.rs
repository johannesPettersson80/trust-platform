use std::path::PathBuf;

use thiserror::Error;

/// Failure returned by the native Windows ADS boundary.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AdsError {
    /// The native Windows library cannot be loaded on this host platform.
    #[error("TcAdsDll is available only on Windows")]
    UnsupportedPlatform,

    /// No DLL existed in a trusted Windows system or `TwinCAT` installation path.
    #[error(
        "TcAdsDll.dll was not found in the Windows system directory or trusted TwinCAT installation paths: {searched:?}"
    )]
    LibraryNotFound {
        /// Absolute installation paths that were inspected after the secure
        /// system-directory lookup failed.
        searched: Vec<PathBuf>,
    },

    /// Windows found a DLL in a trusted location but could not load it.
    #[error("failed to load TcAdsDll.dll from {path}: {reason}")]
    LibraryLoad {
        /// Trusted path used for the load attempt.
        path: PathBuf,
        /// Loader error without any secret material.
        reason: String,
    },

    /// The installed DLL does not export the required Beckhoff function.
    #[error("TcAdsDll.dll is missing required export {symbol}: {reason}")]
    MissingSymbol {
        /// Required exported function name.
        symbol: &'static str,
        /// Loader error.
        reason: String,
    },

    /// Windows could not resolve a trusted system-wide directory.
    #[error("failed to resolve trusted Windows {directory} directory: {reason}")]
    WindowsDirectory {
        /// Stable Windows directory name.
        directory: &'static str,
        /// Operating-system error without secret material.
        reason: String,
    },

    /// `AdsPortOpenEx` returned the documented failure sentinel.
    #[error("AdsPortOpenEx could not open a port to the local TwinCAT router")]
    PortOpenFailed,

    /// A native ADS call returned a non-zero ADS error code.
    #[error("{operation} failed with ADS error {code} (0x{code:08X}): {description}")]
    Call {
        /// Stable operation name.
        operation: &'static str,
        /// Raw Beckhoff ADS error code.
        code: i32,
        /// Stable human-readable semantics used by callers for diagnostics.
        description: &'static str,
    },

    /// A Rust byte slice cannot be represented by the 32-bit `TcAdsDll` ABI.
    #[error("{operation} buffer length {length} exceeds the TcAdsDll 32-bit length limit")]
    BufferTooLarge {
        /// Stable operation name.
        operation: &'static str,
        /// Requested byte length.
        length: usize,
    },

    /// A duration cannot be represented by the signed Windows `LONG` timeout ABI.
    #[error("timeout {millis} ms exceeds the TcAdsDll signed 32-bit timeout limit")]
    TimeoutTooLarge {
        /// Requested timeout in milliseconds.
        millis: u128,
    },

    /// The native DLL reported that it wrote beyond the supplied read buffer.
    #[error("{operation} reported {reported} bytes for a {capacity}-byte buffer")]
    InvalidReadLength {
        /// Stable operation name.
        operation: &'static str,
        /// Bytes reported by `TcAdsDll`.
        reported: u32,
        /// Supplied Rust buffer capacity.
        capacity: usize,
    },
}

impl AdsError {
    pub(crate) fn native_call(operation: &'static str, code: i32) -> Self {
        Self::Call {
            operation,
            code,
            description: ads_error_description(code),
        }
    }
}

// These messages are deliberately hard-coded instead of delegated to an OS or
// installed TwinCAT message table. Callers therefore get stable, actionable
// semantics while AdsError::Call retains the exact numeric code returned by
// TcAdsDll. The values follow Beckhoff's documented ADS Return Codes table.
#[allow(clippy::too_many_lines)]
const fn ads_error_description(code: i32) -> &'static str {
    match code {
        0x001 => "internal ADS error",
        0x002 => "real-time system is unavailable",
        0x003 => "locked-memory allocation failed",
        0x004 => "ADS mailbox is full",
        0x005 => "wrong ADS receive message",
        0x006 => "target ADS port was not found; the ADS server may not be running",
        0x007 => "target computer was not found; verify the AMS route",
        0x008 => "unknown ADS command",
        0x009 => "invalid ADS task ID",
        0x00A => "ADS I/O is unavailable",
        0x00B => "unknown AMS command",
        0x00C => "Windows reported an ADS transport error",
        0x00D => "ADS port is not connected",
        0x00E => "invalid AMS message length",
        0x00F => "invalid AMS Net ID",
        0x010 => "TwinCAT installation level is too low",
        0x011 => "ADS debugging is unavailable",
        0x012 => "ADS port is disabled; the TwinCAT system service may not be running",
        0x013 => "ADS port is already connected",
        0x014 => "AMS synchronous Windows error",
        0x015 => "AMS synchronous request timed out",
        0x016 => "AMS synchronous request failed",
        0x017 => "AMS synchronous index map is unavailable",
        0x018 => "invalid AMS port",
        0x019 => "ADS transport is out of memory",
        0x01A => "ADS TCP send failed",
        0x01B => "ADS target host is unreachable",
        0x01C => "invalid AMS fragment",
        0x01D => "secure ADS TLS send failed",
        0x01E => "secure ADS access was denied",

        0x500 => "ADS router has no locked memory",
        0x501 => "ADS router could not resize memory",
        0x502 => "ADS router mailbox is full",
        0x503 => "ADS router debug mailbox is full",
        0x504 => "ADS router port type is unknown",
        0x505 => "ADS router is not initialized",
        0x506 => "requested ADS router port is already assigned",
        0x507 => "ADS router port is not registered",
        0x508 => "ADS router has reached its maximum port count",
        0x509 => "ADS router port is invalid",
        0x50A => "ADS router is not active",
        0x50B => "ADS router fragment mailbox is full",
        0x50C => "ADS router fragment timed out",
        0x50D => "ADS router port was removed",

        0x700 => "general ADS device error",
        0x701 => "ADS service is not supported by the target",
        0x702 => "invalid ADS index group",
        0x703 => "invalid ADS index offset",
        0x704 => "ADS read or write is not permitted",
        0x705 => "ADS parameter size is incorrect",
        0x706 => "ADS data values are invalid",
        0x707 => "ADS device is not ready",
        0x708 => "ADS device is busy",
        0x709 => "invalid ADS operating-system context",
        0x70A => "ADS device has insufficient memory",
        0x70B => "ADS parameter values are invalid",
        0x70C => "ADS object was not found",
        0x70D => "ADS command or file has a syntax error",
        0x70E => "ADS objects are incompatible",
        0x70F => "ADS object already exists",
        0x710 => "ADS symbol was not found",
        0x711 => "ADS symbol version is invalid; acquire a new handle",
        0x712 => "ADS device is in an invalid state",
        0x713 => "ADS transmission mode is not supported",
        0x714 => "ADS notification handle is invalid",
        0x715 => "ADS notification client is not registered",
        0x716 => "no more ADS notification handles are available",
        0x717 => "ADS notification size is too large",
        0x718 => "ADS device is not initialized",
        0x719 => "ADS device timed out",
        0x71A => "ADS interface query failed",
        0x71B => "wrong ADS interface was requested",
        0x71C => "ADS class ID is invalid",
        0x71D => "ADS object ID is invalid",
        0x71E => "ADS request is pending",
        0x71F => "ADS request was aborted",
        0x720 => "ADS device reported a warning",
        0x721 => "ADS array index is invalid",
        0x722 => "ADS symbol is not active; acquire a new handle",
        0x723 => "ADS device access was denied",
        0x724 => "required ADS device license was not found",
        0x725 => "ADS device license has expired",
        0x726 => "ADS device license limit was exceeded",
        0x727 => "ADS device license is invalid",
        0x728 => "ADS device license has an invalid system ID",
        0x729 => "ADS device license is not time-limited",
        0x72A => "ADS device license issue time is in the future",
        0x72B => "ADS device license period is too long",
        0x72C => "ADS device-specific code raised an exception",
        0x72D => "ADS device license file was read twice",
        0x72E => "ADS device license signature is invalid",
        0x72F => "ADS device public-key certificate is invalid",
        0x730 => "ADS device OEM public key is unknown",
        0x731 => "ADS device license is not valid for this system ID",
        0x732 => "ADS device demo license is prohibited",
        0x733 => "ADS device function ID is invalid",
        0x734 => "ADS device value is outside the valid range",
        0x735 => "ADS device data alignment is invalid",
        0x736 => "ADS device platform level is invalid",
        0x737 => "ADS request must be forwarded to passive level",
        0x738 => "ADS request must be forwarded to dispatch level",
        0x739 => "ADS request must be forwarded to real-time level",

        0x740 => "general ADS client error",
        0x741 => "ADS client service contains an invalid parameter",
        0x742 => "ADS client polling list is empty",
        0x743 => "ADS variable connection is already in use",
        0x744 => "ADS invoke ID is already in use",
        0x745 => {
            "ADS request timed out because the target did not respond; verify the AMS route and target service port"
        }
        0x746 => "ADS client Windows subsystem error",
        0x747 => "ADS client timeout value is invalid",
        0x748 => "ADS client port is not open",
        0x749 => "ADS client has no AMS address",
        0x750 => "internal ADS synchronous client error",
        0x751 => "ADS client hash table overflow",
        0x752 => "ADS client hash key was not found",
        0x753 => "No more symbols in cache",
        0x754 => "ADS client received an invalid response",
        0x755 => "ADS synchronous client port is locked",
        _ => "unknown ADS error",
    }
}

#[cfg(test)]
mod tests {
    use super::{ads_error_description, AdsError};

    #[test]
    fn maps_router_device_and_timeout_codes_to_stable_semantics() {
        assert_eq!(
            ads_error_description(0x500),
            "ADS router has no locked memory"
        );
        assert_eq!(ads_error_description(0x50D), "ADS router port was removed");
        assert_eq!(
            ads_error_description(0x701),
            "ADS service is not supported by the target"
        );
        assert_eq!(ads_error_description(0x702), "invalid ADS index group");
        assert_eq!(
            ads_error_description(1861),
            "ADS request timed out because the target did not respond; verify the AMS route and target service port"
        );
        assert_eq!(ads_error_description(-1), "unknown ADS error");
    }

    #[test]
    fn every_documented_router_and_ads_device_client_code_is_mapped() {
        for code in 0x500..=0x50D {
            assert_ne!(
                ads_error_description(code),
                "unknown ADS error",
                "missing router code 0x{code:03X}"
            );
        }
        let device_and_client_codes = (0x700..=0x739).chain(0x740..=0x749).chain(0x750..=0x755);
        for code in device_and_client_codes {
            assert_ne!(
                ads_error_description(code),
                "unknown ADS error",
                "missing ADS code 0x{code:03X}"
            );
        }
    }

    #[test]
    fn native_call_preserves_numeric_code_in_data_and_display() {
        let error = AdsError::native_call("AdsSyncReadStateReqEx", 1861);
        assert_eq!(
            error,
            AdsError::Call {
                operation: "AdsSyncReadStateReqEx",
                code: 1861,
                description: "ADS request timed out because the target did not respond; verify the AMS route and target service port",
            }
        );
        let rendered = error.to_string();
        assert!(rendered.contains("1861"));
        assert!(rendered.contains("0x00000745"));
        assert!(rendered.contains("target did not respond"));
    }
}
