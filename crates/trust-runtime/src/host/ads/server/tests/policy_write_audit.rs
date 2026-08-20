use super::*;

#[test]
fn client_source_pins_enforce_exact_ip_and_ipv4_ipv6_cidr() {
    let mut config = config(false);
    config.clients = vec![
        AdsServerClientConfig {
            ams_net_id: AmsNetId::new("1.1.1.1.1.1"),
            source: AdsServerSourcePin::Ip(SmolStr::new("192.0.2.10")),
        },
        AdsServerClientConfig {
            ams_net_id: AmsNetId::new("2.2.2.2.2.2"),
            source: AdsServerSourcePin::Cidr(SmolStr::new("198.51.100.0/24")),
        },
        AdsServerClientConfig {
            ams_net_id: AmsNetId::new("3.3.3.3.3.3"),
            source: AdsServerSourcePin::Cidr(SmolStr::new("2001:db8::/32")),
        },
        AdsServerClientConfig {
            ams_net_id: AmsNetId::new("4.4.4.4.4.4"),
            source: AdsServerSourcePin::Ip(SmolStr::new("2001:db8::1")),
        },
        AdsServerClientConfig {
            ams_net_id: AmsNetId::new("5.5.5.5.5.5"),
            source: AdsServerSourcePin::Ip(SmolStr::new("not-an-ip")),
        },
        AdsServerClientConfig {
            ams_net_id: AmsNetId::new("6.6.6.6.6.6"),
            source: AdsServerSourcePin::Cidr(SmolStr::new("not-a-cidr")),
        },
    ];
    let policy = AdsServerClientPolicy::new(&config);

    assert!(policy
        .permits_client(&ClientId::new(AmsNetId::new("1.1.1.1.1.1")).with_source_ip("192.0.2.10")));
    assert!(!policy
        .permits_client(&ClientId::new(AmsNetId::new("1.1.1.1.1.1")).with_source_ip("192.0.2.11")));
    assert!(policy.permits_client(
        &ClientId::new(AmsNetId::new("2.2.2.2.2.2")).with_source_ip("198.51.100.255")
    ));
    assert!(!policy.permits_client(
        &ClientId::new(AmsNetId::new("2.2.2.2.2.2")).with_source_ip("198.51.101.1")
    ));
    assert!(policy.permits_client(
        &ClientId::new(AmsNetId::new("3.3.3.3.3.3")).with_source_ip("2001:db8:1::7")
    ));
    assert!(!policy.permits_client(
        &ClientId::new(AmsNetId::new("3.3.3.3.3.3")).with_source_ip("2001:db9::7")
    ));
    assert!(!policy.permits_client(
        &ClientId::new(AmsNetId::new("3.3.3.3.3.3")).with_source_ip("198.51.100.7")
    ));
    assert!(policy.permits_client(
        &ClientId::new(AmsNetId::new("4.4.4.4.4.4")).with_source_ip("2001:0db8:0:0:0:0:0:1")
    ));
    assert!(!policy
        .permits_client(&ClientId::new(AmsNetId::new("5.5.5.5.5.5")).with_source_ip("not-an-ip")));
    assert!(!policy
        .permits_client(&ClientId::new(AmsNetId::new("6.6.6.6.6.6")).with_source_ip("192.0.2.10")));
    assert!(!policy.permits_client(&ClientId::new(AmsNetId::new("1.1.1.1.1.1"))));
}

#[test]
fn unpinned_client_requires_explicit_runtime_opt_in() {
    let client = ClientId::new(AmsNetId::new("5.23.91.12.1.1"));
    let allowed = AdsServerClientConfig {
        ams_net_id: client.ams_net_id.clone(),
        source: AdsServerSourcePin::Unpinned,
    };
    let mut config = config(false);
    config.clients = vec![allowed.clone()];
    config.allow_unpinned_clients = false;

    assert!(!AdsServerClientPolicy::new(&config).permits_client(&client));

    config.clients = vec![allowed];
    config.allow_unpinned_clients = true;
    assert!(AdsServerClientPolicy::new(&config).permits_client(&client));
}

