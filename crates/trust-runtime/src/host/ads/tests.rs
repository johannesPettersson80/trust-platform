use std::time::{Duration, Instant};

use crate::harness::{CompileSession, SourceFile};
use trust_ads_core::{
    AdsDataTypeDescriptor, AdsRoute, AdsSecurityPolicy, AmsNetId, ArrayDimension, IecDataType,
    PointAccess, PointQuality, QualityState, SymbolDescriptor, SymbolFlag, SymbolSnapshot,
    TransportSecurity, UpdateMode,
};
use trust_hir::TypeId;
use trust_runtime_core::value::Value;

use crate::{RetainPolicy, Runtime};

use super::*;

const VALID_ADS_TOML: &str = r#"
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"
mode = "notify"
notification_mode = "cyclic"

[[connections.points]]
symbol = "GVL.Setpoint"
var = "line1_setpoint"
type = "REAL"
access = "write"

[[connections.points]]
index_group = 16416
index_offset = 0
size = 2
var = "line1_status"
type = "WORD"
"#;

#[test]
fn parses_ads_toml_and_applies_security_and_point_defaults() {
    let config = parse_ads_toml(VALID_ADS_TOML).expect("valid ADS config");

    let connection = &config.connections[0];
    assert_eq!(connection.route.name, "line1");
    assert_eq!(
        connection.route.security.transport,
        TransportSecurity::Plain
    );
    assert_eq!(connection.points.len(), 3);
    assert_eq!(connection.points[0].point_name, "line1_temp");
    assert_eq!(connection.points[0].access, PointAccess::Read);
    assert_eq!(connection.points[0].mode, UpdateMode::Notify);
    assert_eq!(
        connection.points[0].notification_mode,
        AdsNotificationMode::Cyclic
    );
    assert_eq!(connection.points[1].access, PointAccess::Write);
    assert!(matches!(
        connection.points[2].address,
        AdsPointAddress::Index {
            index_group: 16416,
            index_offset: 0,
            size: 2,
        }
    ));
}

#[test]
fn rejects_plain_transport_without_explicit_ack() {
    let text = VALID_ADS_TOML.replace("insecure_transport = true\n", "");

    let error = parse_ads_toml(&text).expect_err("plain transport must be acknowledged");

    assert!(error
        .to_string()
        .contains("transport='plain' but insecure_transport=true is missing"));
}

#[test]
fn plain_transport_ack_surfaces_security_warning() {
    let config = parse_ads_toml(VALID_ADS_TOML).expect("valid ADS config");

    let warnings = config.connections[0].security_warnings();

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("plain ADS transport"));
    assert!(warnings[0].contains("cleartext"));
    assert!(warnings[0].contains("AMS route trust"));
}

#[test]
fn production_local_identity_validation_requires_pinned_local_net_id() {
    let config = parse_ads_toml(VALID_ADS_TOML).expect("valid ADS config");
    let local = local_identity("192.168.10.20.1.1");

    let error = validate_ads_config_local_identity(&config, &local)
        .expect_err("missing local_net_id is not production-ready");

    assert!(error.to_string().contains("missing local_net_id"));
}

#[test]
fn production_local_identity_validation_rejects_mismatched_local_net_id() {
    let text = VALID_ADS_TOML.replace(
        "insecure_transport = true\n",
        "insecure_transport = true\nlocal_net_id = \"192.168.10.99.1.1\"\n",
    );
    let config = parse_ads_toml(&text).expect("valid ADS config");
    let local = local_identity("192.168.10.20.1.1");

    let error = validate_ads_config_local_identity(&config, &local)
        .expect_err("mismatched local_net_id is not production-ready");

    assert!(error.to_string().contains("does not match"));
}

#[test]
fn production_local_identity_validation_accepts_matching_local_net_id() {
    let text = VALID_ADS_TOML.replace(
        "insecure_transport = true\n",
        "insecure_transport = true\nlocal_net_id = \"192.168.10.20.1.1\"\n",
    );
    let config = parse_ads_toml(&text).expect("valid ADS config");
    let local = local_identity("192.168.10.20.1.1");

    validate_ads_config_local_identity(&config, &local).expect("matching local_net_id");
}

#[test]
fn rejects_insecure_ack_without_plain_transport() {
    let text = VALID_ADS_TOML.replace("transport = \"plain\"", "transport = \"secure\"");

    let error = parse_ads_toml(&text).expect_err("insecure ack must match plain transport");

    assert!(error
        .to_string()
        .contains("insecure_transport=true without transport='plain'"));
}

