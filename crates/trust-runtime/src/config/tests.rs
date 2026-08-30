use super::{
    parser::{parse_io_toml_from_text, parse_runtime_toml_from_text},
    validate_io_toml_text, validate_opcua_client_toml_text, validate_runtime_toml_text,
    RuntimeBundle, RuntimeConfig,
};

fn runtime_toml() -> String {
    r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

[runtime]
execution_backend = "vm"

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"

[runtime.web]
enabled = true
listen = "0.0.0.0:8080"
auth = "local"
tls = false

[runtime.discovery]
enabled = true
service_name = "truST"
advertise = true
interfaces = ["eth0"]

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
publish = []
subscribe = {}

[runtime.cloud]
profile = "dev"

[runtime.cloud.wan]
allow_write = []
"#
    .to_string()
}

fn io_toml() -> String {
    r#"
[io]
driver = "loopback"
params = {}
"#
    .to_string()
}

#[test]
fn runtime_schema_rejects_unknown_keys() {
    let text = format!("{}\n[runtime.extra]\nflag = true\n", runtime_toml());
    let err = validate_runtime_toml_text(&text).expect_err("runtime schema should fail");
    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn runtime_schema_rejects_invalid_ranges() {
    let text = runtime_toml().replace("cycle_interval_ms = 100", "cycle_interval_ms = 0");
    let err = validate_runtime_toml_text(&text).expect_err("cycle interval range should fail");
    assert!(err
        .to_string()
        .contains("resource.cycle_interval_ms must be >= 1"));
}

#[test]
fn runtime_schema_preserves_signed_duration_bounds() {
    let maximum_millis = (i64::MAX / 1_000_000) as u64;
    let text = runtime_toml().replace(
        "cycle_interval_ms = 100",
        &format!("cycle_interval_ms = {maximum_millis}"),
    );
    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("maximum signed runtime duration must remain representable");
    assert_eq!(
        runtime.cycle_interval,
        crate::value::Duration::from_millis(maximum_millis as i64)
    );

    let above_maximum = maximum_millis + 1;
    let cases = [
        (
            "resource.cycle_interval_ms",
            runtime_toml().replace(
                "cycle_interval_ms = 100",
                &format!("cycle_interval_ms = {above_maximum}"),
            ),
        ),
        (
            "runtime.retain.save_interval_ms",
            runtime_toml().replace(
                "save_interval_ms = 1000",
                &format!("save_interval_ms = {above_maximum}"),
            ),
        ),
        (
            "runtime.watchdog.timeout_ms",
            runtime_toml().replace(
                "timeout_ms = 5000",
                &format!("timeout_ms = {above_maximum}"),
            ),
        ),
        (
            "resource.tasks[].interval_ms",
            runtime_toml().replace(
                "cycle_interval_ms = 100",
                &format!(
                    "cycle_interval_ms = 100\n\n[[resource.tasks]]\nname = \"Fast\"\ninterval_ms = {above_maximum}\npriority = 1\nprograms = [\"Main\"]"
                ),
            ),
        ),
        (
            "runtime.ads.worker_tick_interval_ms",
            format!(
                "{}\n[runtime.ads]\nworker_tick_interval_ms = {above_maximum}\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.opcua_client.poll_interval_ms",
            format!(
                "{}\n[runtime.opcua_client]\npoll_interval_ms = {above_maximum}\n",
                runtime_toml()
            ),
        ),
    ];
    for (field, text) in cases {
        let error = validate_runtime_toml_text(&text)
            .expect_err("a duration outside the signed runtime range must reject");
        assert!(
            error.to_string().contains(field),
            "unexpected error for {field}: {error}"
        );
        assert!(error
            .to_string()
            .contains(&format!("must be <= {maximum_millis}")));
    }
}

#[test]
fn runtime_schema_rejects_blank_explicit_paths() {
    let cases = [
        (
            "runtime.ads.config_path",
            format!("{}\n[runtime.ads]\nconfig_path = \"   \"\n", runtime_toml()),
        ),
        (
            "runtime.opcua_client.config_path",
            format!(
                "{}\n[runtime.opcua_client]\nconfig_path = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.hmi_persistence.history_path",
            format!(
                "{}\n[runtime.hmi_persistence]\nhistory_path = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.observability.history_path",
            format!(
                "{}\n[runtime.observability]\nhistory_path = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.retain.path",
            runtime_toml().replace(
                "save_interval_ms = 1000",
                "path = \"   \"\nsave_interval_ms = 1000",
            ),
        ),
        (
            "runtime.openot.path",
            format!("{}\n[runtime.openot]\npath = \"   \"\n", runtime_toml()),
        ),
        (
            "runtime.openot.producer_instance",
            format!(
                "{}\n[runtime.openot]\nproducer_instance = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.observability.prometheus_path",
            format!(
                "{}\n[runtime.observability]\nprometheus_path = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.ads_server.listen",
            format!(
                "{}\n[runtime.ads_server]\nlisten = \"   \"\n",
                runtime_toml()
            ),
        ),
    ];
    for (field, text) in cases {
        let error = validate_runtime_toml_text(&text)
            .expect_err("an explicit blank configuration path must reject");
        assert!(
            error.to_string().contains(field),
            "unexpected error for {field}: {error}"
        );
    }
}

#[test]
fn runtime_schema_rejects_blank_explicit_scalars() {
    let cases = [
        (
            "resource.tasks[].single",
            runtime_toml().replace(
                "cycle_interval_ms = 100",
                "cycle_interval_ms = 100\n\n[[resource.tasks]]\nname = \"Fast\"\ninterval_ms = 10\npriority = 1\nprograms = [\"Main\"]\nsingle = \"   \"",
            ),
        ),
        (
            "runtime.control.auth_token",
            runtime_toml().replace(
                "endpoint = \"unix:///tmp/trust-runtime.sock\"",
                "endpoint = \"unix:///tmp/trust-runtime.sock\"\nauth_token = \"   \"",
            ),
        ),
        (
            "runtime.discovery.host_group",
            runtime_toml().replace(
                "interfaces = [\"eth0\"]",
                "interfaces = [\"eth0\"]\nhost_group = \"   \"",
            ),
        ),
        (
            "runtime.mesh.auth_token",
            runtime_toml().replace(
                "listen = \"0.0.0.0:5200\"\ntls = false",
                "listen = \"0.0.0.0:5200\"\ntls = false\nauth_token = \"   \"",
            ),
        ),
        (
            "runtime.ads_server.ams_net_id",
            format!(
                "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"127.0.0.1\"\ninsecure_transport = true\nams_net_id = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.ads_server.clients[].source_ip",
            format!(
                "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"127.0.0.1\"\ninsecure_transport = true\nallow_unpinned_clients = true\n[[runtime.ads_server.clients]]\nams_net_id = \"5.23.91.12.1.1\"\nsource_ip = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.observability.alerts[].hook",
            format!(
                "{}\n[runtime.observability]\n[[runtime.observability.alerts]]\nname = \"HighTemp\"\nvariable = \"Main.Temp\"\nabove = 80.0\nhook = \"   \"\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.opcua.username",
            format!(
                "{}\n[runtime.opcua]\nenabled = true\nallow_anonymous = true\nsecurity_policy = \"none\"\nsecurity_mode = \"none\"\nusername = \"   \"\npassword = \"   \"\n",
                runtime_toml()
            ),
        ),
    ];
    for (field, text) in cases {
        let error = validate_runtime_toml_text(&text)
            .expect_err("an explicit blank scalar must reject instead of becoming omitted");
        assert!(
            error.to_string().contains(field),
            "unexpected error for {field}: {error}"
        );
    }
}

#[test]
fn runtime_schema_rejects_blank_explicit_collection_entries() {
    let cases = [
        (
            "runtime.discovery.interfaces",
            runtime_toml().replace("interfaces = [\"eth0\"]", "interfaces = [\"   \"]"),
        ),
        (
            "runtime.mesh.connect",
            runtime_toml().replace("publish = []", "connect = [\"   \"]\npublish = []"),
        ),
        (
            "runtime.mesh.publish",
            runtime_toml().replace("publish = []", "publish = [\"   \"]"),
        ),
        (
            "runtime.mesh.subscribe",
            runtime_toml().replace("subscribe = {}", "subscribe = { \"Source\" = \"   \" }"),
        ),
        (
            "runtime.mesh.plugin_versions",
            runtime_toml().replace(
                "subscribe = {}",
                "subscribe = {}\nplugin_versions = { \"plugin\" = \"   \" }",
            ),
        ),
        (
            "runtime.ads_server.expose",
            format!(
                "{}\n[runtime.ads_server]\nexpose = [\"   \"]\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.opcua.expose",
            format!("{}\n[runtime.opcua]\nexpose = [\"   \"]\n", runtime_toml()),
        ),
        (
            "runtime.openot.producer_instances",
            format!(
                "{}\n[runtime.openot]\nsource = \"st-fb\"\nproducer_instances = [\"Main.Producer\", \"   \"]\n",
                runtime_toml()
            ),
        ),
        (
            "runtime.observability.include",
            format!(
                "{}\n[runtime.observability]\ninclude = [\"   \"]\n",
                runtime_toml()
            ),
        ),
    ];
    for (field, text) in cases {
        let error = validate_runtime_toml_text(&text)
            .expect_err("an explicit blank collection entry must reject");
        assert!(
            error.to_string().contains(field),
            "unexpected error for {field}: {error}"
        );
    }
}

#[test]
fn runtime_schema_accepts_vm_execution_backend() {
    let text = runtime_toml();
    validate_runtime_toml_text(&text).expect("vm backend setting should validate");
}

#[test]
fn runtime_schema_defaults_execution_backend_when_omitted() {
    let text = runtime_toml().replace("[runtime]\nexecution_backend = \"vm\"\n\n", "");
    validate_runtime_toml_text(&text).expect("execution backend default should validate");
}

#[test]
fn runtime_schema_rejects_invalid_execution_backend() {
    let text = runtime_toml().replace(
        "execution_backend = \"vm\"",
        "execution_backend = \"bytecode\"",
    );
    let err = validate_runtime_toml_text(&text).expect_err("execution backend enum should fail");
    assert!(err
        .to_string()
        .contains("invalid runtime.execution_backend 'bytecode'"));
}

#[test]
fn runtime_schema_rejects_interpreter_execution_backend_for_production() {
    let text = runtime_toml().replace(
        "execution_backend = \"vm\"",
        "execution_backend = \"interpreter\"",
    );
    let err =
        validate_runtime_toml_text(&text).expect_err("interpreter backend should be rejected");
    assert!(err
        .to_string()
        .contains("runtime.execution_backend='interpreter' is no longer supported"));
}

#[test]
fn runtime_schema_requires_control_auth_for_tcp_endpoints() {
    let text = runtime_toml().replace(
        "endpoint = \"unix:///tmp/trust-runtime.sock\"",
        "endpoint = \"tcp://127.0.0.1:5000\"",
    );
    let err = validate_runtime_toml_text(&text).expect_err("tcp auth should fail");
    assert!(err
        .to_string()
        .contains("runtime.control.auth_token required for tcp endpoint"));
}

#[test]
fn runtime_schema_requires_deploy_keyring_when_signed_deploy_enabled() {
    let text = format!(
        "{}\n[runtime.deploy]\nrequire_signed = true\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("signed deploy config should fail");
    assert!(err
        .to_string()
        .contains("runtime.deploy.keyring_path required when runtime.deploy.require_signed=true"));
}

#[test]
fn runtime_schema_requires_tls_credentials_when_tls_enabled() {
    let text = format!(
        "{}\n[runtime.tls]\nmode = \"self-managed\"\n",
        runtime_toml().replace("tls = false", "tls = true")
    );
    let err = validate_runtime_toml_text(&text).expect_err("tls credential config should fail");
    assert!(err
        .to_string()
        .contains("runtime.tls.cert_path required when TLS is enabled"));
}

#[test]
fn runtime_schema_rejects_remote_web_without_tls_when_required() {
    let text = format!(
        "{}\n[runtime.tls]\nmode = \"disabled\"\nrequire_remote = true\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("remote tls policy should fail");
    assert!(err.to_string().contains(
            "runtime.web.tls must be true when runtime.tls.require_remote=true and runtime.web.listen is remote"
        ));
}

#[test]
fn runtime_schema_rejects_provisioned_tls_without_ca_path() {
    let text = format!(
            "{}\n[runtime.tls]\nmode = \"provisioned\"\ncert_path = \"certs/server.pem\"\nkey_path = \"certs/server.key\"\n",
            runtime_toml().replace("tls = false", "tls = true")
        );
    let err =
        validate_runtime_toml_text(&text).expect_err("provisioned tls without ca path should fail");
    assert!(err
        .to_string()
        .contains("runtime.tls.ca_path required when runtime.tls.mode='provisioned'"));
}

#[test]
fn runtime_schema_accepts_web_tls_with_self_managed_cert_paths() {
    let text = format!(
            "{}\n[runtime.tls]\nmode = \"self-managed\"\ncert_path = \"security/server-cert.pem\"\nkey_path = \"security/server-key.pem\"\n",
            runtime_toml().replace("tls = false", "tls = true")
        );
    validate_runtime_toml_text(&text).expect("web tls config should be valid");
}

#[test]
fn runtime_schema_rejects_unknown_runtime_cloud_profile() {
    let text = runtime_toml().replace("profile = \"dev\"", "profile = \"edge\"");
    let err = validate_runtime_toml_text(&text).expect_err("cloud profile should fail");
    assert!(err
        .to_string()
        .contains("invalid runtime.cloud.profile 'edge'"));
}

#[test]
fn runtime_schema_accepts_discovery_host_group_and_cloud_link_preferences() {
    let text = format!(
        "{}\n[runtime.cloud.links]\ntransports = [{{ source = \"runtime-a\", target = \"runtime-b\", transport = \"realtime\" }}]\n",
        runtime_toml().replace("interfaces = [\"eth0\"]", "interfaces = [\"eth0\"]\nhost_group = \"hq-vm-cluster\"")
    );
    validate_runtime_toml_text(&text)
        .expect("runtime.cloud.links transports + discovery host_group should be valid");
}

#[test]
fn runtime_schema_accepts_extended_cloud_link_transports() {
    let text = format!(
        "{}\n[runtime.cloud.links]\ntransports = [\
{{ source = \"runtime-a\", target = \"runtime-b\", transport = \"realtime\" }}, \
{{ source = \"runtime-a\", target = \"runtime-c\", transport = \"zenoh\" }}, \
{{ source = \"runtime-a\", target = \"runtime-d\", transport = \"mesh\" }}, \
{{ source = \"runtime-a\", target = \"runtime-e\", transport = \"mqtt\" }}, \
{{ source = \"runtime-a\", target = \"runtime-f\", transport = \"modbus-tcp\" }}, \
{{ source = \"runtime-a\", target = \"runtime-g\", transport = \"opcua\" }}, \
{{ source = \"runtime-a\", target = \"runtime-h\", transport = \"discovery\" }}, \
{{ source = \"runtime-a\", target = \"runtime-i\", transport = \"web\" }}\
]\n",
        runtime_toml()
    );
    validate_runtime_toml_text(&text).expect("all documented link transports should be valid");
}

#[test]
fn runtime_schema_accepts_preempt_rt_profile_section() {
    let text = format!(
        "{}\n[runtime.realtime]\nenabled = true\nrequire_preempt_rt_kernel = true\nlock_memory = true\nscheduler = \"fifo\"\npriority = 70\ncpu_affinity = [2]\nstrict = true\n",
        runtime_toml()
    );
    validate_runtime_toml_text(&text).expect("preempt rt runtime section should validate");
}

#[test]
fn runtime_schema_defaults_openot_telemetry_disabled() {
    let runtime = parse_runtime_toml_from_text(&runtime_toml(), "runtime.toml")
        .expect("runtime config should parse");
    assert!(!runtime.openot.enabled);
    assert_eq!(runtime.openot.capacity, 4096);
    assert_eq!(
        runtime.openot.fence_mode,
        crate::config::OpenOtTelemetryFenceMode::Fenced
    );
    assert_eq!(
        runtime.openot.source,
        crate::config::OpenOtTelemetrySource::Heartbeat
    );
    assert!(runtime.openot.producer_instance.is_none());
    assert!(runtime.openot.producer_instances.is_empty());
}

#[test]
fn runtime_schema_accepts_explicitly_disabled_openot_persistence() {
    let text = format!(
        "{}\n[runtime.openot]\n[runtime.openot.persistence]\nenabled = false\n",
        runtime_toml()
    );

    parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("disabled OpenOT persistence should parse without selecting a backend");
}

#[test]
fn runtime_schema_selects_sqlite_openot_persistence_from_toml() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\n[runtime.openot.persistence]\nenabled = true\nbackend = \"sqlite\"\n\
         \n[runtime.openot.persistence.sqlite]\npath = \"history/openot.sqlite3\"\n",
        runtime_toml()
    );

    parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("TOML backend='sqlite' should select a valid SQLite persistence config");
}

#[test]
fn runtime_schema_rejects_persistence_when_openot_carriage_is_disabled() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = false\n\
         \n[runtime.openot.persistence]\nenabled = true\nbackend = \"sqlite\"\n\
         \n[runtime.openot.persistence.sqlite]\npath = \"history/openot.sqlite3\"\n",
        runtime_toml()
    );

    let error = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect_err("persistence cannot consume a disabled carriage");
    assert!(
        error
            .to_string()
            .contains("requires runtime.openot.enabled=true"),
        "{error}"
    );
}

#[test]
fn runtime_schema_accepts_every_openot_persistence_backend_table() {
    for (backend, backend_table) in [
        (
            "postgresql",
            "connection_url_env = \"TRUST_OPENOT_DATABASE_URL\"\nschema = \"openot\"\ntls = \"require\"",
        ),
        (
            "timescaledb",
            "connection_url_env = \"TRUST_OPENOT_DATABASE_URL\"\nschema = \"openot\"\ntls = \"require\"",
        ),
        (
            "mysql",
            "connection_url_env = \"TRUST_OPENOT_DATABASE_URL\"\ndatabase = \"openot\"\ntls = \"require\"",
        ),
        (
            "sqlserver",
            "connection_url_env = \"TRUST_OPENOT_DATABASE_URL\"\nschema = \"openot\"\ntls = \"require\"",
        ),
        (
            "influxdb3",
            "host_env = \"TRUST_OPENOT_INFLUX_HOST\"\ntoken_env = \"TRUST_OPENOT_INFLUX_TOKEN\"\ndatabase = \"openot\"\nspool_path = \"history/openot-influx-spool.sqlite3\"\nmax_bytes = 1073741824",
        ),
    ] {
        let text = format!(
            "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\n[runtime.openot.persistence]\nenabled = true\nbackend = \"{backend}\"\n\
             \n[runtime.openot.persistence.{backend}]\n{backend_table}\n",
            runtime_toml()
        );

        parse_runtime_toml_from_text(&text, "runtime.toml")
            .unwrap_or_else(|error| panic!("backend={backend}: {error}"));
    }
}

#[test]
fn runtime_schema_requires_an_explicit_bounded_influx_spool_capacity() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\n[runtime.openot.persistence]\nenabled = true\nbackend = \"influxdb3\"\n\
         \n[runtime.openot.persistence.influxdb3]\nhost_env = \"TRUST_OPENOT_INFLUX_HOST\"\ntoken_env = \"TRUST_OPENOT_INFLUX_TOKEN\"\ndatabase = \"openot\"\nspool_path = \"history/openot-influx-spool.sqlite3\"\nmax_bytes = 1048576\n",
        runtime_toml()
    );

    parse_runtime_toml_from_text(&text, "runtime.toml").expect("bounded spool");
}

#[test]
fn runtime_schema_rejects_unselected_openot_persistence_backend_table() {
    let text = format!(
        "{}\n[runtime.openot]\n[runtime.openot.persistence]\nenabled = true\nbackend = \"sqlite\"\n\
         \n[runtime.openot.persistence.sqlite]\npath = \"history/openot.sqlite3\"\n\
         \n[runtime.openot.persistence.postgresql]\nconnection_url_env = \"TRUST_OPENOT_DATABASE_URL\"\nschema = \"openot\"\ntls = \"require\"\n",
        runtime_toml()
    );

    let error = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect_err("an unselected backend table must fail closed");
    assert!(
        error
            .to_string()
            .contains("unselected runtime.openot.persistence.postgresql"),
        "{error}"
    );
}

#[test]
fn runtime_schema_accepts_openot_persistence_operational_limits() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\n[runtime.openot.persistence]\nenabled = true\nbackend = \"sqlite\"\nbatch_size = 128\nflush_interval_ms = 500\nqueue_capacity = 2048\nshutdown_timeout_ms = 7000\nretry_initial_ms = 100\nretry_max_ms = 10000\nretry_multiplier = 3\nretry_max_attempts = 4\n\
         \n[runtime.openot.persistence.sqlite]\npath = \"history/openot.sqlite3\"\n",
        runtime_toml()
    );

    parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("valid OpenOT persistence operational limits should parse");
}

#[test]
fn openot_example_has_parseable_toml_for_every_supported_database_product() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example_root = repository_root.join("examples/openot_database");
    for (directory, expected_backend) in [
        ("sqlite", crate::config::OpenOtPersistenceBackend::Sqlite),
        (
            "postgresql",
            crate::config::OpenOtPersistenceBackend::PostgreSql,
        ),
        (
            "timescaledb",
            crate::config::OpenOtPersistenceBackend::TimescaleDb,
        ),
        ("mysql", crate::config::OpenOtPersistenceBackend::MySql),
        ("mariadb", crate::config::OpenOtPersistenceBackend::MySql),
        (
            "sqlserver",
            crate::config::OpenOtPersistenceBackend::SqlServer,
        ),
        (
            "influxdb3",
            crate::config::OpenOtPersistenceBackend::InfluxDb3,
        ),
    ] {
        let path = example_root.join(directory).join("runtime.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read runnable example '{}': {error}", path.display()));
        let config = parse_runtime_toml_from_text(&text, "runtime.toml")
            .unwrap_or_else(|error| panic!("parse runnable example '{directory}': {error}"));
        assert!(config.openot.persistence.enabled, "{directory}");
        assert_eq!(
            config.openot.persistence.backend,
            Some(expected_backend),
            "{directory}"
        );
        assert!(example_root.join(directory).join("README.md").is_file());
    }
    assert_eq!(
        std::fs::read(example_root.join("workload/Main.st")).expect("shared workload"),
        std::fs::read(repository_root.join("examples/openot_multi_program/src/Main.st"))
            .expect("canonical workload")
    );
    assert_eq!(
        std::fs::read(example_root.join("workload/openot-coverage-manifest.json"))
            .expect("shared coverage manifest"),
        std::fs::read(
            repository_root.join("examples/openot_multi_program/openot-coverage-manifest.json")
        )
        .expect("canonical coverage manifest")
    );
}

#[test]
fn runtime_schema_accepts_openot_remote_database_ca_certificate_path() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"history/openot.jsonl\"\n[runtime.openot.persistence]\nenabled = true\nbackend = \"postgresql\"\n\
         \n[runtime.openot.persistence.postgresql]\nconnection_url_env = \"TRUST_OPENOT_DATABASE_URL\"\nschema = \"openot\"\ntls = \"require\"\nca_cert_path = \"certs/openot-database-ca.pem\"\n",
        runtime_toml()
    );

    parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("remote database CA certificate path should parse");
}

#[test]
fn runtime_schema_accepts_openot_influxdb3_ca_certificate_path() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"history/openot.jsonl\"\n[runtime.openot.persistence]\nenabled = true\nbackend = \"influxdb3\"\n\
         \n[runtime.openot.persistence.influxdb3]\nhost_env = \"TRUST_OPENOT_INFLUX_HOST\"\ntoken_env = \"TRUST_OPENOT_INFLUX_TOKEN\"\ndatabase = \"openot\"\nspool_path = \"history/openot-influx-spool.sqlite3\"\nmax_bytes = 1073741824\nca_cert_path = \"certs/openot-influx-ca.pem\"\n",
        runtime_toml()
    );

    parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("InfluxDB 3 CA certificate path should parse");
}

