use trust_ads_core::{
    AdsDataTypeDescriptor, ArrayDimension, IecDataType, SymbolDescriptor, SymbolFlag,
};

use super::*;

#[test]
fn apply_import_rejects_malformed_existing_toml() {
    let error = apply_symbol_import(
        Some("[[connections]\nname ="),
        SymbolImportApplyRequest {
            response: import_response("line1", "line1_", "MAIN.Temperature"),
            target: target("5.23.91.12.1.1", "192.168.10.5"),
            local: local("192.168.10.20.1.1"),
            existing_snapshots: Vec::new(),
            write_acknowledged: false,
        },
    )
    .expect_err("malformed existing config must fail before merge");

    assert!(matches!(
        error,
        SymbolImportApplyError::InvalidExistingToml(_)
    ));
}

#[test]
fn apply_import_projects_string_array_and_notify_mode() {
    let name = SymbolDescriptor::new(
        "GVL.Name",
        AdsDataTypeDescriptor::string("STRING(32)", 32),
        0x4020,
        0,
        33,
    )
    .with_flag(SymbolFlag::Read);
    let samples_type =
        AdsDataTypeDescriptor::scalar("DINT", IecDataType::Dint).with_dimensions(vec![
            ArrayDimension {
                lower: -2,
                upper: 2,
            },
        ]);
    let samples = SymbolDescriptor::new("GVL.Samples", samples_type, 0x4020, 40, 20)
        .with_flag(SymbolFlag::Read);
    let mut response = build_symbol_import_response(
        &SymbolImportRequest {
            connection_name: "line1".to_string(),
            symbols: Vec::new(),
            include_patterns: Vec::new(),
            name_prefix: Some("line1_".to_string()),
        },
        vec![name, samples],
    );
    response
        .candidates
        .iter_mut()
        .find(|candidate| candidate.descriptor.name == "GVL.Samples")
        .expect("samples candidate")
        .mode = UpdateMode::Notify;

    let artifacts = apply_symbol_import(
        None,
        SymbolImportApplyRequest {
            response,
            target: target("5.23.91.12.1.1", "192.168.10.5"),
            local: local("192.168.10.20.1.1"),
            existing_snapshots: Vec::new(),
            write_acknowledged: false,
        },
    )
    .expect("string and array projection");
    let config = parse_ads_toml(&artifacts.ads_toml).expect("parse generated ads.toml");
    let name = config.connections[0]
        .points
        .iter()
        .find(|point| point.point_name == "line1_gvl_name")
        .expect("name point");
    assert_eq!(name.data_type.iec_type, IecDataType::String);
    assert_eq!(name.data_type.string_len, Some(32));
    let samples = config.connections[0]
        .points
        .iter()
        .find(|point| point.point_name == "line1_gvl_samples")
        .expect("samples point");
    assert_eq!(samples.data_type.iec_type, IecDataType::Dint);
    assert_eq!(samples.data_type.dimensions.len(), 1);
    assert_eq!(samples.data_type.dimensions[0].lower, -2);
    assert_eq!(samples.data_type.dimensions[0].upper, 2);
    assert_eq!(samples.mode, UpdateMode::Notify);
}

#[test]
fn iec_type_name_covers_all_supported_scalar_types() {
    let cases = [
        (IecDataType::Bool, "BOOL"),
        (IecDataType::Sint, "SINT"),
        (IecDataType::Int, "INT"),
        (IecDataType::Dint, "DINT"),
        (IecDataType::Lint, "LINT"),
        (IecDataType::Usint, "USINT"),
        (IecDataType::Uint, "UINT"),
        (IecDataType::Udint, "UDINT"),
        (IecDataType::Ulint, "ULINT"),
        (IecDataType::Real, "REAL"),
        (IecDataType::Lreal, "LREAL"),
        (IecDataType::Byte, "BYTE"),
        (IecDataType::Word, "WORD"),
        (IecDataType::Dword, "DWORD"),
        (IecDataType::Lword, "LWORD"),
        (IecDataType::String, "STRING"),
    ];

    for (iec_type, expected) in cases {
        let descriptor = AdsDataTypeDescriptor::scalar(expected, iec_type);
        assert_eq!(iec_type_name(&descriptor), expected);
    }
}

fn import_response(connection: &str, prefix: &str, symbol: &str) -> SymbolImportResponse {
    build_symbol_import_response(
        &SymbolImportRequest {
            connection_name: connection.to_string(),
            symbols: Vec::new(),
            include_patterns: Vec::new(),
            name_prefix: Some(prefix.to_string()),
        },
        vec![real_symbol(symbol)],
    )
}

fn real_symbol(name: &str) -> SymbolDescriptor {
    SymbolDescriptor::new(
        name,
        AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        0x4020,
        0,
        4,
    )
    .with_flag(SymbolFlag::Read)
}

fn target(net_id: &str, ip: &str) -> TargetIdentity {
    TargetIdentity {
        name: Some("CX".to_string()),
        ip: ip.to_string(),
        ams_net_id: net_id.to_string(),
        ams_port: 851,
        tc_version: Some("3.1.4024".to_string()),
    }
}

fn local(net_id: &str) -> LocalIdentity {
    LocalIdentity {
        host_name: Some("line-controller".to_string()),
        chosen_ip: "192.168.10.20".to_string(),
        ams_net_id: net_id.to_string(),
        nic: Some("eth0".to_string()),
        candidates: Vec::new(),
        classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
    }
}