#[test]
fn rejects_duplicate_declared_variable_binding() {
    let text = VALID_ADS_TOML.replace("var = \"line1_setpoint\"", "var = \"line1_temp\"");

    let error = parse_ads_toml(&text).expect_err("duplicate point target must fail");

    assert!(error
        .to_string()
        .contains("one binding per declared variable"));
}

#[test]
fn rejects_notification_mode_without_notify_mode() {
    let text = VALID_ADS_TOML.replace("mode = \"notify\"", "mode = \"poll\"");

    let error = parse_ads_toml(&text).expect_err("notification_mode requires notify mode");

    assert!(error.to_string().contains("notification_mode"));
    assert!(error.to_string().contains("mode is not 'notify'"));
}

#[test]
fn rejects_mixed_symbol_and_index_addressing() {
    let text = VALID_ADS_TOML.replace(
        "symbol = \"MAIN.Temperature\"",
        "symbol = \"MAIN.Temperature\"\nindex_group = 16416\nindex_offset = 0\nsize = 4",
    );

    let error = parse_ads_toml(&text).expect_err("mixed address modes must fail");

    assert!(error
        .to_string()
        .contains("must use either symbol or index_group"));
}

#[test]
fn generates_deterministic_ads_interface_from_snapshot_and_config() {
    let config = parse_ads_toml(VALID_ADS_TOML).expect("valid ADS config");
    let snapshot = snapshot_for_line1(vec![
        real_symbol("MAIN.Temperature"),
        real_symbol("GVL.Setpoint"),
    ]);

    let generated = generate_ads_interface(&config, &[snapshot]).expect("generate interface");

    assert_eq!(generated.point_count, 3);
    assert_eq!(
        generated.source,
        concat!(
            "(* Generated by trust-runtime ads import-symbols. Do not edit by hand. *)\n",
            "TYPE\n",
            "    ADS_QUALITY : (Stale := 0, Good := 1, Error := 2);\n",
            "END_TYPE\n",
            "\n",
            "VAR_GLOBAL\n",
            "    (* ADS connection: line1 *)\n",
            "    (* symbol MAIN.Temperature *)\n",
            "    line1_temp : REAL;\n",
            "    line1_temp_quality : ADS_QUALITY := Stale;\n",
            "    (* symbol GVL.Setpoint *)\n",
            "    line1_setpoint : REAL;\n",
            "    line1_setpoint_quality : ADS_QUALITY := Stale;\n",
            "    (* index group=0x00004020 offset=0x00000000 size=2 *)\n",
            "    line1_status : WORD;\n",
            "    line1_status_quality : ADS_QUALITY := Stale;\n",
            "END_VAR\n",
        )
    );
}

#[test]
fn offline_validation_rejects_stale_generated_ads_interface() {
    let config = parse_ads_toml(VALID_ADS_TOML).expect("valid ADS config");
    let snapshot = snapshot_for_line1(vec![
        real_symbol("MAIN.Temperature"),
        real_symbol("GVL.Setpoint"),
    ]);
    let generated =
        generate_ads_interface(&config, std::slice::from_ref(&snapshot)).expect("generate");
    let stale = generated
        .source
        .replace("line1_temp : REAL", "line1_temp : LREAL");

    let error = validate_ads_interface_offline(&config, &[snapshot], &stale)
        .expect_err("stale generated ST must fail");

    assert!(error.to_string().contains("first difference at line"));
}

#[test]
fn offline_validation_accepts_generated_ads_interface() {
    let config = parse_ads_toml(VALID_ADS_TOML).expect("valid ADS config");
    let snapshot = snapshot_for_line1(vec![
        real_symbol("MAIN.Temperature"),
        real_symbol("GVL.Setpoint"),
    ]);
    let generated =
        generate_ads_interface(&config, std::slice::from_ref(&snapshot)).expect("generate");

    let report = validate_ads_interface_offline(&config, &[snapshot], generated.source.as_str())
        .expect("offline validation");

    assert_eq!(report.point_count, 3);
    assert_eq!(report.generated_bytes, generated.source.len());
}