#[test]
fn runtime_schema_rejects_inline_openot_database_credentials() {
    let text = format!(
        "{}\n[runtime.openot]\n[runtime.openot.persistence]\nenabled = true\nbackend = \"postgresql\"\n\
         \n[runtime.openot.persistence.postgresql]\nconnection_url_env = \"postgresql://logger:secret@db/openot\"\nschema = \"openot\"\ntls = \"require\"\n",
        runtime_toml()
    );

    let error = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect_err("connection_url_env must contain an environment variable name, not a URL");
    assert!(
        error
            .to_string()
            .contains("connection_url_env must be an environment variable name"),
        "{error}"
    );
}

#[test]
fn runtime_schema_rejects_unsafe_openot_database_identifier() {
    let text = format!(
        "{}\n[runtime.openot]\n[runtime.openot.persistence]\nenabled = true\nbackend = \"postgresql\"\n\
         \n[runtime.openot.persistence.postgresql]\nconnection_url_env = \"TRUST_OPENOT_DATABASE_URL\"\nschema = \"openot; DROP SCHEMA public\"\ntls = \"require\"\n",
        runtime_toml()
    );

    let error = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect_err("schema must be a plain SQL identifier");
    assert!(
        error
            .to_string()
            .contains("postgresql.schema must be a SQL identifier"),
        "{error}"
    );
}

