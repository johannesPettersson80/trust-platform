use std::net::IpAddr;

use trust_ads_core::AdsRoute;

use super::parse_local_ams_net_id;
use crate::ads::transport::AdsTransportError;

pub(super) const DEFAULT_SOURCE_PORT: u16 = 58_913;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AdsSourcePolicy {
    RouterAssigned,
    Explicit,
    Auto,
}

pub(super) fn source_policy_for_route(route: &AdsRoute) -> AdsSourcePolicy {
    if is_loopback_host(route.host.as_str()) {
        AdsSourcePolicy::RouterAssigned
    } else if route.local_net_id.is_some() {
        AdsSourcePolicy::Explicit
    } else {
        AdsSourcePolicy::Auto
    }
}

pub(super) fn ads_source_for_route(route: &AdsRoute) -> Result<ads::Source, AdsTransportError> {
    match source_policy_for_route(route) {
        AdsSourcePolicy::RouterAssigned => Ok(ads::Source::Request),
        AdsSourcePolicy::Explicit => {
            let local_net_id = route.local_net_id.as_ref().ok_or_else(|| {
                AdsTransportError::new("explicit ADS source policy requires a local AMS Net ID")
            })?;
            Ok(ads::Source::Addr(ads::AmsAddr::new(
                parse_local_ams_net_id(local_net_id)?,
                DEFAULT_SOURCE_PORT,
            )))
        }
        AdsSourcePolicy::Auto => Ok(ads::Source::Auto),
    }
}

pub(super) fn ads_source_candidates_for_route(
    route: &AdsRoute,
) -> Result<Vec<ads::Source>, AdsTransportError> {
    let primary = ads_source_for_route(route)?;
    if source_policy_for_route(route) != AdsSourcePolicy::RouterAssigned {
        return Ok(vec![primary]);
    }
    let direct = if let Some(local_net_id) = route.local_net_id.as_ref() {
        ads::Source::Addr(ads::AmsAddr::new(
            parse_local_ams_net_id(local_net_id)?,
            DEFAULT_SOURCE_PORT,
        ))
    } else {
        ads::Source::Auto
    };
    Ok(vec![primary, direct])
}

fn is_loopback_host(host: &str) -> bool {
    let normalized = host.trim().trim_end_matches('.');
    normalized.eq_ignore_ascii_case("localhost")
        || normalized
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_ads_core::{AdsSecurityPolicy, AmsNetId, TransportSecurity};

    fn plain_route(host: &str, local_net_id: Option<&str>) -> AdsRoute {
        AdsRoute {
            name: "source-policy".to_string(),
            target_net_id: AmsNetId::new("5.23.91.12.1.1"),
            host: host.to_string(),
            ams_port: 851,
            local_net_id: local_net_id.map(AmsNetId::new),
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Plain,
                auto_add_route: false,
            },
        }
    }

    #[test]
    fn loopback_source_policy_requests_router_assigned_address() {
        for host in [
            "127.0.0.1",
            "127.0.0.2",
            "localhost",
            "LOCALHOST",
            "localhost.",
        ] {
            let route = plain_route(host, Some("127.0.0.1.1.1"));
            assert_eq!(
                source_policy_for_route(&route),
                AdsSourcePolicy::RouterAssigned,
                "loopback host {host} must ask the local AMS Router for its source identity"
            );
            assert!(matches!(
                ads_source_for_route(&route).expect("valid loopback source policy"),
                ads::Source::Request
            ));
        }
    }

    #[test]
    fn remote_source_policy_uses_explicit_configured_address() {
        assert_eq!(
            source_policy_for_route(&plain_route("192.168.10.5", Some("192.168.10.20.1.1"))),
            AdsSourcePolicy::Explicit
        );
        assert!(matches!(
            ads_source_for_route(&plain_route("192.168.10.5", Some("192.168.10.20.1.1")))
                .expect("valid explicit source"),
            ads::Source::Addr(_)
        ));
    }

    #[test]
    fn remote_source_policy_uses_automatic_address_without_configuration() {
        assert_eq!(
            source_policy_for_route(&plain_route("192.168.10.5", None)),
            AdsSourcePolicy::Auto
        );
        assert!(matches!(
            ads_source_for_route(&plain_route("192.168.10.5", None))
                .expect("valid automatic source"),
            ads::Source::Auto
        ));
    }
}
