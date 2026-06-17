use serde::{Deserialize, Serialize};

/// ADS AMS Net ID text, for example `5.23.91.12.1.1`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AmsNetId(pub String);

impl AmsNetId {
    /// Creates an AMS Net ID record without client-specific validation.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// ADS transport security selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportSecurity {
    /// Plain ADS over TCP.
    Plain,
    /// Reserved Secure ADS mode.
    Secure,
}

/// ADS route security policy fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsSecurityPolicy {
    /// Transport mode.
    pub transport: TransportSecurity,
    /// Whether tooling may add an AMS route when explicitly requested.
    #[serde(default)]
    pub auto_add_route: bool,
}

impl Default for AdsSecurityPolicy {
    fn default() -> Self {
        Self {
            transport: TransportSecurity::Secure,
            auto_add_route: false,
        }
    }
}

/// ADS endpoint route record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsRoute {
    /// Stable route name.
    pub name: String,
    /// Target AMS Net ID.
    pub target_net_id: AmsNetId,
    /// Target host name or IP address.
    pub host: String,
    /// Target AMS port, usually `851` for TwinCAT 3 PLC runtime 1.
    pub ams_port: u16,
    /// Optional local AMS Net ID override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_net_id: Option<AmsNetId>,
    /// Security policy for this route.
    #[serde(default)]
    pub security: AdsSecurityPolicy,
}

impl AdsRoute {
    /// Creates a route with secure transport reserved by default.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        target_net_id: AmsNetId,
        host: impl Into<String>,
        ams_port: u16,
    ) -> Self {
        Self {
            name: name.into(),
            target_net_id,
            host: host.into(),
            ams_port,
            local_net_id: None,
            security: AdsSecurityPolicy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_security_serializes_reserved_secure_by_default() {
        let route = AdsRoute::new(
            "line1",
            AmsNetId::new("5.23.91.12.1.1"),
            "192.168.10.5",
            851,
        );
        let json = serde_json::to_string_pretty(&route).expect("serialize route");

        assert!(json.contains(r#""transport": "secure""#));

        let round_trip: AdsRoute = serde_json::from_str(&json).expect("deserialize route");
        assert_eq!(round_trip, route);
    }

    #[test]
    fn plain_transport_round_trips_as_explicit_policy_data() {
        let policy = AdsSecurityPolicy {
            transport: TransportSecurity::Plain,
            auto_add_route: true,
        };
        let json = serde_json::to_string(&policy).expect("serialize policy");

        assert_eq!(json, r#"{"transport":"plain","auto_add_route":true}"#);
        assert_eq!(
            serde_json::from_str::<AdsSecurityPolicy>(&json).expect("deserialize policy"),
            policy
        );
    }
}