#[test]
fn runtime_schema_accepts_openot_telemetry_section() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\ncapacity = 4096\nfence_mode = \"fenced\"\n",
        runtime_toml()
    );
    let runtime =
        parse_runtime_toml_from_text(&text, "runtime.toml").expect("OpenOT config should parse");
    assert!(runtime.openot.enabled);
    assert_eq!(runtime.openot.path, std::path::PathBuf::from("openot.shm"));
    assert_eq!(runtime.openot.capacity, 4096);
    assert_eq!(
        runtime.openot.fence_mode,
        crate::config::OpenOtTelemetryFenceMode::Fenced
    );
    assert_eq!(
        runtime.openot.source,
        crate::config::OpenOtTelemetrySource::Heartbeat
    );
}

#[test]
fn runtime_schema_accepts_openot_st_fb_source() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"st-fb\"\nproducer_instance = \"Main.Producer\"\n",
        runtime_toml()
    );
    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("OpenOT ST FB config should parse");
    assert_eq!(
        runtime.openot.source,
        crate::config::OpenOtTelemetrySource::StFb
    );
    assert_eq!(
        runtime.openot.producer_instance.as_deref(),
        Some("Main.Producer")
    );
    assert_eq!(
        runtime
            .openot
            .producer_instances
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>(),
        ["Main.Producer"]
    );
}