#[test]
fn generated_ads_interface_compiles_offline_without_plc() {
    let config = parse_ads_toml(VALID_ADS_TOML).expect("valid ADS config");
    let snapshot = snapshot_for_line1(vec![
        real_symbol("MAIN.Temperature"),
        real_symbol("GVL.Setpoint"),
    ]);
    let generated = generate_ads_interface(&config, &[snapshot]).expect("generate");

    CompileSession::from_sources(vec![
        SourceFile::with_path("src/generated/ads_generated.st", generated.source),
        SourceFile::with_path(
            "src/main.st",
            r#"
PROGRAM Main
VAR_EXTERNAL
    line1_temp : REAL;
    line1_setpoint : REAL;
    line1_status : WORD;
    line1_temp_quality : ADS_QUALITY;
END_VAR

line1_setpoint := line1_temp;
line1_status := WORD#0;
END_PROGRAM
"#,
        ),
        SourceFile::with_path(
            "src/config.st",
            r#"
CONFIGURATION Config
RESOURCE CommRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        ),
    ])
    .build_runtime()
    .expect("generated ADS interface compiles without a PLC");
}

#[test]
fn generated_ads_interface_compiles_multiple_connections_in_one_file() {
    let config = parse_ads_toml(
        r#"
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"

[[connections]]
name = "line2"
target_net_id = "5.23.91.13.1.1"
host = "192.168.10.6"
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line2_temp"
type = "REAL"
"#,
    )
    .expect("valid multi-connection ADS config");
    let generated = generate_ads_interface(
        &config,
        &[
            snapshot_for_line1(vec![real_symbol("MAIN.Temperature")]),
            SymbolSnapshot::new("line2", vec![real_symbol("MAIN.Temperature")]),
        ],
    )
    .expect("generate multi-connection interface");

    assert_eq!(generated.source.matches("ADS_QUALITY : (").count(), 1);
    assert!(generated.source.contains("(* ADS connection: line1 *)"));
    assert!(generated.source.contains("(* ADS connection: line2 *)"));

    CompileSession::from_sources(vec![
        SourceFile::with_path("src/generated/ads_generated.st", generated.source),
        SourceFile::with_path(
            "src/main.st",
            r#"
PROGRAM Main
VAR_EXTERNAL
    line1_temp : REAL;
    line2_temp : REAL;
    line1_temp_quality : ADS_QUALITY;
    line2_temp_quality : ADS_QUALITY;
END_VAR
END_PROGRAM
"#,
        ),
        SourceFile::with_path(
            "src/config.st",
            r#"
CONFIGURATION Config
RESOURCE CommRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        ),
    ])
    .build_runtime()
    .expect("single generated ADS file compiles for multiple connections");
}

#[test]
fn generator_rejects_reserved_words_as_generated_identifiers() {
    let config = connection_with_points(vec![point_config(
        "VAR",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);
    let config = AdsClientConfig {
        connections: vec![config],
    };
    let snapshot = snapshot_for_line1(vec![real_symbol("MAIN.Temperature")]);

    let error = generate_ads_interface(&config, &[snapshot]).expect_err("reserved word must fail");

    assert!(error.to_string().contains("valid generated ST identifier"));
}

#[test]
fn generator_rejects_quality_name_collision() {
    let config = connection_with_points(vec![
        point_config(
            "line1_temp",
            "MAIN.Temperature",
            real_type(),
            PointAccess::Read,
            false,
        ),
        point_config(
            "line1_temp_quality",
            "GVL.LineReady",
            AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
            PointAccess::Read,
            false,
        ),
    ]);
    let config = AdsClientConfig {
        connections: vec![config],
    };
    let snapshot = snapshot_for_line1(vec![
        real_symbol("MAIN.Temperature"),
        SymbolDescriptor::new(
            "GVL.LineReady",
            AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
            0x4020,
            4,
            1,
        )
        .with_flag(SymbolFlag::Read),
    ]);

    let error =
        generate_ads_interface(&config, &[snapshot]).expect_err("quality collision must fail");

    assert!(error
        .to_string()
        .contains("collides with generated quality"));
}

#[test]
fn generator_rejects_snapshot_symbol_byte_size_mismatch() {
    let config = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);
    let config = AdsClientConfig {
        connections: vec![config],
    };
    let snapshot = snapshot_for_line1(vec![SymbolDescriptor::new(
        "MAIN.Temperature",
        real_type(),
        0x4020,
        0,
        8,
    )
    .with_flag(SymbolFlag::Read)]);

    let error =
        generate_ads_interface(&config, &[snapshot]).expect_err("byte-size mismatch must fail");

    assert!(error.to_string().contains("byte size mismatch"));
}

#[test]
fn generator_emits_string_and_array_types() {
    let config = connection_with_points(vec![
        point_config(
            "line1_label",
            "MAIN.Label",
            AdsDataTypeDescriptor::string("STRING", 32),
            PointAccess::Read,
            false,
        ),
        point_config(
            "line1_samples",
            "MAIN.Samples",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real)
                .with_dimensions(vec![ArrayDimension { lower: 1, upper: 3 }]),
            PointAccess::Read,
            false,
        ),
    ]);
    let config = AdsClientConfig {
        connections: vec![config],
    };
    let snapshot = snapshot_for_line1(vec![
        SymbolDescriptor::new(
            "MAIN.Label",
            AdsDataTypeDescriptor::string("STRING", 32),
            0x4020,
            0,
            33,
        )
        .with_flag(SymbolFlag::Read),
        SymbolDescriptor::new(
            "MAIN.Samples",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real)
                .with_dimensions(vec![ArrayDimension { lower: 1, upper: 3 }]),
            0x4020,
            40,
            12,
        )
        .with_flag(SymbolFlag::Read),
    ]);

    let generated = generate_ads_interface(&config, &[snapshot]).expect("generate");

    assert!(generated.source.contains("line1_label : STRING[32];"));
    assert!(generated
        .source
        .contains("line1_samples : ARRAY[1..3] OF REAL;"));
}

