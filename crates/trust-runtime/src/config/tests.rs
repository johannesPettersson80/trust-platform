use super::{validate_io_toml_text, validate_runtime_toml_text, RuntimeConfig};

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
fn runtime_schema_accepts_opcua_secure_profile_with_user_credentials() {
    let text = format!(
            "{}\n[runtime.opcua]\nenabled = true\nallow_anonymous = false\nsecurity_policy = \"basic256sha256\"\nsecurity_mode = \"sign_and_encrypt\"\nusername = \"operator\"\npassword = \"secret\"\n",
            runtime_toml()
        );
    validate_runtime_toml_text(&text).expect("opcua secure profile should be valid");
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
fn io_schema_rejects_mixed_single_and_multi_driver_fields() {
    let text = r#"
[io]
driver = "loopback"
params = {}

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883" }
"#;
    let err =
        validate_io_toml_text(text).expect_err("mixed io.driver and io.drivers should be rejected");
    assert!(err
        .to_string()
        .contains("use either io.driver/io.params or io.drivers"));
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