#[test]
fn runtime_schema_accepts_openot_st_fb_producer_instances() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"st-fb\"\nproducer_instances = [\"First.OotProducer\", \"Second.OotProducer\"]\n",
        runtime_toml()
    );
    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("OpenOT ST FB multi-producer config should parse");
    assert_eq!(
        runtime.openot.source,
        crate::config::OpenOtTelemetrySource::StFb
    );
    assert!(runtime.openot.producer_instance.is_none());
    assert_eq!(
        runtime
            .openot
            .producer_instances
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>(),
        ["First.OotProducer", "Second.OotProducer"]
    );
}

#[test]
fn runtime_schema_rejects_openot_st_fb_both_producer_aliases() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"st-fb\"\nproducer_instance = \"Main.Producer\"\nproducer_instances = [\"Main.Producer\"]\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text)
        .expect_err("OpenOT ST FB source should reject both producer aliases");
    assert!(err
        .to_string()
        .contains("producer_instance and runtime.openot.producer_instances are aliases"));
}

#[test]
fn runtime_schema_rejects_duplicate_openot_st_fb_producer_instances() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"st-fb\"\nproducer_instances = [\"Main.Producer\", \"Main.Producer\"]\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text)
        .expect_err("OpenOT ST FB source should reject duplicate producer paths");
    assert!(err.to_string().contains("duplicate path 'Main.Producer'"));
}