#[test]
fn mock_transport_resolves_reads_writes_and_tracks_symbol_version() {
    let symbol = SymbolDescriptor::new(
        "MAIN.Temperature",
        AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        0x4020,
        0,
        4,
    )
    .with_flag(SymbolFlag::Read)
    .with_flag(SymbolFlag::Write);
    let mut transport = MockAdsTransport::new(vec![symbol.clone()]);

    transport.connect().expect("connect");
    assert_eq!(transport.read_state(), Ok(AdsDeviceState::Run));
    assert_eq!(transport.upload_symbol_table(), Ok(vec![symbol]));

    let request = AdsHandleRequest {
        point_name: "line1_temp".to_string(),
        address: AdsPointAddress::Symbol("MAIN.Temperature".to_string()),
        data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
    };
    let handles = transport.resolve_handles(&[request]).expect("handles");
    transport.set_value("line1_temp", Value::Real(20.5), PointQuality::good(1));

    let reads = transport.sumup_read(&handles).expect("read");
    assert_eq!(reads[0].value, Some(Value::Real(20.5)));

    let qualities = transport
        .sumup_write(&[AdsWriteRequest {
            handle: handles[0].clone(),
            value: Value::Real(21.0),
        }])
        .expect("write");
    assert_eq!(qualities, vec![PointQuality::good(0)]);
    assert_eq!(transport.value("line1_temp"), Some(&Value::Real(21.0)));

    assert_eq!(transport.symbol_version(), Ok(1));
    transport.bump_symbol_version();
    assert_eq!(transport.symbol_version(), Ok(2));
}

#[test]
fn mock_transport_reports_missing_read_value_as_point_quality_error() {
    let symbol = SymbolDescriptor::new(
        "MAIN.Temperature",
        AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        0x4020,
        0,
        4,
    );
    let mut transport = MockAdsTransport::new(vec![symbol]);
    transport.connect().expect("connect");

    let handles = transport
        .resolve_handles(&[AdsHandleRequest {
            point_name: "line1_temp".to_string(),
            address: AdsPointAddress::Symbol("MAIN.Temperature".to_string()),
            data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        }])
        .expect("handles");
    let reads = transport.sumup_read(&handles).expect("batch succeeds");

    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].point_name, "line1_temp");
    assert_eq!(reads[0].value, None);
    assert_eq!(reads[0].quality.state, QualityState::Error);
    assert!(reads[0]
        .quality
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("is not set")));
}

#[test]
fn declared_binding_resolution_rejects_missing_global() {
    let runtime = Runtime::new();
    let connection = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);

    let error =
        resolve_declared_bindings(&runtime, &connection).expect_err("missing global must fail");

    assert!(error
        .to_string()
        .contains("failed to resolve declared global 'line1_temp'"));
}

#[test]
fn declared_binding_resolution_rejects_type_mismatch() {
    let runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::DINT,
        Value::DInt(0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);

    let error =
        resolve_declared_bindings(&runtime, &connection).expect_err("type mismatch must fail");

    assert!(error.to_string().contains("type mismatch"));
}

#[test]
fn retain_read_requires_opt_in_and_allowed_binding_starts_stale() {
    let runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::Retain,
    )]);
    let blocked = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);

    let error = resolve_declared_bindings(&runtime, &blocked)
        .expect_err("retained read target must require opt-in");

    assert!(error.to_string().contains("allow_retain_read=true"));

    let allowed = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        true,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &allowed).expect("opt-in binding");
    let bridge = AdsConnectionBridge::new(bindings).expect("bridge");

    let status = bridge.status("line1_temp").expect("status");
    assert_eq!(status.quality.state, QualityState::Stale);
    assert!(status
        .quality
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("first ADS update")));
}

