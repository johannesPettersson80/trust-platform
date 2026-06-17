use serde_json::json;

use super::CommFieldSchema;

fn field(
    id: &'static str,
    label: &'static str,
    field_type: &'static str,
    default: serde_json::Value,
    required: bool,
    help: &'static str,
) -> CommFieldSchema {
    CommFieldSchema {
        id,
        label,
        field_type,
        required,
        advanced: false,
        secret: false,
        help,
        default,
        validation: None,
        options: None,
    }
}

fn endpoint_field(
    id: &'static str,
    label: &'static str,
    default: &'static str,
    help: &'static str,
) -> CommFieldSchema {
    let mut field = field(id, label, "endpoint", json!(default), true, help);
    field.validation = Some(json!({ "kind": "host_port" }));
    field
}

fn socket_addr_field(
    id: &'static str,
    label: &'static str,
    default: &'static str,
    help: &'static str,
) -> CommFieldSchema {
    let mut field = field(id, label, "endpoint", json!(default), true, help);
    field.validation = Some(json!({ "kind": "socket_addr" }));
    field
}

fn number_field(
    id: &'static str,
    label: &'static str,
    default: i64,
    min: i64,
    max: i64,
    help: &'static str,
) -> CommFieldSchema {
    let mut field = field(id, label, "number", json!(default), true, help);
    field.validation = Some(json!({ "kind": "integer_range", "min": min, "max": max }));
    field
}

fn enum_field(
    id: &'static str,
    label: &'static str,
    default: &'static str,
    options: Vec<&'static str>,
    help: &'static str,
) -> CommFieldSchema {
    let mut field = field(id, label, "enum", json!(default), true, help);
    field.options = Some(options);
    field
}

fn boolean_field(
    id: &'static str,
    label: &'static str,
    default: bool,
    help: &'static str,
) -> CommFieldSchema {
    field(id, label, "boolean", json!(default), false, help)
}

fn advanced(mut field: CommFieldSchema) -> CommFieldSchema {
    field.advanced = true;
    field
}

fn secret(mut field: CommFieldSchema) -> CommFieldSchema {
    field.secret = true;
    field.default = serde_json::Value::Null;
    field
}

fn optional(mut field: CommFieldSchema) -> CommFieldSchema {
    field.required = false;
    field
}

fn string_array_field(
    id: &'static str,
    label: &'static str,
    default: serde_json::Value,
    help: &'static str,
) -> CommFieldSchema {
    field(id, label, "json_array", default, false, help)
}

fn json_object_field(
    id: &'static str,
    label: &'static str,
    default: serde_json::Value,
    help: &'static str,
) -> CommFieldSchema {
    field(id, label, "json_object", default, false, help)
}

pub(super) fn modbus_fields() -> Vec<CommFieldSchema> {
    vec![
        socket_addr_field(
            "address",
            "Device address",
            "127.0.0.1:502",
            "IP address and TCP port of the Modbus device.",
        ),
        number_field("unit_id", "Unit ID", 1, 0, 255, "Modbus unit/server id."),
        number_field(
            "input_start",
            "Input start register",
            0,
            0,
            65535,
            "First input register read by the runtime.",
        ),
        number_field(
            "output_start",
            "Output start register",
            0,
            0,
            65535,
            "First output register written by the runtime.",
        ),
        number_field(
            "timeout_ms",
            "Timeout",
            500,
            1,
            60000,
            "Request timeout in milliseconds.",
        ),
        enum_field(
            "on_error",
            "On error",
            "fault",
            vec!["fault", "warn", "ignore"],
            "Runtime behavior when the driver cannot read or write.",
        ),
    ]
}

