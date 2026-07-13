use core::fmt;

use crate::ads::diagnostics::{
    classify_onboarding_failure, AdsErrorInfo, FailureClassification, OnboardingFailureKind,
};
use crate::ads::AdsTransportFailureKind;

/// Error returned by onboarding engine orchestration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingError {
    message: String,
}

impl OnboardingError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for OnboardingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for OnboardingError {}

/// Field failure kind reported by the onboarding wire boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingWireErrorKind {
    UdpIdentifyBlocked,
    LocalRouterUnavailable,
    Tcp48898Blocked,
    RouteMissing,
    WrongAmsNetId,
    WrongPlcPort,
    SecureRequired,
    NoSymbols,
    CredentialsRejected,
    NatOrPublic,
    Fingerprint1861,
    NotificationFailure,
    UnsupportedOperation,
}

impl OnboardingWireErrorKind {
    pub fn classification(self) -> FailureClassification {
        classify_onboarding_failure(match self {
            Self::UdpIdentifyBlocked => OnboardingFailureKind::UdpIdentifyBlocked,
            Self::LocalRouterUnavailable => OnboardingFailureKind::LocalRouterUnavailable,
            Self::Tcp48898Blocked => OnboardingFailureKind::Tcp48898Blocked,
            Self::RouteMissing => OnboardingFailureKind::RouteMissing,
            Self::WrongAmsNetId => OnboardingFailureKind::WrongAmsNetId,
            Self::WrongPlcPort => OnboardingFailureKind::WrongPlcPort,
            Self::SecureRequired => OnboardingFailureKind::SecureRequired,
            Self::NoSymbols => OnboardingFailureKind::NoSymbols,
            Self::CredentialsRejected => OnboardingFailureKind::CredentialsRejected,
            Self::NatOrPublic => OnboardingFailureKind::NatOrPublic,
            Self::Fingerprint1861 => OnboardingFailureKind::Fingerprint1861,
            Self::NotificationFailure => OnboardingFailureKind::NotificationFailure,
            Self::UnsupportedOperation => OnboardingFailureKind::UnsupportedOperation,
        })
    }
}

/// Error returned by the setup wire boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingWireError {
    pub kind: OnboardingWireErrorKind,
    pub detail: String,
    pub ads_error: Option<AdsErrorInfo>,
    pub transport_failure: Option<AdsTransportFailureKind>,
}

impl OnboardingWireError {
    pub fn new(kind: OnboardingWireErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            ads_error: None,
            transport_failure: None,
        }
    }

    pub fn with_ads_error(mut self, code: u32, name: impl Into<String>) -> Self {
        self.ads_error = Some(AdsErrorInfo::new(code, name));
        self
    }

    #[must_use]
    pub fn with_transport_failure(mut self, failure: AdsTransportFailureKind) -> Self {
        self.transport_failure = Some(failure);
        self
    }

    pub fn classification(&self) -> FailureClassification {
        let mut classification = self.kind.classification();
        if let Some(error) = &self.ads_error {
            classification.ads_error = Some(error.clone());
        }
        classification
    }
}

impl fmt::Display for OnboardingWireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for OnboardingWireError {}

/// Returns true when a symbol-upload failure is most likely caused by a missing
/// return AMS route rather than by a valid PLC response with an empty table.
#[must_use]
pub fn upload_failure_implies_missing_return_route(error: &OnboardingWireError) -> bool {
    match error.kind {
        OnboardingWireErrorKind::RouteMissing => true,
        OnboardingWireErrorKind::NoSymbols => structured_failure_implies_no_reply(error),
        _ => false,
    }
}

/// Returns true when a harmless ADS request could be sent but no reply came
/// back to the runtime host. A TCP connect alone cannot prove an AMS return
/// route because the router accepts the socket before it validates reply
/// routing.
pub(crate) fn reply_failure_implies_missing_return_route(error: &OnboardingWireError) -> bool {
    match error.kind {
        OnboardingWireErrorKind::RouteMissing => true,
        OnboardingWireErrorKind::WrongPlcPort | OnboardingWireErrorKind::NoSymbols => {
            structured_failure_implies_no_reply(error)
        }
        _ => false,
    }
}

fn structured_failure_implies_no_reply(error: &OnboardingWireError) -> bool {
    matches!(
        error.transport_failure,
        Some(AdsTransportFailureKind::TimedOut | AdsTransportFailureKind::HostUnreachable)
    ) || error
        .ads_error
        .as_ref()
        .is_some_and(|error| matches!(error.code, 0x007 | 0x015 | 0x01B | 0x745))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_timeout_is_classified_as_missing_return_route() {
        let error = OnboardingWireError::new(
            OnboardingWireErrorKind::NoSymbols,
            "localized transport failure",
        )
        .with_transport_failure(AdsTransportFailureKind::TimedOut);
        assert!(upload_failure_implies_missing_return_route(&error));
    }

    #[test]
    fn read_state_timeout_is_classified_as_missing_return_route() {
        let error = OnboardingWireError::new(
            OnboardingWireErrorKind::WrongPlcPort,
            "localized transport failure",
        )
        .with_ads_error(0x745, "ADS client synchronous timeout");
        assert!(reply_failure_implies_missing_return_route(&error));
    }

    #[test]
    fn empty_symbol_table_response_is_not_route_missing() {
        let error = OnboardingWireError::new(
            OnboardingWireErrorKind::NoSymbols,
            "ADS symbol table is empty",
        );
        assert!(!upload_failure_implies_missing_return_route(&error));
    }

    #[test]
    fn connection_refused_is_not_route_missing() {
        let error = OnboardingWireError::new(
            OnboardingWireErrorKind::NoSymbols,
            "localized transport failure",
        )
        .with_transport_failure(AdsTransportFailureKind::ConnectionRefused);
        assert!(!upload_failure_implies_missing_return_route(&error));
    }

    #[test]
    fn error_text_alone_never_drives_route_recovery() {
        let error = OnboardingWireError::new(
            OnboardingWireErrorKind::NoSymbols,
            "receiving reply (route set?): timed out",
        );

        assert!(!upload_failure_implies_missing_return_route(&error));
    }
}
