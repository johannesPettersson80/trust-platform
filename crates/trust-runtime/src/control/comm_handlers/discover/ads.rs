use std::net::IpAddr;
#[cfg(feature = "ads-wire")]
use std::time::Duration;

use serde_json::json;

#[cfg(feature = "ads-wire")]
use super::timeout;
use super::{parse_ipv4_cidr, sanitize_id, DiscoverCandidate, DiscoverScope};
use crate::ads::onboarding::{
    directed_broadcast_target, directed_broadcast_targets_from_candidates, discover_targets_report,
    runtime_address_candidates_from_interfaces, AdsOnboardingWire, DiscoveryRequest,
    DiscoveryResult, DiscoverySource, ObservedAdsIdentity, OnboardingError,
    RuntimeAddressCandidate,
};

#[cfg(feature = "ads-wire")]
const DEFAULT_ADS_DIRECTED_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "ads-wire")]
const DEFAULT_ADS_BROADCAST_WINDOW: Duration = Duration::from_millis(900);

#[cfg(feature = "ads-wire")]
pub(super) fn discover_ads(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let request = discovery_request(scope, warnings)?;
    let (directed_timeout, broadcast_window) = ads_discovery_timeouts(scope);
    let mut wire = crate::ads::onboarding::AdsRsOnboardingWire::with_discovery_timeouts(
        directed_timeout,
        broadcast_window,
    );
    discover_ads_request(&mut wire, &request, warnings)
}

#[cfg(feature = "ads-wire")]
fn ads_discovery_timeouts(scope: &DiscoverScope) -> (Duration, Duration) {
    if scope.timeout_ms.is_some() {
        let requested = timeout(scope);
        (requested, requested)
    } else {
        (DEFAULT_ADS_DIRECTED_TIMEOUT, DEFAULT_ADS_BROADCAST_WINDOW)
    }
}

#[cfg(not(feature = "ads-wire"))]
pub(super) fn discover_ads(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let request = discovery_request(scope, warnings)?;
    if request.target_ams_net_id.is_none() || request.include_broadcast {
        warnings.push("ADS wire discovery needs a runtime built with the ads-wire feature.".into());
        return Ok(Vec::new());
    }
    let mut wire = crate::ads::onboarding::MockAdsOnboardingWire::default();
    discover_ads_request(&mut wire, &request, warnings)
}

fn discover_ads_request<W: AdsOnboardingWire>(
    wire: &mut W,
    request: &DiscoveryRequest,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let report = discover_targets_report(wire, request)
        .map_err(|error| format!("ADS discovery failed: {error}"))?;
    warnings.extend(report.warnings);
    Ok(discovery_candidates(report.results))
}

fn discovery_request(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<DiscoveryRequest, String> {
    discovery_request_with_interface_candidates(
        scope,
        warnings,
        runtime_address_candidates_from_interfaces,
    )
}

fn discovery_request_with_interface_candidates<F>(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
    interface_candidates: F,
) -> Result<DiscoveryRequest, String>
where
    F: FnOnce() -> Result<Vec<RuntimeAddressCandidate>, OnboardingError>,
{
    let explicit_target = scope
        .host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(ToString::to_string);
    if let Some(host) = explicit_target.as_deref() {
        validate_host_without_port(host)?;
    }
    let target_ams_net_id = scope
        .target_ams_net_id
        .as_deref()
        .map(str::trim)
        .filter(|net_id| !net_id.is_empty())
        .map(ToString::to_string);
    if let Some(net_id) = target_ams_net_id.as_deref() {
        validate_ams_net_id(net_id)?;
    }
    if explicit_target.is_none() && target_ams_net_id.is_some() {
        return Err(
            "ADS manual discovery needs a Host or IP when target_ams_net_id is supplied."
                .to_string(),
        );
    }

    let include_broadcast = explicit_target.is_none();
    let target = explicit_target.or_else(|| Some("127.0.0.1".to_string()));
    let mut request = DiscoveryRequest {
        target,
        target_ams_net_id,
        ams_port: scope.ams_port.map(|port| port.get()),
        target_name: None,
        include_broadcast,
        broadcast_targets: Vec::new(),
        timeout_ms: scope.timeout_ms,
    };
    let mut interface_enumeration_failed = false;
    if let Some(cidr) = scope.cidr.as_deref() {
        request.include_broadcast = true;
        request.broadcast_targets = vec![broadcast_target_for_cidr(cidr)?];
    } else if request.include_broadcast {
        match interface_candidates() {
            Ok(candidates) => {
                request.broadcast_targets = directed_broadcast_targets_from_candidates(&candidates);
            }
            Err(error) => {
                interface_enumeration_failed = true;
                warnings.push(format!(
                    "Could not enumerate network interfaces for ADS LAN discovery: {error}. This computer will still be searched."
                ));
            }
        }
    }
    if request.include_broadcast
        && request.broadcast_targets.is_empty()
        && !interface_enumeration_failed
    {
        warnings
            .push("No non-loopback IPv4 broadcast target was available for ADS discovery.".into());
    }
    Ok(request)
}

fn validate_host_without_port(host: &str) -> Result<(), String> {
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    let has_inline_port = host.rsplit_once(':').is_some_and(|(name, port)| {
        !name.is_empty() && !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit())
    });
    if has_inline_port {
        return Err(format!(
            "ADS discovery accepts a Host or IP only; remove the inline port from '{host}' and use the separate ADS port setting."
        ));
    }
    Ok(())
}