#[test]
fn bridge_applies_reads_at_input_phase_and_publishes_writes_at_output_phase() {
    let mut runtime = runtime_with_globals(vec![
        (
            "line1_temp",
            TypeId::REAL,
            Value::Real(0.0),
            RetainPolicy::NonRetain,
        ),
        (
            "line1_setpoint",
            TypeId::REAL,
            Value::Real(0.0),
            RetainPolicy::NonRetain,
        ),
    ]);
    let connection = connection_with_points(vec![
        point_config(
            "line1_temp",
            "MAIN.Temperature",
            real_type(),
            PointAccess::Read,
            false,
        ),
        point_config(
            "line1_setpoint",
            "GVL.Setpoint",
            real_type(),
            PointAccess::Write,
            false,
        ),
    ]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let mut transport = MockAdsTransport::new(vec![
        real_symbol("MAIN.Temperature"),
        real_symbol("GVL.Setpoint"),
    ]);
    transport.set_value("line1_temp", Value::Real(20.5), PointQuality::good(10));
    let (mut bridge, mut worker) =
        AdsConnectionBridge::with_transport(transport, bindings).expect("bridge");
    worker.tick(0).expect("worker tick");

    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(0.0))
    );
    bridge
        .apply_inputs(runtime.storage_mut(), 11)
        .expect("apply inputs");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(20.5))
    );

    let setpoint_ref = runtime
        .storage()
        .ref_for_global("line1_setpoint")
        .expect("setpoint ref");
    assert!(runtime
        .storage_mut()
        .write_by_ref(setpoint_ref, Value::Real(31.0)));
    assert_eq!(worker.transport().value("line1_setpoint"), None);

    bridge
        .capture_outputs(runtime.storage_mut(), 12)
        .expect("capture outputs");
    assert_eq!(
        bridge.pending_write("line1_setpoint"),
        Some(Value::Real(31.0))
    );
    assert_eq!(worker.transport().value("line1_setpoint"), None);
    worker.tick(12).expect("publish queued output");
    assert_eq!(
        worker.transport().value("line1_setpoint"),
        Some(&Value::Real(31.0))
    );
    assert_eq!(bridge.pending_write("line1_setpoint"), None);
}

#[test]
fn client_rejects_non_finite_real_and_lreal_before_storage_apply() {
    let values = [
        (
            "REAL NaN",
            TypeId::REAL,
            real_type(),
            Value::Real(12.5),
            Value::Real(f32::NAN),
            4,
        ),
        (
            "REAL positive infinity",
            TypeId::REAL,
            real_type(),
            Value::Real(12.5),
            Value::Real(f32::INFINITY),
            4,
        ),
        (
            "REAL negative infinity",
            TypeId::REAL,
            real_type(),
            Value::Real(12.5),
            Value::Real(f32::NEG_INFINITY),
            4,
        ),
        (
            "LREAL NaN",
            TypeId::LREAL,
            AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal),
            Value::LReal(12.5),
            Value::LReal(f64::NAN),
            8,
        ),
        (
            "LREAL positive infinity",
            TypeId::LREAL,
            AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal),
            Value::LReal(12.5),
            Value::LReal(f64::INFINITY),
            8,
        ),
        (
            "LREAL negative infinity",
            TypeId::LREAL,
            AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal),
            Value::LReal(12.5),
            Value::LReal(f64::NEG_INFINITY),
            8,
        ),
    ];

    for mode in [UpdateMode::Poll, UpdateMode::Notify] {
        for (case, type_id, data_type, initial, non_finite, size) in &values {
            let mut runtime = runtime_with_globals(vec![(
                "line1_temp",
                *type_id,
                initial.clone(),
                RetainPolicy::NonRetain,
            )]);
            let mut point = point_config(
                "line1_temp",
                "MAIN.Temperature",
                data_type.clone(),
                PointAccess::Read,
                false,
            );
            point.mode = mode;
            let connection = connection_with_points(vec![point]);
            let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
            let symbol =
                SymbolDescriptor::new("MAIN.Temperature", data_type.clone(), 0x4020, 0, *size)
                    .with_flag(SymbolFlag::Read)
                    .with_flag(SymbolFlag::Write);
            let mut transport = MockAdsTransport::new(vec![symbol]);
            if mode == UpdateMode::Poll {
                transport.set_value("line1_temp", non_finite.clone(), PointQuality::good(10));
            }
            let (mut bridge, mut worker) =
                AdsConnectionBridge::with_transport(transport, bindings).expect("bridge");
            worker.tick(0).expect("connect ADS worker");
            if mode == UpdateMode::Notify {
                worker
                    .transport_mut()
                    .emit_notification("line1_temp", non_finite.clone(), PointQuality::good(10))
                    .expect("emit non-finite ADS notification");
                worker.tick(1).expect("drain ADS notification");
            }

            assert_eq!(
                bridge.state(),
                AdsConnectionState::Connected,
                "{case} in {mode:?} mode must not disconnect the ADS session"
            );
            let status = bridge.status("line1_temp").expect("point status");
            assert_eq!(
                status.quality.state,
                QualityState::Error,
                "{case} in {mode:?} mode must report point-data error quality"
            );
            assert!(
                status
                    .quality
                    .detail
                    .as_deref()
                    .is_some_and(|detail| !detail.is_empty()),
                "{case} in {mode:?} mode must expose diagnostic detail"
            );

            bridge
                .apply_inputs(runtime.storage_mut(), 11)
                .expect("rejecting input must preserve scan execution");
            assert_eq!(
                runtime.storage().get_global("line1_temp"),
                Some(initial),
                "{case} in {mode:?} mode must not reach PLC variable storage"
            );
        }
    }
}

