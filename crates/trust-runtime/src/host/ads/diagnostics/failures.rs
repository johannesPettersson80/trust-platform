use serde::{Deserialize, Serialize};

use super::{AdsErrorInfo, NextAction, NextActionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingFailureKind {
    UdpIdentifyBlocked,
    #[serde(rename = "tcp_48898_blocked")]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureClassification {
    pub kind: OnboardingFailureKind,
    pub explanation: String,
    pub remediation: String,
    pub next_action: NextAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ads_error: Option<AdsErrorInfo>,
    pub blocks_production_ready: bool,
}

pub fn classify_onboarding_failure(kind: OnboardingFailureKind) -> FailureClassification {
    match kind {
        OnboardingFailureKind::UdpIdentifyBlocked => classification(
            kind,
            "The PLC did not answer ADS discovery.",
            "Enter the PLC IP manually or check UDP 48899 and firewall rules.",
            NextActionKind::PickTarget,
        ),
        OnboardingFailureKind::Tcp48898Blocked => classification(
            kind,
            "TwinCAT router TCP port 48898 is not reachable.",
            "Check the network path and firewall on the TwinCAT target.",
            NextActionKind::FixLocalIp,
        ),
        OnboardingFailureKind::RouteMissing => classification(
            kind,
            "The PLC does not trust this truST runtime host yet.",
            "Add a static ADS route on the PLC for this truST runtime host.",
            NextActionKind::AddRoute,
        ),
        OnboardingFailureKind::WrongAmsNetId => classification(
            kind,
            "The IP is reachable, but the ADS identity does not match the configured target AMS Net ID.",
            "Use the detected AMS Net ID or correct the target configuration.",
            NextActionKind::PickTarget,
        ),
        OnboardingFailureKind::WrongPlcPort => classification(
            kind,
            "TwinCAT router works, but the selected PLC runtime port did not respond.",
            "Try PLC port 851, 852, or select the correct TwinCAT runtime.",
            NextActionKind::PickTarget,
        ),
        OnboardingFailureKind::SecureRequired => classification(
            kind,
            "The target requires Secure ADS, which truST does not support yet.",
            "Use OPC UA for secure integration or change the target ADS policy.",
            NextActionKind::UseSecure,
        ),
        OnboardingFailureKind::NoSymbols => classification(
            kind,
            "Symbol upload worked, but no compatible TwinCAT symbols were found.",
            "Check the PLC project and symbol export settings.",
            NextActionKind::None,
        ),
        OnboardingFailureKind::CredentialsRejected => classification(
            kind,
            "TwinCAT refused the route credentials.",
            "Retry the credentials or use the generated PowerShell route artifact.",
            NextActionKind::AddRoute,
        ),
        OnboardingFailureKind::NatOrPublic => classification(
            kind,
            "The runtime host appears to be behind NAT or public routing.",
            "Use a private OT network or VPN path where the PLC can route back to the runtime host.",
            NextActionKind::FixLocalIp,
        ),
        OnboardingFailureKind::Fingerprint1861 => classification(
            kind,
            "TwinCAT's GUI fingerprint handshake failed for this non-Windows client path.",
            "Use truST route setup or generated route artifacts instead of TwinCAT Broadcast Search.",
            NextActionKind::AddRoute,
        )
        .with_ads_error(AdsErrorInfo::new(1861, "ADSERR_DEVICE_INVALIDCONTEXT")),
        OnboardingFailureKind::NotificationFailure => classification(
            kind,
            "ADS notification subscription did not deliver samples.",
            "Use cyclic polling for this point or check TwinCAT notification settings.",
            NextActionKind::None,
        ),
        OnboardingFailureKind::UnsupportedOperation => classification(
            kind,
            "This ADS onboarding operation is not implemented in the current build.",
            "Use the available setup step or update truST to a build that supports this operation.",
            NextActionKind::None,
        ),
    }
}

pub fn classify_ads_error_code(code: u32) -> Option<FailureClassification> {
    match code {
        1861 => Some(classify_onboarding_failure(
            OnboardingFailureKind::Fingerprint1861,
        )),
        _ => None,
    }
}

fn classification(
    kind: OnboardingFailureKind,
    explanation: &str,
    remediation: &str,
    next_action: NextActionKind,
) -> FailureClassification {
    FailureClassification {
        kind,
        explanation: explanation.to_string(),
        remediation: remediation.to_string(),
        next_action: NextAction::new(next_action),
        ads_error: None,
        blocks_production_ready: true,
    }
}

impl FailureClassification {
    fn with_ads_error(mut self, ads_error: AdsErrorInfo) -> Self {
        self.ads_error = Some(ads_error);
        self
    }
}