fn validate_ams_net_id(net_id: &str) -> Result<(), String> {
    let parts = net_id.split('.').collect::<Vec<_>>();
    if parts.len() != 6 || parts.iter().any(|part| part.parse::<u8>().is_err()) {
        return Err(
            "AMS Net ID must contain six numbers 0-255 separated by dots, for example 192.168.10.20.1.1."
                .to_string(),
        );
    }
    Ok(())
}

fn discovery_candidates(results: Vec<DiscoveryResult>) -> Vec<DiscoverCandidate> {
    struct CandidateGroup {
        target: ObservedAdsIdentity,
        source: DiscoverySource,
        preferred_port: Option<u16>,
        ports: Vec<u16>,
    }

    let mut groups: Vec<CandidateGroup> = Vec::new();
    for result in results {
        let same_device = |group: &&mut CandidateGroup| {
            group.target.ams_net_id == result.target.ams_net_id
                || ((group.target.ams_net_id.is_empty() || result.target.ams_net_id.is_empty())
                    && group.target.ip == result.target.ip)
        };
        if let Some(group) = groups.iter_mut().find(same_device) {
            for port in result.target.responding_ads_ports {
                if !group.ports.contains(&port) {
                    group.ports.push(port);
                }
            }
            if group.preferred_port.is_none() {
                group.preferred_port = result.target.preferred_ams_port;
            }
        } else {
            let preferred_port = result.target.preferred_ams_port;
            let ports = result.target.responding_ads_ports.clone();
            groups.push(CandidateGroup {
                target: result.target,
                source: result.source,
                preferred_port,
                ports,
            });
        }
    }

    groups
        .into_iter()
        .map(|mut group| {
            group.ports.sort_by_key(|port| ads_port_sort_key(*port));
            let (source, confidence, mut warnings) = match group.source {
                DiscoverySource::Manual => (
                    "manual",
                    "declared",
                    vec!["Target identity was declared manually and has not been verified.".into()],
                ),
                DiscoverySource::LocalRouter => ("ads_local_router", "observed", Vec::new()),
                DiscoverySource::DirectedIdentify => ("ads_identify", "observed", Vec::new()),
                DiscoverySource::DirectedBroadcast => ("ads_broadcast", "observed", Vec::new()),
            };
            let observed_ports = group.ports.clone();
            let preferred_port = observed_ports.first().copied().or(group.preferred_port);
            let service_status = if !observed_ports.is_empty() {
                "responding"
            } else if group.source == DiscoverySource::Manual {
                "declared"
            } else {
                warnings.push(
                    "ADS identity responded, but no user ADS service port has been confirmed."
                        .to_string(),
                );
                "identity_only"
            };
            let target = group.target;
            let base_label = target
                .name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .map_or_else(
                    || format!("ADS device {}", target.ams_net_id),
                    |name| format!("{name} · {}", target.ams_net_id),
                );
            let label = if observed_ports.is_empty() {
                if service_status == "identity_only" {
                    format!("{base_label} · identity found")
                } else {
                    base_label
                }
            } else {
                format!(
                    "{base_label} · ADS {}",
                    observed_ports
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let identity_key = if target.ams_net_id.is_empty() {
                target.ip.as_str()
            } else {
                target.ams_net_id.as_str()
            };
            DiscoverCandidate {
                id: format!("ads:{}", sanitize_id(identity_key)),
                label,
                source,
                confidence,
                params: json!({
                    "ams_net_id": target.ams_net_id,
                    "host": target.ip,
                    "name": target.name,
                    "ams_port": preferred_port,
                    "responding_ads_ports": observed_ports,
                    "ads_service_status": service_status,
                    "tc_version": target.tc_version,
                }),
                warnings,
            }
        })
        .collect()
}

fn ads_port_sort_key(port: u16) -> (u8, u16) {
    let priority = match port {
        851 => 0,
        852 => 1,
        853 => 2,
        854 => 3,
        301 => 4,
        501 => 5,
        _ => 6,
    };
    (priority, port)
}

fn broadcast_target_for_cidr(cidr: &str) -> Result<String, String> {
    let (ip, prefix) = parse_ipv4_cidr(cidr)?;
    directed_broadcast_target(&ip.to_string(), prefix)
        .ok_or_else(|| format!("could not compute directed broadcast target for {cidr}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ads::onboarding::{MockAdsOnboardingScenario, MockAdsOnboardingWire};

    #[cfg(feature = "ads-wire")]
    #[test]
    fn default_ads_timeouts_reserve_native_scan_time_without_slowing_each_lan_window() {
        assert_eq!(
            ads_discovery_timeouts(&DiscoverScope::default()),
            (Duration::from_secs(5), Duration::from_millis(900))
        );

        let explicit = DiscoverScope {
            timeout_ms: Some(250),
            ..DiscoverScope::default()
        };
        assert_eq!(
            ads_discovery_timeouts(&explicit),
            (Duration::from_millis(250), Duration::from_millis(250))
        );
    }

    #[test]
    fn default_ads_request_searches_this_computer_and_the_lan() {
        let scope = DiscoverScope {
            cidr: Some("192.168.77.0/24".to_string()),
            ..DiscoverScope::default()
        };
        let mut warnings = Vec::new();

        let request = discovery_request(&scope, &mut warnings).expect("automatic ADS request");

        assert_eq!(request.target.as_deref(), Some("127.0.0.1"));
        assert_eq!(request.ams_port, None);
        assert!(request.include_broadcast);
        assert_eq!(request.broadcast_targets, vec!["192.168.77.255"]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn responding_ports_are_grouped_into_one_deterministic_ads_device() {
        let result = |port| DiscoveryResult {
            target: ObservedAdsIdentity {
                name: Some("Local controller".to_string()),
                ip: "127.0.0.1".to_string(),
                ams_net_id: "10.20.30.40.1.1".to_string(),
                preferred_ams_port: Some(port),
                responding_ads_ports: vec![port],
                tc_version: None,
            },
            source: DiscoverySource::LocalRouter,
        };

        let candidates = discovery_candidates(vec![result(501), result(851), result(301)]);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "ads:10.20.30.40.1.1");
        assert!(candidates[0].label.contains("ADS 851, 301, 501"));
        assert_eq!(
            candidates[0]
                .params
                .get("ams_port")
                .and_then(serde_json::Value::as_u64),
            Some(851)
        );
        assert_eq!(
            candidates[0]
                .params
                .get("responding_ads_ports")
                .and_then(serde_json::Value::as_array)
                .expect("responding ports"),
            &vec![json!(851), json!(301), json!(501)]
        );
    }

    #[test]
    fn identity_only_candidate_is_visible_without_an_unconfirmed_scalar_port() {
        let candidates = discovery_candidates(vec![DiscoveryResult {
            target: ObservedAdsIdentity {
                name: Some("Controller".to_string()),
                ip: "192.168.50.42".to_string(),
                ams_net_id: "10.20.30.40.1.1".to_string(),
                preferred_ams_port: None,
                responding_ads_ports: Vec::new(),
                tc_version: None,
            },
            source: DiscoverySource::DirectedIdentify,
        }]);

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].label.contains("identity found"));
        assert!(candidates[0]
            .params
            .get("ams_port")
            .is_some_and(|value| value.is_null()));
        assert_eq!(
            candidates[0]
                .params
                .get("responding_ads_ports")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            candidates[0]
                .params
                .get("ads_service_status")
                .and_then(serde_json::Value::as_str),
            Some("identity_only")
        );
    }

    #[test]
    fn configured_result_survives_failed_lan_broadcast_with_explicit_warning() {
        let request = DiscoveryRequest {
            target: Some("127.0.0.1".to_string()),
            target_ams_net_id: Some("10.20.30.40.1.1".to_string()),
            ams_port: Some(851),
            target_name: Some("UmRT_Default".to_string()),
            include_broadcast: true,
            broadcast_targets: vec!["192.168.77.255".to_string()],
            timeout_ms: Some(50),
        };
        let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::WrongIp);
        let mut warnings = Vec::new();

        let candidates = discover_ads_request(&mut wire, &request, &mut warnings)
            .expect("configured result must survive a failed LAN broadcast");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0]
                .params
                .get("ams_net_id")
                .and_then(serde_json::Value::as_str),
            Some("10.20.30.40.1.1")
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("192.168.77.255"));
        assert!(warnings[0].contains("did not complete"));
        assert!(!warnings[0].contains("UdpIdentifyBlocked"));
    }

    #[test]
    fn interface_enumeration_failure_keeps_local_discovery_enabled() {
        let scope = DiscoverScope::default();
        let mut warnings = Vec::new();

        let request = discovery_request_with_interface_candidates(&scope, &mut warnings, || {
            Err(OnboardingError::new(
                "simulated interface enumeration failure",
            ))
        })
        .expect("local ADS discovery must survive interface enumeration failure");

        assert_eq!(request.target.as_deref(), Some("127.0.0.1"));
        assert!(request.include_broadcast);
        assert!(request.broadcast_targets.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Could not enumerate network interfaces"));
        assert!(warnings[0].contains("This computer will still be searched"));
    }
}
