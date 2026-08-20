//! Runtime-cloud dispatch/preflight helpers for web routes.

#![allow(missing_docs)]

use super::*;

pub(super) struct RuntimeCloudPreflightPolicy<'a> {
    pub role: AccessRole,
    pub local_supports_secure_transport: bool,
    pub profile: RuntimeCloudProfile,
    pub wan_allow_write: &'a [RuntimeCloudWanAllowRule],
    pub auth_mode: WebAuthMode,
}

pub(super) fn runtime_cloud_preflight_for_action(
    action: &RuntimeCloudActionRequest,
    local_runtime_id: &str,
    discovery: &DiscoveryState,
    policy: RuntimeCloudPreflightPolicy<'_>,
    ha_state: &Mutex<RuntimeCloudHaCoordinator>,
) -> (
    RuntimeCloudActionPreflight,
    Option<RuntimeCloudHaRequest>,
    BTreeMap<String, RuntimeCloudTargetStatus>,
) {
    let known_targets = runtime_cloud_target_status_map(
        discovery,
        local_runtime_id,
        policy.local_supports_secure_transport,
        now_ns(),
    );
    let preflight = preflight_action(
        action,
        RuntimeCloudPreflightContext {
            local_runtime_id,
            role: policy.role,
        },
        &known_targets,
    );
    let preflight = runtime_cloud_apply_profile_policy(
        preflight,
        action,
        &known_targets,
        policy.profile,
        policy.wan_allow_write,
        policy.auth_mode,
        policy.local_supports_secure_transport,
    );
    let (preflight, ha_request) = runtime_cloud_apply_ha_policy(preflight, action, ha_state);
    (preflight, ha_request, known_targets)
}

pub(super) fn runtime_cloud_target_control_url(
    discovery: &DiscoveryState,
    target: &str,
    require_secure_transport: bool,
) -> Option<String> {
    runtime_cloud_target_web_base_url(discovery, target, require_secure_transport)
        .map(|base| format!("{base}/api/control"))
}

pub(super) fn runtime_cloud_target_web_base_url(
    discovery: &DiscoveryState,
    target: &str,
    require_secure_transport: bool,
) -> Option<String> {
    let scheme = if require_secure_transport {
        "https"
    } else {
        "http"
    };
    discovery
        .snapshot()
        .into_iter()
        .find(|entry| entry.name.as_str() == target || entry.id.as_str() == target)
        .and_then(|entry| {
            let port = entry.web_port?;
            let address = select_preferred_peer_address(&entry.addresses)?;
            let host = runtime_cloud_url_host(address);
            Some(format!("{scheme}://{host}:{port}"))
        })
}

fn select_preferred_peer_address(addresses: &[std::net::IpAddr]) -> Option<std::net::IpAddr> {
    addresses
        .iter()
        .copied()
        .find(|addr| addr.is_ipv4() && addr.is_loopback())
        .or_else(|| {
            addresses.iter().copied().find(|addr| {
                matches!(
                    addr,
                    std::net::IpAddr::V4(v4) if !v4.is_unspecified() && !v4.is_multicast()
                )
            })
        })
        .or_else(|| {
            addresses
                .iter()
                .copied()
                .find(|addr| addr.is_ipv6() && addr.is_loopback())
        })
        .or_else(|| {
            addresses.iter().copied().find(|addr| {
                matches!(
                    addr,
                    std::net::IpAddr::V6(v6)
                        if !v6.is_unspecified() && !v6.is_multicast() && !v6.is_unicast_link_local()
                )
            })
        })
        .or_else(|| {
            addresses
                .iter()
                .copied()
                .find(|addr| !addr.is_unspecified())
        })
}

pub(super) fn runtime_cloud_peer_appears_live(
    addresses: &[std::net::IpAddr],
    web_port: Option<u16>,
) -> bool {
    let Some(port) = web_port else {
        return false;
    };
    let Some(address) = select_preferred_peer_address(addresses) else {
        return false;
    };
    let socket = std::net::SocketAddr::new(address, port);
    std::net::TcpStream::connect_timeout(&socket, std::time::Duration::from_millis(200)).is_ok()
}