#[test]
fn read_write_binding_seeds_write_baseline_from_first_good_read() {
    let mut runtime = runtime_with_globals(vec![(
        "line1_setpoint",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_setpoint",
        "GVL.Setpoint",
        real_type(),
        PointAccess::ReadWrite,
        false,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let mut transport = MockAdsTransport::new(vec![real_symbol("GVL.Setpoint")]);
    transport.set_value("line1_setpoint", Value::Real(10.0), PointQuality::good(10));
    let (mut bridge, mut worker) =
        AdsConnectionBridge::with_transport(transport, bindings).expect("bridge");
    worker.tick(0).expect("worker tick");

    bridge
        .apply_inputs(runtime.storage_mut(), 11)
        .expect("apply first read");
    assert_eq!(
        runtime.storage().get_global("line1_setpoint"),
        Some(&Value::Real(10.0))
    );
    bridge
        .capture_outputs(runtime.storage_mut(), 12)
        .expect("unchanged read/write point must not write back");
    assert_eq!(bridge.pending_write("line1_setpoint"), None);

    let setpoint_ref = runtime
        .storage()
        .ref_for_global("line1_setpoint")
        .expect("setpoint ref");
    assert!(runtime
        .storage_mut()
        .write_by_ref(setpoint_ref, Value::Real(88.8)));
    bridge
        .capture_outputs(runtime.storage_mut(), 13)
        .expect("changed read/write point queues write");
    assert_eq!(
        bridge.pending_write("line1_setpoint"),
        Some(Value::Real(88.8))
    );
    worker.tick(13).expect("publish read/write output");
    assert_eq!(
        worker.transport().value("line1_setpoint"),
        Some(&Value::Real(88.8))
    );
}

#[test]
fn spawned_worker_updates_cache_and_scan_applies_snapshot_only() {
    let mut runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let mut transport = MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]);
    transport.set_value("line1_temp", Value::Real(42.25), PointQuality::good(10));
    let (mut bridge, worker) =
        AdsConnectionBridge::with_transport(transport, bindings).expect("bridge");
    let thread = worker
        .spawn(Duration::from_millis(1))
        .expect("spawn worker");

    wait_until("ADS worker cached read", || {
        bridge
            .status("line1_temp")
            .is_some_and(|status| status.quality.state == QualityState::Good)
    });
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(0.0)),
        "worker cache updates must not mutate runtime storage mid-scan"
    );

    bridge
        .apply_inputs(runtime.storage_mut(), 50)
        .expect("apply cached input");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(42.25))
    );

    thread.shutdown().expect("shutdown worker");
}

#[test]
fn notify_binding_subscribes_skips_poll_and_applies_at_scan_boundary() {
    let mut runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let mut point = point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    );
    point.mode = UpdateMode::Notify;
    let connection = connection_with_points(vec![point]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (mut bridge, mut worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect and subscribe");
    let subscription_id = worker
        .subscription_for_point("line1_temp")
        .expect("subscription");
    assert_eq!(
        worker.transport().subscription_id_for_point("line1_temp"),
        Some(subscription_id)
    );
    assert_eq!(
        worker.transport().sumup_read_batches(),
        0,
        "notify-mode reads must not be cyclic sum-up polled"
    );

    worker
        .transport_mut()
        .emit_notification("line1_temp", Value::Real(22.75), PointQuality::good(10))
        .expect("mock notification");
    worker.tick(10).expect("drain notification");

    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(0.0)),
        "notification delivery must not mutate runtime storage mid-scan"
    );
    assert_eq!(
        bridge.status("line1_temp").expect("status").quality.state,
        QualityState::Good
    );

    bridge
        .apply_inputs(runtime.storage_mut(), 11)
        .expect("apply cached notification");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(22.75))
    );
    assert_eq!(worker.transport().sumup_read_batches(), 0);
}

