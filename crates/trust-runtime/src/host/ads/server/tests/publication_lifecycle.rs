use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

use super::*;
use crate::ads::server::{
    global_name_for_server_symbol, server_symbol_name, DEFAULT_ADS_SERVER_ADS_PORT,
    DEFAULT_ADS_SERVER_IDLE_TIMEOUT_MS, DEFAULT_ADS_SERVER_MAX_CLIENTS,
    DEFAULT_ADS_SERVER_MAX_FRAME_BYTES, DEFAULT_ADS_SERVER_MAX_STRING_BYTES,
    DEFAULT_ADS_SERVER_MAX_SUBSCRIPTIONS_PER_CLIENT, DEFAULT_ADS_SERVER_MAX_SUMUP_ITEMS,
    DEFAULT_ADS_SERVER_MAX_SYMBOLS, DEFAULT_ADS_SERVER_MAX_TOTAL_SUBSCRIPTIONS,
    DEFAULT_ADS_SERVER_MAX_WRITE_BYTES, DEFAULT_ADS_SERVER_MIN_NOTIFICATION_CYCLE_MS,
    DEFAULT_ADS_SERVER_READ_TIMEOUT_MS,
};

#[test]
fn server_runtime_defaults_preserve_bounded_product_limits() {
    let config = AdsServerRuntimeConfig::default();

    assert_eq!(
        (
            DEFAULT_ADS_SERVER_ADS_PORT,
            DEFAULT_ADS_SERVER_MAX_SYMBOLS,
            DEFAULT_ADS_SERVER_MAX_CLIENTS,
            DEFAULT_ADS_SERVER_MAX_SUBSCRIPTIONS_PER_CLIENT,
            DEFAULT_ADS_SERVER_MAX_TOTAL_SUBSCRIPTIONS,
            DEFAULT_ADS_SERVER_MAX_FRAME_BYTES,
            DEFAULT_ADS_SERVER_MAX_SUMUP_ITEMS,
            DEFAULT_ADS_SERVER_MAX_WRITE_BYTES,
            DEFAULT_ADS_SERVER_MAX_STRING_BYTES,
            DEFAULT_ADS_SERVER_READ_TIMEOUT_MS,
            DEFAULT_ADS_SERVER_IDLE_TIMEOUT_MS,
            DEFAULT_ADS_SERVER_MIN_NOTIFICATION_CYCLE_MS,
        ),
        (851, 256, 8, 64, 256, 65_536, 512, 8_192, 4_096, 5_000, 60_000, 50)
    );
    assert!(!config.enabled);
    assert_eq!(
        (
            config.ads_port,
            config.max_symbols,
            config.max_clients,
            config.max_subscriptions_per_client,
            config.max_total_subscriptions,
            config.max_frame_bytes,
            config.max_sumup_items,
            config.max_write_bytes,
            config.max_string_bytes,
            config.read_timeout_ms,
            config.idle_timeout_ms,
            config.min_notification_cycle_ms,
        ),
        (851, 256, 8, 64, 256, 65_536, 512, 8_192, 4_096, 5_000, 60_000, 50)
    );
    assert!(!config.insecure_transport);
    assert!(!config.writes_enabled);
    assert!(!config.allow_unpinned_clients);
    assert!(!config.unsafe_allow_public_bind);
    assert!(config.expose.is_empty());
    assert!(config.writable.is_empty());
    assert!(config.clients.is_empty());
}

#[test]
fn symbol_limit_sorts_canonical_names_before_truncation() {
    let mut config = config(false);
    config.max_symbols = 2;
    let snapshot = snapshot([
        ("zulu", Value::Bool(true)),
        ("alpha", Value::Bool(true)),
        ("middle", Value::Bool(true)),
    ]);

    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let names = symbols
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["global.alpha", "global.middle"]);
}