fn runtime_cloud_url_host(address: std::net::IpAddr) -> String {
    match address {
        std::net::IpAddr::V4(v4) => v4.to_string(),
        std::net::IpAddr::V6(v6) => format!("[{v6}]"),
    }
}

pub(super) fn runtime_cloud_denied_results(
    preflight: &RuntimeCloudActionPreflight,
) -> Vec<RuntimeCloudDispatchTargetResult> {
    preflight
        .decisions
        .iter()
        .map(|decision| RuntimeCloudDispatchTargetResult {
            runtime_id: decision.runtime_id.clone(),
            ok: decision.allowed,
            denial_code: decision.denial_code,
            denial_reason: decision.denial_reason.clone(),
            audit_id: None,
            response: None,
        })
        .collect()
}

fn runtime_cloud_is_stale(last_seen_ns: u64, now_ns: u64, force_stale: bool) -> bool {
    if force_stale {
        return true;
    }
    if last_seen_ns > now_ns {
        return true;
    }
    now_ns.saturating_sub(last_seen_ns) >= RUNTIME_CLOUD_STALE_TIMEOUT_NS
}

fn runtime_cloud_target_status_map(
    discovery: &DiscoveryState,
    local_runtime_id: &str,
    local_supports_secure_transport: bool,
    now_ns: u64,
) -> BTreeMap<String, RuntimeCloudTargetStatus> {
    let mut targets = BTreeMap::new();
    targets.insert(
        local_runtime_id.to_string(),
        RuntimeCloudTargetStatus {
            reachable: true,
            stale: false,
            supports_secure_transport: local_supports_secure_transport,
        },
    );
    for entry in discovery.snapshot() {
        let web_reachable = entry.web_port.is_some() && !entry.addresses.is_empty();
        let mesh_reachable = entry.mesh_port.is_some() && !entry.addresses.is_empty();
        let mut stale = runtime_cloud_is_stale(entry.last_seen_ns, now_ns, !mesh_reachable);
        let mut reachable = web_reachable;
        if stale && web_reachable {
            if runtime_cloud_peer_appears_live(&entry.addresses, entry.web_port) {
                stale = false;
                reachable = true;
            } else {
                reachable = false;
            }
        }
        let candidate = RuntimeCloudTargetStatus {
            reachable,
            stale,
            supports_secure_transport: entry.web_tls,
        };
        targets
            .entry(entry.name.to_string())
            .and_modify(|status| runtime_cloud_merge_target_status(status, candidate))
            .or_insert(candidate);
        targets
            .entry(entry.id.to_string())
            .and_modify(|status| runtime_cloud_merge_target_status(status, candidate))
            .or_insert(candidate);
    }
    targets
}

fn runtime_cloud_merge_target_status(
    existing: &mut RuntimeCloudTargetStatus,
    candidate: RuntimeCloudTargetStatus,
) {
    existing.reachable |= candidate.reachable;
    existing.stale &= candidate.stale;
    existing.supports_secure_transport |= candidate.supports_secure_transport;
}