#[test]
fn spawned_worker_drains_output_queue_off_scan_thread() {
    let mut runtime = runtime_with_globals(vec![(
        "line1_setpoint",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_setpoint",
        "GVL.Setpoint",
        real_type(),
        PointAccess::Write,
        false,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (mut bridge, worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("GVL.Setpoint")]),
        bindings,
    )
    .expect("bridge");
    let setpoint_ref = runtime
        .storage()
        .ref_for_global("line1_setpoint")
        .expect("setpoint ref");
    assert!(runtime
        .storage_mut()
        .write_by_ref(setpoint_ref, Value::Real(31.0)));

    bridge
        .capture_outputs(runtime.storage_mut(), 60)
        .expect("queue output");
    assert_eq!(
        bridge.pending_write("line1_setpoint"),
        Some(Value::Real(31.0))
    );

    let thread = worker
        .spawn(Duration::from_millis(1))
        .expect("spawn worker");
    wait_until("ADS worker drained queued write", || {
        bridge.pending_write("line1_setpoint").is_none()
            && bridge
                .status("line1_setpoint")
                .is_some_and(|status| status.quality.state == QualityState::Good)
    });

    thread.shutdown().expect("shutdown worker");
}

#[test]
fn notify_binding_resubscribes_after_symbol_version_change() {
    let runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let mut point = point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    );
    point.mode = UpdateMode::Notify;
    let connection = connection_with_points(vec![point]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (_bridge, mut worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect and subscribe");
    let first_subscription = worker
        .subscription_for_point("line1_temp")
        .expect("first subscription");

    worker.transport_mut().bump_symbol_version();
    worker.tick(999).expect("symbol-version check is throttled");
    assert_eq!(
        worker
            .subscription_for_point("line1_temp")
            .expect("same subscription"),
        first_subscription
    );

    worker.tick(1_000).expect("refresh and resubscribe");
    let second_subscription = worker
        .subscription_for_point("line1_temp")
        .expect("second subscription");
    assert!(second_subscription > first_subscription);
    assert_eq!(
        worker.transport().subscription_id_for_point("line1_temp"),
        Some(second_subscription)
    );
}

#[test]
fn notify_binding_refresh_disconnects_before_resubscribe() {
    let runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let mut point = point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    );
    point.mode = UpdateMode::Notify;
    let connection = connection_with_points(vec![point]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (_bridge, mut worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect and subscribe");
    assert_eq!(worker.transport().subscribe_count(), 1);
    assert_eq!(worker.transport().disconnect_count(), 0);

    worker.transport_mut().bump_symbol_version();
    worker.tick(1_000).expect("refresh and resubscribe");

    assert_eq!(
        worker.transport().disconnect_count(),
        1,
        "symbol-version refresh must release server-side ADS notifications before resubscribing"
    );
    assert_eq!(worker.transport().subscribe_count(), 2);
}

#[test]
fn read_bindings_do_not_publish_outputs() {
    let mut runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (mut bridge, mut worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]),
        bindings,
    )
    .expect("bridge");
    worker.tick(0).expect("worker tick");

    let temp_ref = runtime
        .storage()
        .ref_for_global("line1_temp")
        .expect("temp ref");
    assert!(runtime
        .storage_mut()
        .write_by_ref(temp_ref, Value::Real(99.0)));
    bridge
        .capture_outputs(runtime.storage_mut(), 20)
        .expect("capture outputs");

    assert_eq!(bridge.pending_write("line1_temp"), None);
    assert_eq!(worker.transport().value("line1_temp"), None);
}

#[test]
fn symbol_version_change_revalidates_and_resolves_new_handles() {
    let runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (_bridge, mut worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]),
        bindings,
    )
    .expect("bridge");
    worker.tick(0).expect("worker tick");
    let first = worker.handle_for_point("line1_temp").expect("first handle");

    worker.transport_mut().bump_symbol_version();
    worker.tick(999).expect("symbol-version check is throttled");
    assert_eq!(
        worker.handle_for_point("line1_temp").expect("same handle"),
        first
    );
    worker.tick(1_000).expect("refresh");

    let second = worker
        .handle_for_point("line1_temp")
        .expect("second handle");
    assert!(second > first);
}