#[test]
fn runtime_schema_rejects_openot_st_fb_without_producer_instance() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"st-fb\"\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text)
        .expect_err("OpenOT ST FB source requires producer_instance");
    assert!(err.to_string().contains(
        "runtime.openot.producer_instance or runtime.openot.producer_instances is required"
    ));
}

#[test]
fn runtime_schema_rejects_unqualified_openot_producer_instance() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"st-fb\"\nproducer_instance = \"Producer\"\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text)
        .expect_err("OpenOT ST FB producer_instance must be qualified");
    assert!(err
        .to_string()
        .contains("runtime.openot.producer_instance(s) must be qualified paths"));
}

#[test]
fn runtime_schema_rejects_openot_producer_instance_for_heartbeat_source() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"heartbeat\"\nproducer_instance = \"Main.Producer\"\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text)
        .expect_err("OpenOT heartbeat source must not accept producer_instance");
    assert!(err
        .to_string()
        .contains("runtime.openot.producer_instance(s) are only valid"));
}

#[test]
fn runtime_schema_rejects_openot_producer_instances_for_heartbeat_source() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nsource = \"heartbeat\"\nproducer_instances = [\"Main.Producer\"]\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text)
        .expect_err("OpenOT heartbeat source must not accept producer_instances");
    assert!(err
        .to_string()
        .contains("runtime.openot.producer_instance(s) are only valid"));
}

#[test]
fn runtime_schema_rejects_enabled_openot_without_path() {
    let text = format!("{}\n[runtime.openot]\nenabled = true\n", runtime_toml());
    let err = validate_runtime_toml_text(&text).expect_err("enabled OpenOT requires a path");
    assert!(err
        .to_string()
        .contains("runtime.openot.path must not be empty"));
}

#[test]
fn runtime_schema_rejects_zero_openot_capacity() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\ncapacity = 0\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("OpenOT capacity zero should fail");
    assert!(err
        .to_string()
        .contains("runtime.openot.capacity must be >= 1"));
}

#[test]
fn runtime_schema_rejects_openot_unfenced_without_proof_opt_in() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nfence_mode = \"unfenced\"\n",
        runtime_toml()
    );
    let err =
        validate_runtime_toml_text(&text).expect_err("unfenced OpenOT requires explicit opt-in");
    assert!(err
        .to_string()
        .contains("runtime.openot.fence_mode='unfenced' requires"));
}

#[test]
fn runtime_schema_accepts_openot_unfenced_with_proof_opt_in() {
    let text = format!(
        "{}\n[runtime.openot]\nenabled = true\npath = \"openot.shm\"\nfence_mode = \"unfenced\"\nallow_unfenced_for_proof = true\n",
        runtime_toml()
    );
    let runtime =
        parse_runtime_toml_from_text(&text, "runtime.toml").expect("OpenOT config should parse");
    assert_eq!(
        runtime.openot.fence_mode,
        crate::config::OpenOtTelemetryFenceMode::Unfenced
    );
    assert!(runtime.openot.allow_unfenced_for_proof);
}