#[test]
fn refused_attempts_coalesce_and_history_is_bounded() {
    let config = AdsServerRuntimeConfig::default();
    let coalescing = AdsServerClientPolicy::new(&config);
    let repeated = ClientId::new(AmsNetId::new("1.2.3.4.5.6")).with_source_ip("192.0.2.10");

    assert!(!coalescing.permits_client(&repeated));
    assert!(!coalescing.permits_client(&repeated));
    let refused = coalescing.recently_refused_clients();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].count, 2);

    let bounded = AdsServerClientPolicy::new(&config);
    for suffix in 0..33 {
        let client = ClientId::new(AmsNetId::new(format!("10.0.{suffix}.1.1.1")))
            .with_source_ip(format!("192.0.2.{suffix}"));
        assert!(!bounded.permits_client(&client));
    }
    let refused = bounded.recently_refused_clients();
    assert_eq!(refused.len(), 32);
    assert!(refused
        .iter()
        .all(|entry| entry.ams_net_id != "10.0.0.1.1.1"));
    assert_eq!(
        refused.last().map(|entry| entry.ams_net_id.as_str()),
        Some("10.0.32.1.1.1")
    );
}

#[test]
fn invalid_writable_glob_rejects_write_port_construction() {
    let mut config = config(true);
    config.writable = vec![SmolStr::new("[")];

    let error = match AdsServerRuntimeWritePort::new(
        config.clone(),
        AdsServerClientPolicy::new(&config),
        DebugControl::new(),
        resource_control(),
    ) {
        Ok(_) => panic!("invalid writable glob was accepted"),
        Err(error) => error,
    };

    assert_eq!(error.code(), AdsErrorCode::InvalidParameter);
    assert!(error.message().contains("runtime.ads_server.writable"));
}

#[test]
fn write_port_accepts_only_ready_running_or_paused_resources() {
    let accepted = [
        ResourceState::Ready,
        ResourceState::Running,
        ResourceState::Paused,
    ];
    for state in accepted {
        let (write_port, debug, symbol) = write_port_for_state(state, None);
        write_port
            .write(
                &symbol,
                &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
                &allowed_client(),
            )
            .unwrap_or_else(|error| panic!("{state:?} rejected write: {error}"));
        assert_eq!(debug.drain_var_writes().len(), 1, "{state:?}");
    }

    let rejected = [
        ResourceState::Boot,
        ResourceState::Faulted,
        ResourceState::Stopped,
    ];
    for state in rejected {
        let (write_port, debug, symbol) = write_port_for_state(state, None);
        let error = write_port
            .write(
                &symbol,
                &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
                &allowed_client(),
            )
            .unwrap_err();
        assert_eq!(error.code(), AdsErrorCode::NotReady, "{state:?}");
        assert!(debug.drain_var_writes().is_empty(), "{state:?}");
    }

    let (write_port, debug, symbol) =
        write_port_for_state(ResourceState::Ready, Some(RuntimeError::ResourceFaulted));
    let error = write_port
        .write(
            &symbol,
            &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
            &allowed_client(),
        )
        .expect_err("last runtime error rejects write");
    assert_eq!(error.code(), AdsErrorCode::NotReady);
    assert!(debug.drain_var_writes().is_empty());
}