#[test]
fn symbol_version_type_mismatch_faults_bridge() {
    let runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (bridge, mut worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]),
        bindings,
    )
    .expect("bridge");
    worker.tick(0).expect("worker tick");
    worker
        .transport_mut()
        .set_symbols(vec![SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal),
            0x4020,
            0,
            8,
        )
        .with_flag(SymbolFlag::Read)
        .with_flag(SymbolFlag::Write)]);
    worker.transport_mut().bump_symbol_version();
    worker
        .tick(500)
        .expect("symbol-version check should be throttled");
    assert_eq!(bridge.state(), AdsConnectionState::Connected);

    let error = worker.tick(1_000).expect_err("online type drift must fail");

    assert!(error.to_string().contains("type mismatch"));
    assert_eq!(bridge.state(), AdsConnectionState::Faulted);
}

#[test]
fn reconnect_marks_points_stale_until_backoff_allows_connect() {
    let runtime = runtime_with_globals(vec![(
        "line1_temp",
        TypeId::REAL,
        Value::Real(0.0),
        RetainPolicy::NonRetain,
    )]);
    let connection = connection_with_points(vec![point_config(
        "line1_temp",
        "MAIN.Temperature",
        real_type(),
        PointAccess::Read,
        false,
    )]);
    let bindings = resolve_declared_bindings(&runtime, &connection).expect("bindings");
    let (bridge, mut worker) = AdsConnectionBridge::with_transport(
        MockAdsTransport::new(vec![real_symbol("MAIN.Temperature")]),
        bindings,
    )
    .expect("bridge");
    worker.tick(0).expect("worker tick");

    worker.mark_reconnecting(100, "network fault");

    assert_eq!(bridge.state(), AdsConnectionState::Reconnecting);
    assert_eq!(
        bridge.status("line1_temp").expect("status").quality.state,
        QualityState::Stale
    );
    worker.tick(1_000).expect("backoff waits");
    assert_eq!(bridge.state(), AdsConnectionState::Reconnecting);
    worker.tick(2_100).expect("reconnect");
    assert_eq!(bridge.state(), AdsConnectionState::Connected);
}

fn runtime_with_globals(globals: Vec<(&str, TypeId, Value, RetainPolicy)>) -> Runtime {
    let mut runtime = Runtime::new();
    for (name, type_id, value, retain) in globals {
        let name = smol_str::SmolStr::new(name);
        runtime
            .storage_mut()
            .set_global(name.clone(), value.clone());
        runtime.register_global_meta(name, type_id, retain, crate::GlobalInitValue::Value(value));
    }
    runtime
}

fn connection_with_points(points: Vec<AdsPointConfig>) -> AdsConnectionConfig {
    AdsConnectionConfig {
        route: AdsRoute {
            name: "line1".to_string(),
            target_net_id: AmsNetId::new("5.23.91.12.1.1"),
            host: "192.168.10.5".to_string(),
            ams_port: 851,
            local_net_id: None,
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Plain,
                auto_add_route: false,
            },
        },
        points,
    }
}

fn point_config(
    point_name: &str,
    symbol_name: &str,
    data_type: AdsDataTypeDescriptor,
    access: PointAccess,
    allow_retain_read: bool,
) -> AdsPointConfig {
    AdsPointConfig {
        point_name: point_name.to_string(),
        address: AdsPointAddress::Symbol(symbol_name.to_string()),
        data_type,
        access,
        mode: UpdateMode::Poll,
        notification_mode: AdsNotificationMode::OnChange,
        allow_retain_read,
    }
}

fn real_type() -> AdsDataTypeDescriptor {
    AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real)
}

fn real_symbol(name: &str) -> SymbolDescriptor {
    SymbolDescriptor::new(name, real_type(), 0x4020, 0, 4)
        .with_flag(SymbolFlag::Read)
        .with_flag(SymbolFlag::Write)
}

fn snapshot_for_line1(symbols: Vec<SymbolDescriptor>) -> SymbolSnapshot {
    SymbolSnapshot::new("line1", symbols)
}

fn local_identity(net_id: &str) -> crate::ads::diagnostics::LocalIdentity {
    crate::ads::diagnostics::LocalIdentity {
        host_name: Some("line-controller".to_string()),
        chosen_ip: "192.168.10.20".to_string(),
        ams_net_id: net_id.to_string(),
        nic: Some("eth0".to_string()),
        candidates: Vec::new(),
        classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
    }
}

fn wait_until(label: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {label}");
}