#[test]
fn runtime_schema_rejects_enabled_realtime_section_without_fifo_or_rr_scheduler() {
    let text = format!(
        "{}\n[runtime.realtime]\nenabled = true\nscheduler = \"other\"\npriority = 70\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("invalid realtime scheduler");
    assert!(err
        .to_string()
        .contains("runtime.realtime.scheduler must be 'fifo' or 'rr'"));
}

#[test]
fn runtime_schema_rejects_duplicate_realtime_cpu_affinity() {
    let text = format!(
        "{}\n[runtime.realtime]\nenabled = true\nscheduler = \"fifo\"\npriority = 70\ncpu_affinity = [2, 2]\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("duplicate cpu affinity");
    assert!(err
        .to_string()
        .contains("runtime.realtime.cpu_affinity entries must be unique"));
}

#[test]
fn runtime_schema_rejects_invalid_cloud_link_transport() {
    let text = format!(
        "{}\n[runtime.cloud.links]\ntransports = [{{ source = \"runtime-a\", target = \"runtime-b\", transport = \"udp\" }}]\n",
        runtime_toml()
    );
    let err =
        validate_runtime_toml_text(&text).expect_err("unsupported runtime cloud link transport");
    assert!(err
        .to_string()
        .contains("invalid runtime.cloud.links.transports[].transport 'udp'"));
}

#[test]
fn runtime_schema_rejects_empty_cloud_link_source() {
    let text = format!(
        "{}\n[runtime.cloud.links]\ntransports = [{{ source = \" \", target = \"runtime-b\", transport = \"zenoh\" }}]\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("empty cloud link source should fail");
    assert!(err
        .to_string()
        .contains("runtime.cloud.links.transports[].source must not be empty"));
}

#[test]
fn runtime_schema_rejects_empty_wan_allow_write_rule_target() {
    let text = runtime_toml().replace(
        "allow_write = []",
        "allow_write = [{ action = \"cfg_apply\", target = \"\" }]",
    );
    let err = validate_runtime_toml_text(&text).expect_err("wan allowlist target should fail");
    assert!(err
        .to_string()
        .contains("runtime.cloud.wan.allow_write[].target must not be empty"));
}

#[test]
fn runtime_schema_rejects_allowlist_without_patterns() {
    let text = format!(
        "{}\n[runtime.observability]\nmode = \"allowlist\"\ninclude = []\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("allowlist requires include");
    assert!(err
        .to_string()
        .contains("runtime.observability.include must not be empty when mode='allowlist'"));
}

#[test]
fn runtime_schema_rejects_prometheus_path_without_leading_slash() {
    let text = format!(
        "{}\n[runtime.observability]\nprometheus_path = \"metrics\"\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("prometheus path should fail");
    assert!(err
        .to_string()
        .contains("runtime.observability.prometheus_path must start with '/'"));
}

#[test]
fn runtime_schema_accepts_hmi_persistence_section() {
    let text = format!(
        "{}\n[runtime.hmi_persistence]\nenabled = true\nhistory_path = \"history/hmi.jsonl\"\nmax_entries = 128\n",
        runtime_toml()
    );
    let config = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("hmi persistence config should parse");
    assert!(config.hmi_persistence.enabled);
    assert_eq!(
        config.hmi_persistence.history_path,
        std::path::PathBuf::from("history/hmi.jsonl")
    );
    assert_eq!(config.hmi_persistence.max_entries, 128);
}

#[test]
fn runtime_schema_rejects_empty_hmi_persistence_window() {
    let text = format!(
        "{}\n[runtime.hmi_persistence]\nenabled = true\nmax_entries = 0\n",
        runtime_toml()
    );
    let err = validate_runtime_toml_text(&text).expect_err("max_entries should fail");
    assert!(err
        .to_string()
        .contains("runtime.hmi_persistence.max_entries must be >= 1"));
}

#[test]
fn runtime_schema_rejects_opcua_endpoint_path_without_leading_slash() {
    let text = format!(
            "{}\n[runtime.opcua]\nenabled = true\nallow_anonymous = true\nendpoint_path = \"interop\"\nsecurity_policy = \"none\"\nsecurity_mode = \"none\"\n",
            runtime_toml()
        );
    let err = validate_runtime_toml_text(&text).expect_err("opcua endpoint path should fail");
    assert!(err
        .to_string()
        .contains("runtime.opcua.endpoint_path must start with '/'"));
}

#[test]
fn runtime_schema_requires_opcua_credentials_or_anonymous_when_enabled() {
    let text = format!("{}\n[runtime.opcua]\nenabled = true\n", runtime_toml());
    let err = validate_runtime_toml_text(&text).expect_err("opcua auth config should fail");
    assert!(err
        .to_string()
        .contains("runtime.opcua requires anonymous access or username/password when enabled"));
}

#[test]
fn runtime_schema_defaults_ads_disabled() {
    let runtime = parse_runtime_toml_from_text(&runtime_toml(), "runtime.toml")
        .expect("runtime config should parse");

    assert!(!runtime.ads.enabled);
    assert_eq!(
        runtime.ads.config_path,
        std::path::PathBuf::from("ads.toml")
    );
    assert_eq!(
        runtime.ads.worker_tick_interval,
        crate::value::Duration::from_millis(20)
    );
    assert!(!runtime.ads_server.enabled);
    assert!(runtime.ads_server.listen.is_none());
    assert!(runtime.ads_server.expose.is_empty());
    assert!(runtime.ads_server.clients.is_empty());
}

#[test]
fn runtime_schema_accepts_ads_section() {
    let text = format!(
        "{}\n[runtime.ads]\nenabled = true\nconfig_path = \"config/line1.ads.toml\"\nworker_tick_interval_ms = 5\n",
        runtime_toml()
    );

    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("ADS runtime config should parse");

    assert!(runtime.ads.enabled);
    assert_eq!(
        runtime.ads.config_path,
        std::path::PathBuf::from("config/line1.ads.toml")
    );
    assert_eq!(
        runtime.ads.worker_tick_interval,
        crate::value::Duration::from_millis(5)
    );
}

#[test]
fn runtime_schema_rejects_zero_ads_worker_tick() {
    let text = format!(
        "{}\n[runtime.ads]\nenabled = true\nworker_tick_interval_ms = 0\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("ADS tick interval should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads.worker_tick_interval_ms must be >= 1"));
}

#[test]
fn runtime_schema_accepts_ads_server_section() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"192.168.10.20\"\ninsecure_transport = true\nexpose = [\"global.*\"]\nwritable = [\"global.setpoint\"]\nwrites_enabled = true\n[[runtime.ads_server.clients]]\nams_net_id = \"5.23.91.12.1.1\"\nsource_ip = \"192.168.10.50\"\n",
        runtime_toml()
    );

    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("ADS server config should parse");

    assert!(runtime.ads_server.enabled);
    assert_eq!(runtime.ads_server.listen.as_deref(), Some("192.168.10.20"));
    assert_eq!(
        runtime
            .ads_server
            .ams_net_id
            .as_ref()
            .map(|id| id.0.as_str()),
        Some("192.168.10.20.1.1")
    );
    assert_eq!(runtime.ads_server.ads_port, 851);
    assert!(runtime.ads_server.writes_enabled);
    assert_eq!(
        runtime.ads_server.expose,
        vec![smol_str::SmolStr::new("global.*")]
    );
    assert_eq!(
        runtime.ads_server.writable,
        vec![smol_str::SmolStr::new("global.setpoint")]
    );
    assert_eq!(runtime.ads_server.clients.len(), 1);
    assert!(matches!(
        runtime.ads_server.clients[0].source,
        crate::ads::server::AdsServerSourcePin::Ip(_)
    ));
}

#[test]
fn runtime_schema_accepts_ads_server_empty_fail_closed_runtime() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"192.168.10.20\"\ninsecure_transport = true\n",
        runtime_toml()
    );

    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("empty ADS server config starts fail-closed");

    assert!(runtime.ads_server.enabled);
    assert!(runtime.ads_server.expose.is_empty());
    assert!(runtime.ads_server.clients.is_empty());
}

#[test]
fn runtime_schema_rejects_ads_server_enabled_without_listen() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\ninsecure_transport = true\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("listen should be required");

    assert!(err
        .to_string()
        .contains("runtime.ads_server.enabled=true requires explicit listen IP"));
}

#[test]
fn runtime_schema_rejects_ads_server_wildcard_listen() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"0.0.0.0\"\ninsecure_transport = true\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("wildcard listen should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads_server.listen must not be 0.0.0.0 or ::"));
}