#[test]
fn scalar_publication_preserves_supported_types_and_write_gate() {
    let mut config = config(true);
    config.writable = vec![SmolStr::new("global.*")];
    let snapshot = snapshot([
        ("bool", Value::Bool(true)),
        ("sint", Value::SInt(-1)),
        ("int", Value::Int(-2)),
        ("dint", Value::DInt(-3)),
        ("lint", Value::LInt(-4)),
        ("usint", Value::USInt(1)),
        ("uint", Value::UInt(2)),
        ("udint", Value::UDInt(3)),
        ("ulint", Value::ULInt(4)),
        ("real", Value::Real(1.5)),
        ("lreal", Value::LReal(2.5)),
        ("byte", Value::Byte(1)),
        ("word", Value::Word(2)),
        ("dword", Value::DWord(3)),
        ("lword", Value::LWord(4)),
    ]);
    let expected = [
        ("global.bool", IecDataType::Bool),
        ("global.sint", IecDataType::Sint),
        ("global.int", IecDataType::Int),
        ("global.dint", IecDataType::Dint),
        ("global.lint", IecDataType::Lint),
        ("global.usint", IecDataType::Usint),
        ("global.uint", IecDataType::Uint),
        ("global.udint", IecDataType::Udint),
        ("global.ulint", IecDataType::Ulint),
        ("global.real", IecDataType::Real),
        ("global.lreal", IecDataType::Lreal),
        ("global.byte", IecDataType::Byte),
        ("global.word", IecDataType::Word),
        ("global.dword", IecDataType::Dword),
        ("global.lword", IecDataType::Lword),
    ];

    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");

    assert_eq!(symbols.symbols.len(), expected.len());
    for (name, iec_type) in expected {
        let symbol = symbols
            .symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(symbol.data_type.iec_type, iec_type, "{name}");
        assert!(symbol.flags.contains(&SymbolFlag::Read), "{name}");
        assert!(symbol.flags.contains(&SymbolFlag::Write), "{name}");
    }

    config.writes_enabled = false;
    let read_only = build_runtime_symbol_snapshot(&config, &snapshot).expect("read-only symbols");
    assert!(read_only
        .symbols
        .iter()
        .all(|symbol| !symbol.flags.contains(&SymbolFlag::Write)));
}

#[test]
fn symbol_namespace_mapping_is_exact_and_reversible() {
    let mut config = config(false);
    config.symbol_namespace = SmolStr::new("line_1");

    let canonical = server_symbol_name(&config, "setpoint");

    assert_eq!(canonical, "line_1.global.setpoint");
    assert_eq!(
        global_name_for_server_symbol(&config, canonical.as_str()),
        Some("setpoint")
    );
    assert_eq!(
        global_name_for_server_symbol(&config, "line_2.global.setpoint"),
        None
    );
    assert_eq!(global_name_for_server_symbol(&config, "setpoint"), None);
}

#[test]
fn bare_runtime_name_globs_match_namespaced_symbols() {
    let mut config = config(true);
    config.symbol_namespace = SmolStr::new("line_1");
    config.expose = vec![SmolStr::new("setpoint")];
    config.writable = vec![SmolStr::new("setpoint")];

    let symbols =
        build_runtime_symbol_snapshot(&config, &snapshot([("setpoint", Value::Real(1.0))]))
            .expect("symbol snapshot");

    assert_eq!(symbols.symbols.len(), 1);
    assert_eq!(symbols.symbols[0].name, "line_1.global.setpoint");
    assert!(symbols.symbols[0].flags.contains(&SymbolFlag::Write));
}

#[test]
fn empty_expose_publishes_nothing_and_invalid_globs_fail_closed() {
    let mut empty = config(true);
    empty.expose.clear();
    let symbols =
        build_runtime_symbol_snapshot(&empty, &snapshot([("setpoint", Value::Real(1.0))]))
            .expect("empty symbol snapshot");
    assert!(symbols.symbols.is_empty());

    let mut invalid_expose = config(false);
    invalid_expose.expose = vec![SmolStr::new("[")];
    let error =
        build_runtime_symbol_snapshot(&invalid_expose, &snapshot([("setpoint", Value::Real(1.0))]))
            .expect_err("invalid expose glob");
    assert!(error.to_string().contains("runtime.ads_server.expose"));

    let mut invalid_writable = config(true);
    invalid_writable.writable = vec![SmolStr::new("[")];
    let error = build_runtime_symbol_snapshot(
        &invalid_writable,
        &snapshot([("setpoint", Value::Real(1.0))]),
    )
    .expect_err("invalid writable glob");
    assert!(error.to_string().contains("runtime.ads_server.writable"));
}

