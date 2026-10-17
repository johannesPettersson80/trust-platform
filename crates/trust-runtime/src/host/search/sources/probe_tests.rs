use std::net::TcpListener;
use std::time::Duration;

use super::super::modbus_tcp::{probe_modbus_tcp, ModbusDiscoveryProbe};
use super::super::mqtt::probe_mqtt;
use super::*;

#[test]
fn outcome_constructors_use_closed_confidence_and_default_flags() {
    let confirmed = DiscoveryProbeOutcome::confirmed("wire", "confirmed");
    assert_eq!(confirmed.confidence, CONFIRMED);
    assert_eq!(confirmed.source, "wire");
    assert_eq!(confirmed.detail, "confirmed");
    assert!(confirmed.warnings.is_empty());
    assert!(!confirmed.auth_required);

    let likely = DiscoveryProbeOutcome::likely("wire", "likely");
    assert_eq!(likely.confidence, LIKELY);
    assert!(!likely.auth_required);

    let reachable = DiscoveryProbeOutcome::port_reachable("tcp", "reachable");
    assert_eq!(reachable.confidence, PORT_REACHABLE);
    assert!(!reachable.auth_required);
}

#[test]
fn outcome_modifiers_preserve_warning_order_and_auth_flag() {
    let outcome = DiscoveryProbeOutcome::likely("mqtt", "rejected")
        .with_warning("first")
        .with_warning("second")
        .with_auth_required();
    assert_eq!(outcome.warnings, vec!["first", "second"]);
    assert!(outcome.auth_required);
    assert_eq!(outcome.confidence, LIKELY);
}

#[test]
fn unreachable_modbus_and_mqtt_targets_return_no_observation() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused socket");
    let address = listener.local_addr().expect("unused address");
    drop(listener);
    assert!(probe_modbus_tcp(
        address,
        Duration::from_millis(25),
        ModbusDiscoveryProbe {
            unit_id: 1,
            safe_read: None
        }
    )
    .is_none());
    assert!(probe_mqtt(address, Duration::from_millis(25)).is_none());
}