#[test]
fn runtime_schema_rejects_ads_server_plain_without_ack() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"192.168.10.20\"\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("plain ack should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads_server.insecure_transport=true is required"));
}

#[test]
fn runtime_schema_rejects_ads_server_writable_not_exposed() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"192.168.10.20\"\ninsecure_transport = true\nexpose = [\"global.ready\"]\nwritable = [\"global.setpoint\"]\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("writable subset should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads_server.writable entry 'global.setpoint' is not covered"));
}

#[test]
fn runtime_schema_rejects_ads_server_unpinned_bare_clients_by_default() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"192.168.10.20\"\ninsecure_transport = true\nallow_clients = [\"5.23.91.12.1.1\"]\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("bare clients should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads_server.allow_clients requires allow_unpinned_clients=true"));
}

#[test]
fn runtime_schema_accepts_ads_server_unpinned_clients_with_lab_override() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"127.0.0.1\"\ninsecure_transport = true\nallow_unpinned_clients = true\nallow_clients = [\"5.23.91.12.1.1\"]\n",
        runtime_toml()
    );

    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("unpinned lab clients should parse with override");

    assert!(runtime.ads_server.allow_unpinned_clients);
    assert!(matches!(
        runtime.ads_server.clients[0].source,
        crate::ads::server::AdsServerSourcePin::Unpinned
    ));
}

#[test]
fn runtime_schema_rejects_ads_server_structured_client_without_source_pin() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"192.168.10.20\"\ninsecure_transport = true\n[[runtime.ads_server.clients]]\nams_net_id = \"5.23.91.12.1.1\"\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("source pin should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads_server.clients[] requires source_ip or source_cidr"));
}

#[test]
fn runtime_schema_rejects_ads_server_public_bind_without_override() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"8.8.8.8\"\ninsecure_transport = true\n",
        runtime_toml()
    );

    let err = validate_runtime_toml_text(&text).expect_err("public bind should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads_server.listen is public/NAT-suspect"));
}

#[test]
fn runtime_schema_accepts_ads_server_source_cidr_client() {
    let text = format!(
        "{}\n[runtime.ads_server]\nenabled = true\nlisten = \"192.168.10.20\"\ninsecure_transport = true\n[[runtime.ads_server.clients]]\nams_net_id = \"5.23.91.12.1.1\"\nsource_cidr = \"192.168.10.0/24\"\n",
        runtime_toml()
    );

    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("source CIDR client should parse");

    assert!(matches!(
        runtime.ads_server.clients[0].source,
        crate::ads::server::AdsServerSourcePin::Cidr(_)
    ));
}

#[test]
fn runtime_schema_accepts_opcua_secure_profile_with_user_credentials() {
    let text = format!(
            "{}\n[runtime.opcua]\nenabled = true\nallow_anonymous = false\nsecurity_policy = \"basic256sha256\"\nsecurity_mode = \"sign_and_encrypt\"\nusername = \"operator\"\npassword = \"secret\"\n",
            runtime_toml()
        );
    validate_runtime_toml_text(&text).expect("opcua secure profile should be valid");
}

#[test]
fn runtime_schema_accepts_opcua_client_pointer() {
    let text = format!(
        "{}\n[runtime.opcua_client]\nenabled = true\nconfig_path = \"config/opcua_client.toml\"\npoll_interval_ms = 100\n",
        runtime_toml()
    );
    let runtime = parse_runtime_toml_from_text(&text, "runtime.toml")
        .expect("opcua client pointer should parse");

    assert!(runtime.opcua_client.enabled);
    assert_eq!(
        runtime.opcua_client.config_path,
        std::path::PathBuf::from("config/opcua_client.toml")
    );
    assert_eq!(
        runtime.opcua_client.poll_interval,
        crate::value::Duration::from_millis(100)
    );
}

#[test]
fn opcua_client_toml_accepts_connection_points() {
    validate_opcua_client_toml_text(opcua_client_toml())
        .expect("OPC UA client config should parse");
}

#[test]
fn runtime_bundle_loads_ads_toml_when_enabled() {
    let root = unique_temp_dir("trust-runtime-config-ads-bundle");
    std::fs::write(
        root.join("runtime.toml"),
        format!(
            "{}\n[runtime.ads]\nenabled = true\nconfig_path = \"ads.toml\"\n",
            runtime_toml()
        ),
    )
    .expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml()).expect("write io.toml");
    std::fs::write(root.join("program.stbc"), []).expect("write bytecode");
    std::fs::write(root.join("ads.toml"), ads_toml()).expect("write ads.toml");

    let bundle = RuntimeBundle::load(&root).expect("load runtime bundle");

    let ads = bundle.ads.expect("ADS config should be loaded");
    let expected_ads_hash = crate::ads::diagnostics::sha256_evidence_hash(ads_toml().as_bytes());
    assert_eq!(
        bundle.ads_config_hash.as_deref(),
        Some(expected_ads_hash.as_str())
    );
    assert_eq!(ads.connections.len(), 1);
    assert_eq!(ads.connections[0].route.name, "line1");
    assert_eq!(ads.connections[0].points[0].point_name, "line1_temp");
}

#[test]
fn runtime_bundle_loads_opcua_client_toml_when_enabled() {
    let root = unique_temp_dir("trust-runtime-config-opcua-client-bundle");
    std::fs::write(
        root.join("runtime.toml"),
        format!(
            "{}\n[runtime.opcua_client]\nenabled = true\nconfig_path = \"opcua_client.toml\"\n",
            runtime_toml()
        ),
    )
    .expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml()).expect("write io.toml");
    std::fs::write(root.join("program.stbc"), []).expect("write bytecode");
    std::fs::write(root.join("opcua_client.toml"), opcua_client_toml())
        .expect("write opcua_client.toml");

    let bundle = RuntimeBundle::load(&root).expect("load runtime bundle");

    let config = bundle
        .opcua_client
        .expect("OPC UA client config should be loaded");
    let expected_hash =
        crate::ads::diagnostics::sha256_evidence_hash(opcua_client_toml().as_bytes());
    assert_eq!(
        bundle.opcua_client_config_hash.as_deref(),
        Some(expected_hash.as_str())
    );
    assert_eq!(config.connections.len(), 1);
    assert_eq!(config.connections[0].name, "line1");
    assert_eq!(config.connections[0].points[0].var, "conveyor_speed");
}

