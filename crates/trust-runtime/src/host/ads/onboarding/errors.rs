use core::fmt;

use crate::ads::diagnostics::{
    classify_onboarding_failure, AdsErrorInfo, FailureClassification, OnboardingFailureKind,
};

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
}

impl OnboardingWireError {
    pub fn new(kind: OnboardingWireErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            ads_error: None,
        }
    }

    pub fn with_ads_error(mut self, code: u32, name: impl Into<String>) -> Self {
        self.ads_error = Some(AdsErrorInfo::new(code, name));
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