#[test]
fn write_port_error_priority_is_security_first_and_mutation_free() {
    let config = config(true);
    let symbols = build_runtime_symbol_snapshot(
        &config,
        &snapshot([("setpoint", Value::Real(0.0)), ("other", Value::Real(0.0))]),
    )
    .expect("symbol snapshot");
    let setpoint = symbols
        .symbols
        .iter()
        .find(|symbol| symbol.name == "global.setpoint")
        .expect("setpoint")
        .clone();
    let other = symbols
        .symbols
        .iter()
        .find(|symbol| symbol.name == "global.other")
        .expect("other")
        .clone();
    let debug = DebugControl::new();
    let (resource, _rx) =
        ResourceControl::stub_with_state(StdClock::new(), ResourceState::Ready, None);
    let write_port = AdsServerRuntimeWritePort::new(
        config.clone(),
        AdsServerClientPolicy::new(&config),
        debug.clone(),
        resource,
    )
    .expect("write port");
    let valid_bytes =
        ads_bytes_from_value(&setpoint.data_type, &Value::Real(1.0)).expect("REAL bytes");

    let mut foreign = setpoint.clone();
    foreign.name = "foreign.setpoint".to_string();
    let error = write_port
        .write(&foreign, &valid_bytes, &allowed_client())
        .expect_err("foreign namespace");
    assert_eq!(error.code(), AdsErrorCode::AccessDenied);
    assert!(debug.drain_var_writes().is_empty());

    let error = write_port
        .write(&other, &valid_bytes, &allowed_client())
        .expect_err("non-writable symbol");
    assert_eq!(error.code(), AdsErrorCode::AccessDenied);
    assert!(debug.drain_var_writes().is_empty());

    let error = write_port
        .write(&setpoint, &[0], &allowed_client())
        .expect_err("malformed REAL payload");
    assert_eq!(error.code(), AdsErrorCode::InvalidData);
    assert!(debug.drain_var_writes().is_empty());

    let (stopped, stopped_debug, mut stopped_symbol) =
        write_port_for_state(ResourceState::Stopped, None);
    stopped_symbol.name = "foreign.setpoint".to_string();
    let error = stopped
        .write(&stopped_symbol, &valid_bytes, &allowed_client())
        .expect_err("symbol authorization precedes runtime readiness");
    assert_eq!(error.code(), AdsErrorCode::AccessDenied);
    assert!(stopped_debug.drain_var_writes().is_empty());
}

#[test]
fn audit_sink_preserves_policy_rejection_context() {
    let (tx, rx) = channel();
    let sink = AdsServerRuntimeAuditSink::new(Some(tx));
    let event = AdsServerAuditEvent::new(
        AdsServerAuditKind::PolicyRejected,
        allowed_client(),
        None,
        Err(AdsServerError::device(
            AdsErrorCode::AccessDenied,
            "source pin rejected",
        )),
        77,
    );

    sink.record(&event);

    let audit = rx.recv().expect("policy audit");
    assert_eq!(audit.timestamp_ms, 77);
    assert_eq!(audit.request_type.as_str(), "ads.server.policy");
    assert!(!audit.ok);
    assert!(!audit.auth_present);
    assert_eq!(audit.details.as_ref().unwrap()["kind"], "policy_rejected");
    assert_eq!(
        audit.details.as_ref().unwrap()["client_ams_net_id"],
        "5.23.91.12.1.1"
    );
    assert_eq!(
        audit.details.as_ref().unwrap()["ads_error"]["code"],
        AdsErrorCode::AccessDenied.value()
    );
    assert!(audit
        .client
        .as_deref()
        .is_some_and(|client| client.contains("source_ip=192.168.10.50")));

    sink.record(&event);
    let repeated = rx.recv().expect("repeated policy audit");
    assert_ne!(audit.event_id, repeated.event_id);

    drop(rx);
    sink.record(&event);
}

fn write_port_for_state(
    state: ResourceState,
    last_error: Option<RuntimeError>,
) -> (
    AdsServerRuntimeWritePort,
    DebugControl,
    trust_ads_core::SymbolDescriptor,
) {
    let config = config(true);
    let symbol =
        build_runtime_symbol_snapshot(&config, &snapshot([("setpoint", Value::Real(0.0))]))
            .expect("symbol snapshot")
            .symbols
            .into_iter()
            .next()
            .expect("setpoint symbol");
    let debug = DebugControl::new();
    let (resource, _rx) = ResourceControl::stub_with_state(StdClock::new(), state, last_error);
    let write_port = AdsServerRuntimeWritePort::new(
        config.clone(),
        AdsServerClientPolicy::new(&config),
        debug.clone(),
        resource,
    )
    .expect("write port");
    (write_port, debug, symbol)
}
