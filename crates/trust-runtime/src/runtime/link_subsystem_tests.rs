//! Specification 42, §3: every state of an ADS and an OPC UA client connection as a link's state.

use smol_str::SmolStr;

use super::{ads_reading, opcua_reading, LinkState};
use crate::ads::diagnostics::{AdsConnectionStatus, AdsConnectionStatusState};
use crate::opcua::{OpcUaClientConnectionState, OpcUaClientConnectionStatus};

fn ads(state: AdsConnectionStatusState, degraded_points: usize) -> AdsConnectionStatus {
    AdsConnectionStatus {
        name: "line".to_string(),
        target: None,
        state,
        point_count: 4,
        degraded_points,
        last_good_value_ms: None,
        symbol_version: None,
        summary: "route refused".to_string(),
    }
}

fn opcua(state: OpcUaClientConnectionState, degraded_points: usize) -> OpcUaClientConnectionStatus {
    OpcUaClientConnectionStatus {
        name: SmolStr::new("plant"),
        endpoint_url: "opc.tcp://127.0.0.1:4840".to_string(),
        state,
        point_count: 4,
        degraded_points,
        last_seen_ms: None,
        detail: "session closed".to_string(),
        points: Vec::new(),
    }
}

#[test]
fn every_ads_connection_state_maps_to_a_link_state() {
    use AdsConnectionStatusState as Ads;
    let cases = [
        (ads(Ads::Connected, 0), LinkState::Connected),
        (ads(Ads::Connected, 1), LinkState::Degraded),
        (ads(Ads::Stale, 0), LinkState::Degraded),
        (ads(Ads::NotReady, 0), LinkState::Connecting),
        (ads(Ads::Reconnecting, 0), LinkState::Failed),
        (ads(Ads::Faulted, 0), LinkState::Failed),
        (ads(Ads::Disabled, 0), LinkState::Disabled),
        (ads(Ads::Unknown, 0), LinkState::Unknown),
    ];
    for (connection, state) in cases {
        let reading = ads_reading(&connection);
        assert_eq!(
            (reading.name.as_str(), reading.state),
            ("ads:line", state),
            "{:?}",
            connection.state
        );
    }
    assert_eq!(
        ads_reading(&ads(Ads::Faulted, 0)).error.as_deref(),
        Some("route refused")
    );
    assert_eq!(
        ads_reading(&ads(Ads::Connected, 0)).error,
        None,
        "a healthy link reports no error"
    );
}

#[test]
fn every_opcua_client_connection_state_maps_to_a_link_state() {
    use OpcUaClientConnectionState as Ua;
    let cases = [
        (opcua(Ua::Connected, 0), LinkState::Connected),
        (opcua(Ua::Connected, 2), LinkState::Degraded),
        (opcua(Ua::Configured, 0), LinkState::Connecting),
        (opcua(Ua::Connecting, 0), LinkState::Connecting),
        (opcua(Ua::Stale, 0), LinkState::Degraded),
        (opcua(Ua::Reconnecting, 0), LinkState::Failed),
        (opcua(Ua::Faulted, 0), LinkState::Failed),
        (opcua(Ua::Disabled, 0), LinkState::Disabled),
    ];
    for (connection, state) in cases {
        let reading = opcua_reading(&connection);
        assert_eq!(
            (reading.name.as_str(), reading.state),
            ("opcua_client:plant", state),
            "{:?}",
            connection.state
        );
    }
    assert_eq!(
        opcua_reading(&opcua(Ua::Faulted, 0)).error.as_deref(),
        Some("session closed")
    );
}
