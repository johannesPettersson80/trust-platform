use serde::{Deserialize, Serialize};

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::onboarding::errors::OnboardingWireError;
use crate::ads::onboarding::identity::RuntimeAddressCandidate;
use crate::ads::onboarding::wire::AdsOnboardingWire;

#[path = "discover/interface_targets.rs"]
mod interface_targets;
pub use interface_targets::interface_directed_targets;

/// Source that produced an ADS target discovery result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySource {
    Manual,
    DirectedIdentify,
    DirectedBroadcast,
}

/// Discovery request accepted by the onboarding engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryRequest {
    /// Optional IP/hostname entered by the user or selected by UI.
    pub target: Option<String>,
    /// Additional runtime-host interface addresses to probe with directed identify.
    #[serde(default)]
    pub directed_targets: Vec<String>,
    /// Optional manually supplied target AMS Net ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ams_net_id: Option<String>,
    /// Target PLC AMS port. Defaults to 851 when absent.
    #[serde(default)]
    pub ams_port: Option<u16>,
    /// Optional friendly target name supplied by manual entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    /// Whether directed broadcast candidates should be tried.
    #[serde(default)]
    pub include_broadcast: bool,
    /// Directed broadcast addresses already derived from runtime-host interfaces.
    #[serde(default)]
    pub broadcast_targets: Vec<String>,
    /// Optional discovery timeout budget in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

impl DiscoveryRequest {
    pub fn manual(target: impl Into<String>) -> Self {
        Self {
            target: Some(target.into()),
            directed_targets: Vec::new(),
            target_ams_net_id: None,
            ams_port: None,
            target_name: None,
            include_broadcast: false,
            broadcast_targets: Vec::new(),
            timeout_ms: None,
        }
    }

    pub fn manual_with_ams_net_id(
        target: impl Into<String>,
        ams_net_id: impl Into<String>,
    ) -> Self {
        Self {
            target: Some(target.into()),
            directed_targets: Vec::new(),
            target_ams_net_id: Some(ams_net_id.into()),
            ams_port: None,
            target_name: None,
            include_broadcast: false,
            broadcast_targets: Vec::new(),
            timeout_ms: None,
        }
    }
}

/// One ADS discovery result with its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryResult {
    /// Discovered or manually declared target identity.
    pub target: TargetIdentity,
    /// Path that produced this result.
    pub source: DiscoverySource,
}

/// Discovers ADS targets using manual, directed identify, and directed broadcast paths.
///
/// Manual targets with a supplied AMS Net ID do not require broadcast or UDP
/// identify to succeed. This keeps IP/hostname entry as a first-class path for
/// routed networks where UDP discovery is blocked.
pub fn discover_targets<W: AdsOnboardingWire>(
    wire: &mut W,
    request: &DiscoveryRequest,
) -> Result<Vec<DiscoveryResult>, OnboardingWireError> {
    let mut results = Vec::new();

    if let Some(target) = request
        .target
        .as_deref()
        .filter(|target| !target.is_empty())
    {
        if let Some(ams_net_id) = request.target_ams_net_id.as_ref() {
            results.push(DiscoveryResult {
                target: TargetIdentity {
                    name: request.target_name.clone(),
                    ip: target.to_string(),
                    ams_net_id: ams_net_id.clone(),
                    ams_port: request.ams_port.unwrap_or(851),
                    tc_version: None,
                },
                source: DiscoverySource::Manual,
            });
        } else {
            match wire.udp_identify(target) {
                Ok(mut identity) => {
                    if identity.ip.is_empty() {
                        identity.ip = target.to_string();
                    }
                    if let Some(port) = request.ams_port {
                        identity.ams_port = port;
                    }
                    results.push(DiscoveryResult {
                        target: identity,
                        source: DiscoverySource::DirectedIdentify,
                    });
                }
                Err(_) if request.include_broadcast => {}
                Err(error) => return Err(error),
            }
        }
    }

    if results.is_empty() {
        for target in &request.directed_targets {
            if request.target.as_deref() == Some(target.as_str()) {
                continue;
            }
            let Ok(mut identity) = wire.udp_identify(target) else {
                continue;
            };
            if identity.ip.is_empty() {
                identity.ip = target.clone();
            }
            if let Some(port) = request.ams_port {
                identity.ams_port = port;
            }
            push_unique(
                &mut results,
                DiscoveryResult {
                    target: identity,
                    source: DiscoverySource::DirectedIdentify,
                },
            );
            break;
        }
    }

    if request.include_broadcast {
        for target in &request.broadcast_targets {
            match wire.udp_identify_all(target) {
                Ok(identities) => {
                    for mut identity in identities {
                        if identity.ip.is_empty() {
                            identity.ip = target.clone();
                        }
                        if let Some(port) = request.ams_port {
                            identity.ams_port = port;
                        }
                        push_unique(
                            &mut results,
                            DiscoveryResult {
                                target: identity,
                                source: DiscoverySource::DirectedBroadcast,
                            },
                        );
                    }
                }
                Err(_) => continue,
            }
        }
    }

    Ok(results)
}