pub(super) fn mqtt_fields() -> Vec<CommFieldSchema> {
    vec![
        endpoint_field(
            "broker",
            "Broker",
            "127.0.0.1:1883",
            "MQTT broker host and port.",
        ),
        optional(field(
            "client_id",
            "Client ID",
            "string",
            json!(""),
            false,
            "Optional MQTT client id. Empty lets the runtime choose one.",
        )),
        field(
            "topic_in",
            "Input topic",
            "string",
            json!("trust/io/in"),
            true,
            "Topic used for incoming process values.",
        ),
        field(
            "topic_out",
            "Output topic",
            "string",
            json!("trust/io/out"),
            true,
            "Topic used for outgoing process values.",
        ),
        optional(field(
            "username",
            "Username",
            "string",
            json!(""),
            false,
            "Broker username. Use together with password.",
        )),
        optional(secret(field(
            "password",
            "Password",
            "secret",
            serde_json::Value::Null,
            false,
            "Broker password. It is never returned by schema defaults.",
        ))),
        field(
            "tls",
            "TLS",
            "boolean",
            json!(false),
            false,
            "Use TLS for the MQTT connection.",
        ),
        field(
            "allow_insecure_remote",
            "Allow insecure remote broker",
            "boolean",
            json!(false),
            false,
            "Required when using a non-TLS broker that is not local.",
        ),
        number_field(
            "reconnect_ms",
            "Reconnect delay",
            500,
            1,
            60000,
            "Delay between reconnect attempts in milliseconds.",
        ),
        number_field(
            "keep_alive_s",
            "Keepalive",
            5,
            1,
            65535,
            "MQTT keepalive in seconds.",
        ),
        advanced(optional(field(
            "tls_ca_path",
            "TLS CA path",
            "path",
            json!(""),
            false,
            "CA certificate path when TLS is enabled.",
        ))),
        advanced(optional(field(
            "tls_client_cert_path",
            "Client cert path",
            "path",
            json!(""),
            false,
            "Client certificate path for mTLS.",
        ))),
        advanced(optional(field(
            "tls_client_key_path",
            "Client key path",
            "path",
            json!(""),
            false,
            "Client key path for mTLS.",
        ))),
        advanced(optional(field(
            "tls_alpn",
            "TLS ALPN",
            "json_array",
            json!([]),
            false,
            "Optional ALPN protocol list.",
        ))),
    ]
}

pub(super) fn ethercat_fields() -> Vec<CommFieldSchema> {
    vec![
        field(
            "adapter",
            "Adapter",
            "string",
            json!("mock"),
            true,
            "Network adapter name, or mock for simulation.",
        ),
        number_field(
            "timeout_ms",
            "Timeout",
            250,
            1,
            60000,
            "EtherCAT operation timeout.",
        ),
        number_field(
            "cycle_warn_ms",
            "Cycle warning",
            5,
            1,
            60000,
            "Warn when an EtherCAT cycle exceeds this duration.",
        ),
        enum_field(
            "on_error",
            "On error",
            "fault",
            vec!["fault", "warn", "ignore"],
            "Runtime behavior when EtherCAT reports an error.",
        ),
        advanced(field(
            "modules",
            "Modules",
            "json_array",
            json!([
                { "model": "EK1100", "slot": 0 },
                { "model": "EL1008", "slot": 1, "channels": 8 },
                { "model": "EL2008", "slot": 2, "channels": 8 }
            ]),
            false,
            "Expected EtherCAT module list.",
        )),
        advanced(field(
            "mock_inputs",
            "Mock input frames",
            "json_array",
            json!([]),
            false,
            "Optional hex frames used by the mock adapter.",
        )),
    ]
}

pub(super) fn gpio_fields() -> Vec<CommFieldSchema> {
    vec![
        enum_field(
            "backend",
            "Backend",
            "sysfs",
            vec!["sysfs"],
            "GPIO backend.",
        ),
        field(
            "sysfs_base",
            "Sysfs base",
            "path",
            json!("/sys/class/gpio"),
            true,
            "Linux GPIO sysfs root.",
        ),
        field(
            "inputs",
            "Inputs",
            "json_array",
            json!([{ "address": "%IX0.0", "line": 17 }]),
            false,
            "Input pin mappings.",
        ),
        field(
            "outputs",
            "Outputs",
            "json_array",
            json!([{ "address": "%QX0.0", "line": 27 }]),
            false,
            "Output pin mappings.",
        ),
    ]
}