#[test]
fn publisher_rejects_foreign_or_missing_runtime_globals_without_bytes() {
    let config = config(false);
    let descriptor_snapshot = snapshot([("setpoint", Value::Real(1.0))]);
    let symbol = build_runtime_symbol_snapshot(&config, &descriptor_snapshot)
        .expect("symbol snapshot")
        .symbols
        .into_iter()
        .next()
        .expect("setpoint symbol");
    let available = snapshot([("other", Value::Real(2.0))]);
    let publisher =
        AdsServerValuePublisher::new(config.clone(), Arc::new(move || Some(available.clone())));

    let missing = publisher.read(&symbol).expect_err("runtime global missing");
    assert_eq!(missing.code(), AdsErrorCode::NotFound);

    let mut foreign = symbol;
    foreign.name = "foreign.setpoint".to_string();
    let foreign_error = publisher.read(&foreign).expect_err("foreign symbol");
    assert_eq!(foreign_error.code(), AdsErrorCode::NotFound);
}

#[test]
fn publisher_rejects_incompatible_values_and_size_mismatch_without_bytes() {
    let config = config(false);
    let descriptor_snapshot = snapshot([("setpoint", Value::Real(1.0))]);
    let symbol = build_runtime_symbol_snapshot(&config, &descriptor_snapshot)
        .expect("symbol snapshot")
        .symbols
        .into_iter()
        .next()
        .expect("setpoint symbol");

    let incompatible = snapshot([("setpoint", Value::Bool(true))]);
    let incompatible_publisher =
        AdsServerValuePublisher::new(config.clone(), Arc::new(move || Some(incompatible.clone())));
    let error = incompatible_publisher
        .read(&symbol)
        .expect_err("incompatible runtime value");
    assert_eq!(error.code(), AdsErrorCode::InvalidData);

    let current = snapshot([("setpoint", Value::Real(1.0))]);
    let size_publisher =
        AdsServerValuePublisher::new(config, Arc::new(move || Some(current.clone())));
    let mut wrong_size = symbol;
    wrong_size.byte_size += 1;
    let error = size_publisher
        .read(&wrong_size)
        .expect_err("descriptor size mismatch");
    assert_eq!(error.code(), AdsErrorCode::InvalidSize);
}

#[test]
fn disabled_lifecycle_avoids_snapshot_and_socket_work() {
    let config = AdsServerRuntimeConfig::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider_calls = calls.clone();

    let server = super::super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        DebugControl::new(),
        resource_control(),
        Arc::new(move || {
            provider_calls.fetch_add(1, Ordering::SeqCst);
            None
        }),
        None,
        0,
    )
    .expect("disabled lifecycle");

    assert!(server.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn lifecycle_client_count_shutdown_and_drop_are_observable() {
    let mut config = config(false);
    config.listen = Some(SmolStr::new("127.0.0.1"));
    config.ams_net_id = Some(AmsNetId::new("127.0.0.1.1.1"));
    let runtime_snapshot = snapshot([("setpoint", Value::Real(0.0))]);
    let start_server = || {
        super::super::lifecycle::start_ads_server_runtime_on_port(
            "RESOURCE",
            &config,
            DebugControl::new(),
            resource_control(),
            Arc::new({
                let runtime_snapshot = runtime_snapshot.clone();
                move || Some(runtime_snapshot.clone())
            }),
            None,
            0,
        )
        .expect("start ADS server")
        .expect("server enabled")
    };

    let mut shutdown_server = start_server();
    let shutdown_addr = shutdown_server.local_addr();
    let shutdown_identify_addr = shutdown_server.identify_bind_addr();
    let client = TcpStream::connect(shutdown_addr).expect("connect client");
    wait_for_client_count(&shutdown_server, 1);
    drop(client);
    wait_for_client_count(&shutdown_server, 0);
    shutdown_server.shutdown();
    assert!(
        TcpStream::connect(shutdown_addr).is_err(),
        "explicit shutdown must stop the TCP listener"
    );
    let rebound_identify = UdpSocket::bind(shutdown_identify_addr)
        .expect("explicit shutdown must release the UDP identify socket");
    drop(rebound_identify);

    let drop_server = start_server();
    let drop_addr = drop_server.local_addr();
    let drop_identify_addr = drop_server.identify_bind_addr();
    drop(drop_server);
    assert!(
        TcpStream::connect(drop_addr).is_err(),
        "Drop must stop the TCP listener"
    );
    UdpSocket::bind(drop_identify_addr).expect("Drop must release the UDP identify socket");
}

fn wait_for_client_count(server: &super::super::AdsServerRuntime, expected: usize) {
    let deadline = Instant::now() + StdDuration::from_secs(2);
    while server.connected_clients() != expected && Instant::now() < deadline {
        thread::sleep(StdDuration::from_millis(10));
    }
    assert_eq!(server.connected_clients(), expected);
}