/// Computes the directed broadcast target for an IPv4 interface address.
pub fn directed_broadcast_target(ip: &str, prefix_len: u8) -> Option<String> {
    if prefix_len > 32 {
        return None;
    }
    let ip: std::net::Ipv4Addr = ip.parse().ok()?;
    let raw = u32::from(ip);
    let mask = if prefix_len == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix_len))
    };
    Some(std::net::Ipv4Addr::from(raw | !mask).to_string())
}

/// Returns directed broadcast targets for runtime-host interface candidates.
pub fn directed_broadcast_targets_from_candidates(
    candidates: &[RuntimeAddressCandidate],
) -> Vec<String> {
    let mut targets = Vec::new();
    for candidate in candidates {
        if !is_broadcast_discovery_source(&candidate.ip) {
            continue;
        }
        let target = candidate
            .broadcast
            .as_deref()
            .filter(|reported| *reported != candidate.ip)
            .map(ToString::to_string)
            .or_else(|| directed_broadcast_target(&candidate.ip, candidate.prefix_len?))
            .filter(|target| target != &candidate.ip);
        if let Some(target) = target {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

fn is_broadcast_discovery_source(ip: &str) -> bool {
    let Ok(addr) = ip.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    !addr.is_loopback() && !addr.is_link_local()
}

fn push_unique(results: &mut Vec<DiscoveryResult>, result: DiscoveryResult) {
    let seen = results.iter().any(|existing| {
        existing.target.ams_net_id == result.target.ams_net_id
            && existing.target.ams_port == result.target.ams_port
    });
    if !seen {
        results.push(result);
    }
}

#[cfg(test)]
#[path = "discover/local_fallback_tests.rs"]
mod local_fallback_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ads::onboarding::wire::{MockAdsOnboardingScenario, MockAdsOnboardingWire};

    fn result(ip: &str, ams_net_id: &str, ams_port: u16) -> DiscoveryResult {
        DiscoveryResult {
            target: TargetIdentity {
                name: None,
                ip: ip.to_string(),
                ams_net_id: ams_net_id.to_string(),
                ams_port,
                tc_version: None,
            },
            source: DiscoverySource::DirectedBroadcast,
        }
    }

    #[test]
    fn discovery_deduplicates_by_ads_endpoint_not_host_ip() {
        let mut results = vec![result("192.168.10.5", "1.2.3.4.5.6", 851)];

        push_unique(&mut results, result("192.168.10.5", "1.2.3.4.5.7", 851));
        assert_eq!(results.len(), 2, "distinct same-host runtime was lost");

        push_unique(&mut results, result("192.168.10.99", "1.2.3.4.5.7", 851));
        assert_eq!(results.len(), 2, "same ADS endpoint was duplicated");

        push_unique(&mut results, result("192.168.10.5", "1.2.3.4.5.7", 852));
        assert_eq!(results.len(), 3, "distinct AMS port was lost");
    }

    #[test]
    fn broadcast_failure_preserves_successful_manual_identity() {
        let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::WrongIp);
        let mut request = DiscoveryRequest::manual_with_ams_net_id("192.168.10.5", "1.2.3.4.5.6");
        request.include_broadcast = true;
        request.broadcast_targets = vec!["192.168.10.255".to_string()];

        let results = discover_targets(&mut wire, &request).expect("partial discovery result");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, DiscoverySource::Manual);
        assert_eq!(results[0].target.ams_net_id, "1.2.3.4.5.6");
    }
}