pub(super) fn simulated_fields() -> Vec<CommFieldSchema> {
    vec![
        number_field(
            "input_count",
            "Input count",
            8,
            0,
            4096,
            "Number of simulated input points.",
        ),
        number_field(
            "output_count",
            "Output count",
            8,
            0,
            4096,
            "Number of simulated output points.",
        ),
        number_field(
            "scan_period_ms",
            "Scan period",
            10,
            1,
            60000,
            "Simulated driver update period in milliseconds.",
        ),
        enum_field(
            "mode",
            "Mode",
            "static",
            vec!["static", "counter", "random"],
            "How simulated input values are produced.",
        ),
    ]
}

pub(super) fn loopback_fields() -> Vec<CommFieldSchema> {
    vec![
        number_field(
            "input_count",
            "Input count",
            8,
            0,
            4096,
            "Number of loopback input points.",
        ),
        number_field(
            "output_count",
            "Output count",
            8,
            0,
            4096,
            "Number of loopback output points.",
        ),
        number_field(
            "scan_period_ms",
            "Scan period",
            10,
            1,
            60000,
            "Loopback driver update period in milliseconds.",
        ),
        enum_field(
            "mode",
            "Mode",
            "mirror",
            vec!["mirror", "hold_last"],
            "How outputs are reflected into inputs.",
        ),
    ]
}

pub(super) fn opcua_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field("enabled", "Enable OPC UA", true, "Enable the OPC UA server."),
        endpoint_field(
            "listen",
            "Listen address",
            "0.0.0.0:4840",
            "Host and port where OPC UA clients connect.",
        ),
        field(
            "endpoint_path",
            "Endpoint path",
            "string",
            json!("/"),
            true,
            "URL path advertised to OPC UA clients. It must start with '/'.",
        ),
        field(
            "namespace_uri",
            "Namespace URI",
            "string",
            json!("urn:trust:runtime"),
            true,
            "OPC UA namespace URI for exported truST variables.",
        ),
        number_field(
            "publish_interval_ms",
            "Publish interval",
            250,
            1,
            60000,
            "Default subscription publish interval in milliseconds.",
        ),
        number_field("max_nodes", "Max nodes", 128, 1, 100000, "Maximum exposed nodes."),
        string_array_field(
            "expose",
            "Expose globals",
            json!(["global.*"]),
            "Glob patterns for truST globals to expose. Use the runtime symbol picker when available.",
        ),
        enum_field(
            "security_policy",
            "Security policy",
            "basic256sha256",
            vec!["basic256sha256", "aes128sha256rsaoaep", "none"],
            "OPC UA security policy.",
        ),
        enum_field(
            "security_mode",
            "Security mode",
            "sign_and_encrypt",
            vec!["sign_and_encrypt", "sign", "none"],
            "OPC UA message security mode.",
        ),
        boolean_field(
            "allow_anonymous",
            "Allow anonymous",
            true,
            "Allow clients without username/password. Disable this when adding username/password.",
        ),
        optional(field(
            "username",
            "Username",
            "string",
            json!(""),
            false,
            "Username for authenticated OPC UA clients.",
        )),
        optional(secret(field(
            "password",
            "Password",
            "secret",
            serde_json::Value::Null,
            false,
            "Password for authenticated OPC UA clients. It is never returned by schema defaults.",
        ))),
    ]
}

pub(super) fn openot_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field(
            "enabled",
            "Enable OpenOT",
            true,
            "Enable OpenOT telemetry output.",
        ),
        field(
            "path",
            "Shared memory path",
            "path",
            json!("openot.shm"),
            true,
            "OpenOT shared-memory path.",
        ),
        number_field("capacity", "Capacity", 4096, 1, 1_000_000, "Ring capacity."),
        enum_field(
            "fence_mode",
            "Fence mode",
            "fenced",
            vec!["fenced", "unfenced"],
            "Use fenced mode unless explicitly proving unfenced behavior.",
        ),
        boolean_field(
            "allow_unfenced_for_proof",
            "Allow unfenced proof",
            false,
            "Required when fence mode is unfenced.",
        ),
        enum_field(
            "source",
            "Source",
            "heartbeat",
            vec!["heartbeat", "st-fb"],
            "Heartbeat emits runtime heartbeat records; ST-FB drains producer FB instances.",
        ),
        string_array_field(
            "producer_instances",
            "Producer instances",
            json!([]),
            "Qualified producer FB paths such as Main.Producer when source is st-fb.",
        ),
    ]
}