#[test]
fn runtime_bundle_rejects_missing_enabled_opcua_client_toml() {
    let root = unique_temp_dir("trust-runtime-config-opcua-client-missing");
    std::fs::write(
        root.join("runtime.toml"),
        format!(
            "{}\n[runtime.opcua_client]\nenabled = true\n",
            runtime_toml()
        ),
    )
    .expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml()).expect("write io.toml");
    std::fs::write(root.join("program.stbc"), []).expect("write bytecode");

    let err = RuntimeBundle::load(&root).expect_err("missing OPC UA client config should fail");

    assert!(err
        .to_string()
        .contains("runtime.opcua_client.enabled=true but OPC UA client config is missing"));
}

#[test]
fn runtime_bundle_rejects_missing_enabled_ads_toml() {
    let root = unique_temp_dir("trust-runtime-config-ads-missing");
    std::fs::write(
        root.join("runtime.toml"),
        format!("{}\n[runtime.ads]\nenabled = true\n", runtime_toml()),
    )
    .expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml()).expect("write io.toml");
    std::fs::write(root.join("program.stbc"), []).expect("write bytecode");

    let err = RuntimeBundle::load(&root).expect_err("missing ADS config should fail");

    assert!(err
        .to_string()
        .contains("runtime.ads.enabled=true but ADS config is missing"));
}

fn ads_toml() -> &'static str {
    r#"
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
"#
}

fn opcua_client_toml() -> &'static str {
    r#"
[[connections]]
name = "line1"
endpoint_url = "opc.tcp://127.0.0.1:4840/trust"
security_policy = "none"
security_mode = "none"
auth = "anonymous"
trust_server_certificate = false

[[connections.points]]
var = "conveyor_speed"
node_id = "ns=2;s=MAIN.conveyor_speed"
type = "REAL"
access = "read"
"#
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{prefix}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    root
}

#[test]
fn io_schema_rejects_unknown_keys() {
    let text = io_toml().replace("params = {}", "params = {}\nunknown = true");
    let err = validate_io_toml_text(&text).expect_err("io schema should fail");
    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn io_schema_requires_table_params() {
    let text = io_toml().replace("params = {}", "params = 42");
    let err = validate_io_toml_text(&text).expect_err("io.params type should fail");
    assert!(err.to_string().contains("io.params must be a table"));
}

#[test]
fn io_schema_preserves_safe_state_integer_widths() {
    let maximums = r#"
[io]
driver = "loopback"
params = {}
safe_state = [
  { address = "%QB0", value = "255" },
  { address = "%QW0", value = "65535" },
  { address = "%QD0", value = "4294967295" },
  { address = "%QL0", value = "18446744073709551615" },
]
"#;
    let config = parse_io_toml_from_text(maximums, "io.toml")
        .expect("maximum safe-state integers must remain exact");
    let values = config
        .safe_state
        .outputs
        .iter()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![
            crate::value::Value::Byte(u8::MAX),
            crate::value::Value::Word(u16::MAX),
            crate::value::Value::DWord(u32::MAX),
            crate::value::Value::LWord(u64::MAX),
        ]
    );

    for (address, value) in [("%QB0", "256"), ("%QW0", "65536"), ("%QD0", "4294967296")] {
        let text = format!(
            "[io]\ndriver = \"loopback\"\nparams = {{}}\nsafe_state = [{{ address = \"{address}\", value = \"{value}\" }}]\n"
        );
        let error = validate_io_toml_text(&text)
            .expect_err("safe-state integers above the addressed width must reject");
        assert!(
            error.to_string().contains("out of range"),
            "unexpected error for {address}={value}: {error}"
        );
    }
}

#[test]
fn io_schema_accepts_multiple_drivers() {
    let text = r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "modbus-tcp"
params = { address = "127.0.0.1:502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" }

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883", topic_in = "trust/io/in", topic_out = "trust/io/out", reconnect_ms = 500, keep_alive_s = 5, allow_insecure_remote = false }
"#;
    validate_io_toml_text(text).expect("io.drivers profile should be valid");
}

#[test]
fn io_schema_accepts_disabled_multi_driver_entries() {
    let text = r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "modbus-tcp"
enabled = false
params = { address = "127.0.0.1:502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" }
"#;
    validate_io_toml_text(text).expect("disabled io.drivers entry should be valid");
}

#[test]
fn io_schema_accepts_driver_alias_inside_multi_driver_entries() {
    let text = r#"
[io]
safe_state = []

[[io.drivers]]
driver = "modbus-tcp"
params = { address = "127.0.0.1:502", unit_id = 1 }

[[io.drivers]]
driver = "ethercat"
params = { adapter = "eth0" }
"#;
    validate_io_toml_text(text).expect("io.drivers driver alias should be valid");
}

#[test]
fn io_schema_rejects_mixed_single_and_multi_driver_fields() {
    let driver_and_drivers = r#"
[io]
driver = "loopback"
params = {}

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883" }
"#;
    let params_and_drivers = r#"
[io]
params = { ignored = true }

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883" }
"#;
    let empty_driver_and_drivers = r#"
[io]
driver = ""

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883" }
"#;

    for text in [
        driver_and_drivers,
        params_and_drivers,
        empty_driver_and_drivers,
    ] {
        let err = validate_io_toml_text(text)
            .expect_err("any mixed single-driver and multi-driver fields must reject");
        assert!(err
            .to_string()
            .contains("use either io.driver/io.params or io.drivers"));
    }
}

#[test]
fn io_schema_rejects_empty_multi_driver_list() {
    let text = r#"
[io]
drivers = []
"#;
    let err = validate_io_toml_text(text).expect_err("empty io.drivers list should be rejected");
    assert!(err
        .to_string()
        .contains("io.driver or io.drivers must be set"));
}

#[test]
fn runtime_config_load_records_execution_backend_source_from_config() {
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-config-backend-source-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    let runtime_path = root.join("runtime.toml");
    std::fs::write(&runtime_path, runtime_toml()).expect("write runtime config");

    let runtime = RuntimeConfig::load(&runtime_path).expect("load runtime config");
    assert_eq!(
        runtime.execution_backend,
        crate::execution_backend::ExecutionBackend::BytecodeVm
    );
    assert_eq!(
        runtime.execution_backend_source,
        crate::execution_backend::ExecutionBackendSource::Config
    );
}

#[test]
fn runtime_config_load_defaults_execution_backend_source_when_omitted() {
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-config-backend-default-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    let runtime_path = root.join("runtime.toml");
    std::fs::write(
        &runtime_path,
        runtime_toml().replace("[runtime]\nexecution_backend = \"vm\"\n\n", ""),
    )
    .expect("write runtime config");

    let runtime = RuntimeConfig::load(&runtime_path).expect("load runtime config");
    assert_eq!(
        runtime.execution_backend,
        crate::execution_backend::ExecutionBackend::BytecodeVm
    );
    assert_eq!(
        runtime.execution_backend_source,
        crate::execution_backend::ExecutionBackendSource::Default
    );
}
