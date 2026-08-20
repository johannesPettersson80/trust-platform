use std::cell::Cell;

use super::*;
use trust_ads_core::{AdsSecurityPolicy, AmsNetId};

#[test]
fn persistent_ads_symbol_sets_retain_guardrail_flag() {
    let symbol = ads::symbol::Symbol {
        name: "GVL.RetainedSetpoint".to_string(),
        ix_group: 0x4020,
        ix_offset: 0,
        typ: "DINT".to_string(),
        size: 4,
        base_type: ADS_BASE_TYPE_DINT,
        flags: ADS_SYMBOL_FLAG_PERSISTENT,
    };
    let descriptor = symbol_descriptor_from_ads(&symbol, &ads::symbol::TypeMap::default())
        .expect("symbol descriptor")
        .expect("supported scalar symbol");

    assert!(descriptor.flags.contains(&SymbolFlag::Persistent));
    assert!(descriptor.flags.contains(&SymbolFlag::Retain));
    assert!(descriptor.flags.contains(&SymbolFlag::Write));
}

#[test]
fn readonly_ads_symbol_does_not_get_write_flag() {
    let symbol = ads::symbol::Symbol {
        name: "GVL.ReadOnlyStatus".to_string(),
        ix_group: 0x4020,
        ix_offset: 0,
        typ: "DINT".to_string(),
        size: 4,
        base_type: ADS_BASE_TYPE_DINT,
        flags: ADS_SYMBOL_FLAG_READ_ONLY,
    };
    let descriptor = symbol_descriptor_from_ads(&symbol, &ads::symbol::TypeMap::default())
        .expect("symbol descriptor")
        .expect("supported scalar symbol");

    assert!(descriptor.flags.contains(&SymbolFlag::Read));
    assert!(!descriptor.flags.contains(&SymbolFlag::Write));
}

#[test]
fn unsupported_compound_symbol_is_not_a_bindable_descriptor() {
    let symbol = ads::symbol::Symbol {
        name: "GVL.LibraryVersion".to_string(),
        ix_group: 0x4020,
        ix_offset: 0,
        typ: "ST_LibVersion".to_string(),
        size: 16,
        base_type: ADS_BASE_TYPE_COMPOUND,
        flags: 0,
    };

    let descriptor = symbol_descriptor_from_ads(&symbol, &ads::symbol::TypeMap::default())
        .expect("unsupported complex symbols are skipped, not fatal");

    assert!(descriptor.is_none());
}

#[test]
fn subscribe_rejects_poll_mode_before_connection() {
    let mut transport = AdsRsTransport::new(AdsRoute {
        name: "line1".to_string(),
        target_net_id: AmsNetId::new("5.23.91.12.1.1"),
        host: "192.168.10.5".to_string(),
        ams_port: 851,
        local_net_id: None,
        security: AdsSecurityPolicy {
            transport: TransportSecurity::Plain,
            auto_add_route: false,
        },
    });
    let request = AdsSubscribeRequest {
        handle: AdsResolvedHandle {
            point_name: "line1_temp".to_string(),
            address: AdsPointAddress::Index {
                index_group: 0x4020,
                index_offset: 0,
                size: 4,
            },
            data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            handle: 0,
        },
        mode: UpdateMode::Poll,
        notification_mode: AdsNotificationMode::OnChange,
    };

    let error = transport
        .subscribe(request)
        .expect_err("poll subscriptions must be rejected locally");

    assert!(error.message().contains("poll points use sumup_read"));
}

#[test]
fn connect_rejects_secure_transport_before_network() {
    let mut transport = AdsRsTransport::new(AdsRoute {
        name: "line1".to_string(),
        target_net_id: AmsNetId::new("5.23.91.12.1.1"),
        host: "192.0.2.5".to_string(),
        ams_port: 851,
        local_net_id: None,
        security: AdsSecurityPolicy {
            transport: TransportSecurity::Secure,
            auto_add_route: false,
        },
    });

    let error = transport
        .connect()
        .expect_err("secure transport is reserved");

    assert!(error.message().contains("Secure ADS is reserved"));
}

#[test]
fn connect_rejects_auto_add_route_before_network() {
    let mut transport = AdsRsTransport::new(AdsRoute {
        name: "line1".to_string(),
        target_net_id: AmsNetId::new("5.23.91.12.1.1"),
        host: "192.0.2.5".to_string(),
        ams_port: 851,
        local_net_id: None,
        security: AdsSecurityPolicy {
            transport: TransportSecurity::Plain,
            auto_add_route: true,
        },
    });

    let error = transport
        .connect()
        .expect_err("runtime must not write AMS routes");

    assert!(error.message().contains("auto_add_route=true"));
}

fn assert_invalid_authority_preserves_state(
    case: &str,
    target_net_id: &str,
    ams_port: u16,
    local_net_id: Option<&str>,
    expected_error: &str,
) {
    let mut transport = AdsRsTransport::new(AdsRoute {
        name: case.to_string(),
        target_net_id: AmsNetId::new(target_net_id),
        host: "unused.invalid".to_string(),
        ams_port,
        local_net_id: local_net_id.map(AmsNetId::new),
        security: AdsSecurityPolicy {
            transport: TransportSecurity::Plain,
            auto_add_route: false,
        },
    });
    transport
        .symbol_handles_by_point
        .insert("existing_symbol".to_string(), 41);
    transport.subscriptions_by_handle.insert(
        73,
        AdsResolvedHandle {
            point_name: "existing_subscription".to_string(),
            address: AdsPointAddress::Index {
                index_group: 0x4020,
                index_offset: 4,
                size: 4,
            },
            data_type: AdsDataTypeDescriptor::scalar("DINT", IecDataType::Dint),
            handle: 0,
        },
    );
    let expected_symbol_handles = transport.symbol_handles_by_point.clone();
    let expected_subscriptions = transport.subscriptions_by_handle.clone();
    let connector_invoked = Cell::new(false);

    let error = transport
        .connect_with(|_, _, _| {
            connector_invoked.set(true);
            Err(AdsTransportError::new("test connector was invoked"))
        })
        .expect_err("invalid ADS authority must fail before the connector");

    assert_eq!(error.message(), expected_error, "{case}");
    assert!(!connector_invoked.get(), "{case} invoked the ADS connector");
    assert_eq!(
        transport.symbol_handles_by_point, expected_symbol_handles,
        "{case} cleared existing local symbol-handle state"
    );
    assert_eq!(
        transport.subscriptions_by_handle, expected_subscriptions,
        "{case} cleared existing local subscription state"
    );
}

