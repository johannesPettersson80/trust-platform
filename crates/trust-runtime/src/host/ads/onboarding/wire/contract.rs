use crate::ads::diagnostics::{LocalIdentity, TargetIdentity};
use crate::ads::onboarding::errors::OnboardingWireError;
use crate::ads::onboarding::route::{RouteAddRequest, RouteRemoveRequest};
use trust_ads_core::{AdsDataTypeDescriptor, QualityState, SymbolDescriptor};
use trust_runtime_core::value::Value;

/// Thin setup/control-plane wire boundary for ADS onboarding.
///
/// Real ADS/socket calls belong behind this trait. Business logic and public
/// schema types must not expose raw wire crate types.
pub trait AdsOnboardingWire {
    fn udp_identify(&mut self, target_ip: &str) -> Result<TargetIdentity, OnboardingWireError>;
    fn directed_identity(
        &mut self,
        target_ip: &str,
    ) -> Result<DirectedIdentityObservation, OnboardingWireError> {
        self.udp_identify(target_ip)
            .map(|target| DirectedIdentityObservation {
                identity: ObservedAdsIdentity::identity_only(target),
                transport: DirectedIdentityTransport::Udp,
                warnings: Vec::new(),
            })
    }
    fn directed_identities(
        &mut self,
        target_ip: &str,
    ) -> Result<Vec<DirectedIdentityObservation>, OnboardingWireError> {
        self.directed_identity(target_ip)
            .map(|identity| vec![identity])
    }
    fn udp_identify_all(
        &mut self,
        target_ip: &str,
    ) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
        self.udp_identify(target_ip).map(|target| vec![target])
    }
    fn tcp_probe_48898(&mut self, target_ip: &str) -> Result<(), OnboardingWireError>;
    fn probe_ads_router(
        &mut self,
        target: &TargetIdentity,
    ) -> Result<AdsRouterProbe, OnboardingWireError> {
        self.tcp_probe_48898(&target.ip)?;
        Ok(AdsRouterProbe::Tcp48898)
    }
    fn route_requirement(&self, _target_ip: &str) -> AdsRouteRequirement {
        AdsRouteRequirement::ReciprocalRouteRequired
    }
    /// Actual source AMS address assigned to the connected native client port.
    ///
    /// Remote transports and native implementations that cannot expose the
    /// router-assigned address return `None`; callers must not substitute a
    /// routed-IP or configured identity and label it as native evidence.
    fn native_source_address(&self) -> Option<NativeAmsSourceAddress> {
        None
    }
    fn check_route(
        &mut self,
        target: &TargetIdentity,
        local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError>;
    fn verify_ams_target(&mut self, target: &TargetIdentity) -> Result<(), OnboardingWireError>;
    fn read_state(&mut self, target: &TargetIdentity) -> Result<String, OnboardingWireError>;
    fn upload_symbols(
        &mut self,
        target: &TargetIdentity,
    ) -> Result<Vec<SymbolDescriptor>, OnboardingWireError>;
    fn resolve_handle(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<u32, OnboardingWireError>;
    fn sumup_read(
        &mut self,
        target: &TargetIdentity,
        handles: &[u32],
    ) -> Result<Vec<Vec<u8>>, OnboardingWireError>;
    fn guarded_write_probe(
        &mut self,
        target: &TargetIdentity,
        probe: &GuardedWriteProbe,
    ) -> Result<(), OnboardingWireError>;
    fn subscribe_notification(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<(), OnboardingWireError>;
    fn read_update_sample(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<AdsReadUpdateSample, OnboardingWireError>;
    fn symbol_version(&mut self, target: &TargetIdentity) -> Result<u32, OnboardingWireError>;
    fn add_route(&mut self, request: &RouteAddRequest) -> Result<(), OnboardingWireError>;
    fn remove_route(&mut self, request: &RouteRemoveRequest) -> Result<(), OnboardingWireError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectedIdentityTransport {
    Udp,
    LocalRouter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsRouterProbe {
    Tcp48898,
    NativeRouterRoundTrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsRouteRequirement {
    ReciprocalRouteRequired,
    NativeLocalRouter,
}

/// Source AMS address assigned by the native TwinCAT router connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeAmsSourceAddress {
    pub ams_net_id: String,
    pub ams_port: u16,
}

impl NativeAmsSourceAddress {
    #[must_use]
    pub fn new(ams_net_id: impl Into<String>, ams_port: u16) -> Self {
        Self {
            ams_net_id: ams_net_id.into(),
            ams_port,
        }
    }

    #[must_use]
    pub fn differs_from_target(&self, target: &TargetIdentity) -> bool {
        self.ams_net_id != target.ams_net_id || self.ams_port != target.ams_port
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectedIdentityObservation {
    pub identity: ObservedAdsIdentity,
    pub transport: DirectedIdentityTransport,
    pub warnings: Vec<String>,
}

/// Identity and service-port evidence produced by discovery. A UDP identity or
/// router-system reply can exist without any confirmed user ADS service.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservedAdsIdentity {
    pub name: Option<String>,
    pub ip: String,
    pub ams_net_id: String,
    pub preferred_ams_port: Option<u16>,
    pub responding_ads_ports: Vec<u16>,
    pub tc_version: Option<String>,
}

impl ObservedAdsIdentity {
    #[must_use]
    pub fn identity_only(target: TargetIdentity) -> Self {
        Self {
            name: target.name,
            ip: target.ip,
            ams_net_id: target.ams_net_id,
            preferred_ams_port: None,
            responding_ads_ports: Vec::new(),
            tc_version: target.tc_version,
        }
    }

    #[must_use]
    pub fn responding(target: TargetIdentity) -> Self {
        let ams_port = target.ams_port;
        Self {
            name: target.name,
            ip: target.ip,
            ams_net_id: target.ams_net_id,
            preferred_ams_port: Some(ams_port),
            responding_ads_ports: vec![ams_port],
            tc_version: target.tc_version,
        }
    }

    #[must_use]
    pub fn into_responding_target(self) -> Option<TargetIdentity> {
        let ams_port = self.preferred_ams_port?;
        if !self.responding_ads_ports.contains(&ams_port) {
            return None;
        }
        Some(TargetIdentity {
            name: self.name,
            ip: self.ip,
            ams_net_id: self.ams_net_id,
            ams_port,
            tc_version: self.tc_version,
        })
    }
}

/// Explicit, reversible write probe requested by a user.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardedWriteProbe {
    pub symbol: String,
    pub data_type: AdsDataTypeDescriptor,
    pub value: Value,
}

/// Evidence that a subscribed ADS value was actually read after registration.
/// The value itself is intentionally omitted from diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsReadUpdateSample {
    pub point_name: String,
    pub subscription_id: u32,
    pub quality: QualityState,
}