pub(super) fn discovery_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field(
            "enabled",
            "Enable discovery",
            true,
            "Enable mDNS runtime discovery.",
        ),
        field(
            "service_name",
            "Service name",
            "string",
            json!("truST"),
            true,
            "Human-readable service name advertised on the network.",
        ),
        boolean_field("advertise", "Advertise", true, "Advertise this runtime."),
        string_array_field(
            "interfaces",
            "Interfaces",
            json!([]),
            "Optional network interface names. Empty lets the runtime choose.",
        ),
        optional(field(
            "host_group",
            "Host group",
            "string",
            json!(""),
            false,
            "Optional grouping label for runtime cloud/fleet views.",
        )),
    ]
}

pub(super) fn mesh_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field(
            "enabled",
            "Enable mesh",
            true,
            "Enable Mesh / Zenoh transport.",
        ),
        enum_field(
            "role",
            "Role",
            "peer",
            vec!["peer", "client", "router"],
            "Mesh role for this runtime.",
        ),
        endpoint_field(
            "listen",
            "Listen address",
            "0.0.0.0:5200",
            "Mesh listen address.",
        ),
        string_array_field(
            "connect",
            "Connect to",
            json!([]),
            "Peer/router endpoints this runtime should connect to.",
        ),
        boolean_field("tls", "TLS", false, "Use TLS for mesh transport."),
        optional(secret(field(
            "auth_token",
            "Auth token",
            "secret",
            serde_json::Value::Null,
            false,
            "Optional mesh auth token. It is never returned by schema defaults.",
        ))),
        string_array_field(
            "publish",
            "Publish topics",
            json!([]),
            "Topic patterns this runtime publishes.",
        ),
        json_object_field(
            "subscribe",
            "Subscribe mappings",
            json!({}),
            "Mapping of remote topic to local variable path.",
        ),
        advanced(field(
            "zenohd_version",
            "Zenohd version",
            "string",
            json!("1.7.2"),
            true,
            "Expected zenohd version for managed deployments.",
        )),
        advanced(json_object_field(
            "plugin_versions",
            "Plugin versions",
            json!({}),
            "Optional plugin version pins.",
        )),
    ]
}

pub(super) fn realtime_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field(
            "enabled",
            "Enable realtime posture",
            true,
            "Enable realtime host checks.",
        ),
        boolean_field(
            "require_preempt_rt_kernel",
            "Require PREEMPT_RT",
            false,
            "Require a PREEMPT_RT kernel.",
        ),
        boolean_field(
            "lock_memory",
            "Lock memory",
            false,
            "Lock runtime memory at startup.",
        ),
        enum_field(
            "scheduler",
            "Scheduler",
            "fifo",
            vec!["fifo", "rr", "other"],
            "Linux scheduler policy requested for realtime operation.",
        ),
        number_field(
            "priority",
            "Priority",
            70,
            0,
            99,
            "Realtime scheduler priority.",
        ),
        string_array_field(
            "cpu_affinity",
            "CPU affinity",
            json!([]),
            "CPU indexes to pin. Use numbers, for example [2, 3].",
        ),
        boolean_field(
            "strict",
            "Strict",
            false,
            "Treat warnings as startup failures.",
        ),
    ]
}

pub(super) fn runtime_cloud_fields() -> Vec<CommFieldSchema> {
    vec![
        enum_field(
            "profile",
            "Profile",
            "dev",
            vec!["dev", "plant", "wan"],
            "Federation policy profile.",
        ),
        string_array_field(
            "wan_allow_write",
            "WAN write allow rules",
            json!([]),
            "Rules like [{\"action\":\"deploy\",\"target\":\"line-a\"}].",
        ),
        string_array_field(
            "link_transports",
            "Preferred link transports",
            json!([]),
            "Rules like [{\"source\":\"a\",\"target\":\"b\",\"transport\":\"zenoh\"}].",
        ),
    ]
}