#[test]
fn connect_rejects_invalid_target_or_zero_port_before_connector_or_state_cleanup() {
    let cases = [
        (
            "short_target_net_id",
            "5.23",
            851,
            "invalid target AMS Net ID '5.23'; expected six decimal octets in canonical form",
        ),
        (
            "blank_target_net_id",
            "",
            851,
            "invalid target AMS Net ID ''; expected six decimal octets in canonical form",
        ),
        (
            "nondigit_target_net_id",
            "1.2.3.4.5.x",
            851,
            "invalid target AMS Net ID '1.2.3.4.5.x'; expected six decimal octets in canonical form",
        ),
        (
            "extra_target_net_id_octet",
            "1.2.3.4.5.6.7",
            851,
            "invalid target AMS Net ID '1.2.3.4.5.6.7'; expected six decimal octets in canonical form",
        ),
        (
            "out_of_range_target_net_id_octet",
            "256.2.3.4.5.6",
            851,
            "invalid target AMS Net ID '256.2.3.4.5.6'; expected six decimal octets in canonical form",
        ),
        (
            "noncanonical_target_net_id",
            "01.2.3.4.5.6",
            851,
            "invalid target AMS Net ID '01.2.3.4.5.6'; expected six decimal octets in canonical form",
        ),
        (
            "zero_target_ams_port",
            "5.23.91.12.1.1",
            0,
            "ADS target AMS port must be non-zero",
        ),
    ];

    for (case, target_net_id, ams_port, expected_error) in cases {
        assert_invalid_authority_preserves_state(
            case,
            target_net_id,
            ams_port,
            None,
            expected_error,
        );
    }
}

#[test]
fn connect_rejects_invalid_local_identity_before_connector_or_state_cleanup() {
    let cases = [
        (
            "short_local_net_id",
            "5.23",
            "invalid local AMS Net ID '5.23'; expected six decimal octets in canonical form",
        ),
        (
            "blank_local_net_id",
            "",
            "invalid local AMS Net ID ''; expected six decimal octets in canonical form",
        ),
        (
            "nondigit_local_net_id",
            "1.2.3.4.5.x",
            "invalid local AMS Net ID '1.2.3.4.5.x'; expected six decimal octets in canonical form",
        ),
        (
            "extra_local_net_id_octet",
            "1.2.3.4.5.6.7",
            "invalid local AMS Net ID '1.2.3.4.5.6.7'; expected six decimal octets in canonical form",
        ),
        (
            "out_of_range_local_net_id_octet",
            "256.2.3.4.5.6",
            "invalid local AMS Net ID '256.2.3.4.5.6'; expected six decimal octets in canonical form",
        ),
        (
            "noncanonical_local_net_id",
            "01.2.3.4.5.6",
            "invalid local AMS Net ID '01.2.3.4.5.6'; expected six decimal octets in canonical form",
        ),
    ];

    for (case, local_net_id, expected_error) in cases {
        assert_invalid_authority_preserves_state(
            case,
            "5.23.91.12.1.1",
            851,
            Some(local_net_id),
            expected_error,
        );
    }
}

#[test]
fn connect_accepts_canonical_authority_and_reaches_connector() {
    let cases = [
        ("automatic_source", None, None),
        (
            "explicit_source",
            Some("192.168.10.20.1.1"),
            Some([192, 168, 10, 20, 1, 1]),
        ),
    ];

    for (case, local_net_id, expected_explicit_net_id) in cases {
        let mut transport = AdsRsTransport::new(AdsRoute {
            name: case.to_string(),
            target_net_id: AmsNetId::new("5.23.91.12.1.1"),
            host: "unused.invalid".to_string(),
            ams_port: 851,
            local_net_id: local_net_id.map(AmsNetId::new),
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Plain,
                auto_add_route: false,
            },
        });
        let connector_invoked = Cell::new(false);
        let observed_source = Cell::new(None);

        let error = transport
            .connect_with(|_, _, source| {
                connector_invoked.set(true);
                observed_source.set(Some(source));
                Err(AdsTransportError::new("canonical authority sentinel"))
            })
            .expect_err("the hermetic connector returns its sentinel error");

        assert_eq!(error.message(), "canonical authority sentinel", "{case}");
        assert!(
            connector_invoked.get(),
            "{case} did not reach the connector"
        );
        match (observed_source.get(), expected_explicit_net_id) {
            (Some(ads::Source::Auto), None) => {}
            (Some(ads::Source::Addr(address)), Some(expected_net_id)) => {
                assert_eq!(
                    address.netid(),
                    ads::AmsNetId::from(expected_net_id),
                    "{case}"
                );
                assert_eq!(address.port(), DEFAULT_SOURCE_PORT, "{case}");
            }
            (observed, expected) => {
                panic!("{case} recorded unexpected source {observed:?} for {expected:?}")
            }
        }
    }
}