fn runtime_cloud_apply_ha_policy(
    preflight: RuntimeCloudActionPreflight,
    action: &RuntimeCloudActionRequest,
    ha_state: &Mutex<RuntimeCloudHaCoordinator>,
) -> (RuntimeCloudActionPreflight, Option<RuntimeCloudHaRequest>) {
    let ha_request = match parse_action_ha_request(&action.payload) {
        Ok(request) => request,
        Err(error) => {
            let denied =
                runtime_cloud_apply_global_denial(preflight, ReasonCode::ContractViolation, error);
            return (denied, None);
        }
    };
    let Some(ha_request_ref) = ha_request.as_ref() else {
        return (preflight, None);
    };

    let mut preflight = preflight;
    let split_brain = ha_request_ref.split_brain_runtimes(action.action_type.as_str());
    let coordinator = match ha_state.lock() {
        Ok(guard) => guard,
        Err(_) => {
            let denied = runtime_cloud_apply_global_denial(
                preflight,
                ReasonCode::TransportFailure,
                "runtime cloud HA state is unavailable".to_string(),
            );
            return (denied, None);
        }
    };

    for decision in &mut preflight.decisions {
        if !decision.allowed {
            continue;
        }
        if split_brain.contains(decision.runtime_id.as_str()) {
            decision.allowed = false;
            decision.denial_code = Some(ReasonCode::LeaseUnavailable);
            decision.denial_reason = Some(format!(
                "HA split-brain risk: multiple ACTIVE candidates for group '{}' ; demoted_safe required",
                ha_request_ref.group_id
            ));
            continue;
        }
        let Some(ha_decision) = coordinator.preflight_decision(
            action.action_type.as_str(),
            action.request_id.as_str(),
            decision.runtime_id.as_str(),
            ha_request_ref,
        ) else {
            continue;
        };
        if ha_decision.allowed {
            continue;
        }
        decision.allowed = false;
        decision.denial_code = ha_decision.denial_code;
        decision.denial_reason = ha_decision.denial_reason;
    }
    runtime_cloud_update_preflight_summary(&mut preflight);
    (preflight, ha_request)
}

fn runtime_cloud_apply_global_denial(
    mut preflight: RuntimeCloudActionPreflight,
    denial_code: ReasonCode,
    denial_reason: String,
) -> RuntimeCloudActionPreflight {
    for decision in &mut preflight.decisions {
        decision.allowed = false;
        decision.denial_code = Some(denial_code);
        decision.denial_reason = Some(denial_reason.clone());
    }
    preflight.allowed = false;
    preflight.denial_code = Some(denial_code);
    preflight.denial_reason = Some(denial_reason);
    preflight
}

fn runtime_cloud_update_preflight_summary(preflight: &mut RuntimeCloudActionPreflight) {
    preflight.allowed = preflight.decisions.iter().all(|decision| decision.allowed);
    let (denial_code, denial_reason) = preflight
        .decisions
        .iter()
        .find(|decision| !decision.allowed)
        .map(|decision| (decision.denial_code, decision.denial_reason.clone()))
        .unwrap_or((None, None));
    preflight.denial_code = denial_code;
    preflight.denial_reason = denial_reason;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn runtime_cloud_url_host_wraps_ipv6() {
        let host = runtime_cloud_url_host(IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert_eq!(host, "[::1]");
    }

    #[test]
    fn runtime_cloud_select_preferred_peer_address_prefers_ipv4_loopback() {
        let addresses = vec![
            IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1)),
        ];
        let selected = select_preferred_peer_address(&addresses).expect("selected");
        assert_eq!(selected, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    }

    #[test]
    fn runtime_cloud_select_preferred_peer_address_prefers_non_link_local_ipv6() {
        let addresses = vec![
            IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4)),
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 1, 2, 3, 4)),
        ];
        let selected = select_preferred_peer_address(&addresses).expect("selected");
        assert_eq!(
            selected,
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 1, 2, 3, 4))
        );
    }

    #[test]
    fn runtime_cloud_target_status_revalidates_stale_peer_with_live_socket() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("listener addr").port();
        let discovery = DiscoveryState::new();
        discovery.replace_entries(vec![crate::discovery::DiscoveryEntry {
            id: "runtime-b-1".into(),
            name: "runtime-b".into(),
            addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            web_port: Some(port),
            web_tls: false,
            mesh_port: Some(5202),
            control: Some("unix:///tmp/trust-runtime-b.sock".into()),
            host_group: None,
            last_seen_ns: 1,
        }]);
        let now = RUNTIME_CLOUD_STALE_TIMEOUT_NS + 2;
        let targets = runtime_cloud_target_status_map(&discovery, "runtime-a", false, now);
        let status = targets.get("runtime-b").expect("runtime-b status");
        assert!(status.reachable);
        assert!(!status.stale);
    }
}
