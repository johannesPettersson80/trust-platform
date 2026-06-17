use core::fmt;

/// ADS device error codes returned by the v1 server surface.
///
/// Numeric values follow Beckhoff `AdsDef.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AdsErrorCode {
    /// No error.
    NoError = 0x0000,
    /// Service is not supported by server.
    ServiceNotSupported = 0x0701,
    /// Invalid index group.
    InvalidGroup = 0x0702,
    /// Invalid index offset.
    InvalidOffset = 0x0703,
    /// Reading or writing is not permitted.
    InvalidAccess = 0x0704,
    /// Parameter size is not correct.
    InvalidSize = 0x0705,
    /// Invalid parameter data.
    InvalidData = 0x0706,
    /// Device is not ready.
    NotReady = 0x0707,
    /// Out of memory or configured resource cap exhausted.
    NoMemory = 0x070A,
    /// Invalid parameter.
    InvalidParameter = 0x070B,
    /// Object not found.
    NotFound = 0x070C,
    /// Symbol not found.
    SymbolNotFound = 0x0710,
    /// Symbol version invalid.
    SymbolVersionInvalid = 0x0711,
    /// Server is in invalid state.
    InvalidState = 0x0712,
    /// ADS transmission mode is not supported.
    TransmissionModeNotSupported = 0x0713,
    /// Notification handle is invalid.
    NotificationHandleInvalid = 0x0714,
    /// Notification client is unknown.
    ClientUnknown = 0x0715,
    /// No more handles available.
    NoMoreHandles = 0x0716,
    /// Watch size is invalid.
    InvalidWatchSize = 0x0717,
    /// Access denied.
    AccessDenied = 0x0723,
}

impl AdsErrorCode {
    /// Returns the numeric ADS error value.
    #[must_use]
    pub const fn value(self) -> u32 {
        self as u32
    }
}

/// Error produced by the ADS server protocol/integration boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsServerError {
    code: AdsErrorCode,
    message: String,
}

impl AdsServerError {
    /// Creates a device-level ADS error.
    #[must_use]
    pub fn device(code: AdsErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Returns the ADS error code.
    #[must_use]
    pub const fn code(&self) -> AdsErrorCode {
        self.code
    }

    /// Returns the human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AdsServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} (0x{:04X}): {}",
            self.code,
            self.code.value(),
            self.message
        )
    }
}

impl std::error::Error for AdsServerError {}

#[cfg(test)]
mod tests {
    use super::{AdsErrorCode, AdsServerError};

    #[test]
    fn service_not_supported_uses_beckhoff_srvnotsupp_value() {
        assert_eq!(AdsErrorCode::ServiceNotSupported.value(), 0x0701);

        let error = AdsServerError::device(AdsErrorCode::ServiceNotSupported, "not implemented");
        assert!(error.to_string().contains("0x0701"));
        assert!(!error.to_string().contains("SERVICENOTSUPP"));
    }
}
