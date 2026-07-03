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
            "Broker password. Existing saved passwords are not shown here.",
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
            "Timeout (ms)",
            250,
            1,
            60000,
            "Maximum time to wait for an EtherCAT operation.",
        ),
        number_field(
            "cycle_warn_ms",
            "Cycle warning (ms)",
            5,
            1,
            60000,
            "Warn when an EtherCAT cycle takes longer than this.",
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
        advanced(string_array_field(
            "selected_channels",
            "Selected channels",
            json!([]),
            "PDO channel paths selected from Browse channels.",
        )),
    ]
}

pub(super) fn gpio_fields() -> Vec<CommFieldSchema> {
    vec![
        enum_field(
            "backend",
            "Backend",
            "libgpiod",
            vec!["libgpiod", "sysfs"],
            "Linux GPIO access backend. libgpiod uses the kernel GPIO character device; sysfs is available for legacy hosts.",
        ),
        field(
            "chip",
            "GPIO chip",
            "path",
            json!("/dev/gpiochip0"),
            false,
            "Linux GPIO character device used by libgpiod, usually /dev/gpiochip0 on Raspberry Pi.",
        ),
        field(
            "sysfs_base",
            "Sysfs base",
            "path",
            json!("/sys/class/gpio"),
            false,
            "Legacy sysfs root, usually /sys/class/gpio. Only used when Backend is sysfs.",
        ),
        field(
            "inputs",
            "Inputs",
            "json_array",
            json!([{ "address": "%IX0.0", "line": 17 }]),
            false,
            "Input mappings from IEC address to kernel/BCM GPIO line number, e.g. line 17 = GPIO17.",
        ),
        field(
            "outputs",
            "Outputs",
            "json_array",
            json!([{ "address": "%QX0.0", "line": 27 }]),
            false,
            "Output mappings from IEC address to kernel/BCM GPIO line number, e.g. line 27 = GPIO27.",
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
        boolean_field(
            "enabled",
            "Enable OPC UA server",
            true,
            "Expose selected truST globals over OPC UA.",
        ),
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
        number_field(
            "max_nodes",
            "Max nodes",
            128,
            1,
            100000,
            "Maximum exposed nodes.",
        ),
        string_array_field(
            "expose",
            "Exposed globals",
            json!(["global.*"]),
            "Choose project globals to expose, or add a pattern such as global.*.",
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
            "Password for authenticated OPC UA clients. Existing saved passwords are not shown here.",
        ))),
    ]
}

pub(super) fn opcua_client_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field(
            "enabled",
            "Enable OPC UA client",
            true,
            "Read selected nodes from an external OPC UA server.",
        ),
        optional(field(
            "config_path",
            "OPC UA client config path",
            "path",
            json!("opcua_client.toml"),
            false,
            "Project-relative OPC UA client connection file.",
        )),
        number_field(
            "poll_interval_ms",
            "OPC UA poll interval (ms)",
            250,
            10,
            60000,
            "How often truST reads selected OPC UA nodes. Lower values reduce latency but use more CPU and network traffic.",
        ),
        field(
            "connections",
            "Connections",
            "json_array",
            json!([]),
            false,
            "OPC UA client connection entries. Enabling the client requires at least one connection with selected nodes.",
        ),
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
            "Evidence file",
            "path",
            json!("openot.shm"),
            true,
            "Local file used by the OpenOT collector.",
        ),
        number_field(
            "capacity",
            "Record capacity",
            4096,
            1,
            1_000_000,
            "Maximum records kept before older entries are overwritten.",
        ),
        enum_field(
            "fence_mode",
            "Safety mode",
            "fenced",
            vec!["fenced", "unfenced"],
            "Keep fenced unless an OpenOT test explicitly requires unfenced mode.",
        ),
        boolean_field(
            "allow_unfenced_for_proof",
            "Allow unfenced mode",
            false,
            "Only enable when an OpenOT test requires unfenced mode.",
        ),
        enum_field(
            "source",
            "Telemetry source",
            "heartbeat",
            vec!["heartbeat", "st-fb"],
            "Heartbeat publishes runtime status; ST producer publishes values from selected ST blocks.",
        ),
        string_array_field(
            "producer_instances",
            "ST producer blocks",
            json!([]),
            "ST block paths such as Main.Producer when the telemetry source is ST producer.",
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
            "Peer addresses",
            json!([]),
            "Runtime or router addresses this runtime should connect to.",
        ),
        boolean_field("tls", "TLS", false, "Use TLS for mesh transport."),
        optional(secret(field(
            "auth_token",
            "Mesh token",
            "secret",
            serde_json::Value::Null,
            false,
            "Optional shared token for peers that require mesh authentication.",
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
            "Enable host checks",
            true,
            "Check whether the selected host can meet the realtime requirements below.",
        ),
        boolean_field(
            "require_preempt_rt_kernel",
            "Require real-time kernel",
            false,
            "Require a Linux kernel with PREEMPT_RT enabled before realtime startup.",
        ),
        boolean_field(
            "lock_memory",
            "Lock memory",
            false,
            "Ask the runtime to lock memory at startup to avoid paging delays.",
        ),
        enum_field(
            "scheduler",
            "Scheduler policy",
            "fifo",
            vec!["fifo", "rr", "other"],
            "Linux scheduler policy requested for realtime operation.",
        ),
        number_field(
            "priority",
            "Scheduler priority",
            70,
            0,
            99,
            "Realtime scheduler priority.",
        ),
        string_array_field(
            "cpu_affinity",
            "CPU affinity",
            json!([]),
            "CPU indexes to reserve for realtime work.",
        ),
        boolean_field(
            "strict",
            "Fail startup on warning",
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
            "Federation policy profile for this runtime.",
        ),
        string_array_field(
            "wan_allow_write",
            "Allowed WAN writes",
            json!([]),
            "Optional allow-list entries for write actions across sites.",
        ),
        string_array_field(
            "link_transports",
            "Preferred link transports",
            json!([]),
            "Optional source, target, and transport preferences for runtime-to-runtime links.",
        ),
    ]
}

pub(super) fn ads_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field("enabled", "Enable ADS client", true, "Enable ADS client polling."),
        optional(field(
            "config_path",
            "ADS config path",
            "path",
            json!("ads.toml"),
            false,
            "Project-relative ADS connection file.",
        )),
        number_field(
            "worker_tick_interval_ms",
            "ADS link update interval (ms)",
            20,
            1,
            60000,
            "How often truST services ADS reads, writes, reconnects, and status updates. Lower values reduce latency but use more CPU.",
        ),
        field(
            "connections",
            "Connections",
            "json_array",
            json!([]),
            false,
            "ADS connection entries. Enabling ADS requires at least one connection with at least one selected point.",
        ),
    ]
}

pub(super) fn ads_server_fields() -> Vec<CommFieldSchema> {
    vec![
        boolean_field(
            "enabled",
            "Enable ADS server",
            true,
            "Expose selected truST globals over ADS.",
        ),
        field(
            "listen",
            "Listen IP",
            "string",
            json!("127.0.0.1"),
            true,
            "Local IP address where TwinCAT/ADS clients connect. Wildcard binds are rejected.",
        ),
        optional(field(
            "ams_net_id",
            "AMS Net ID",
            "string",
            json!(""),
            false,
            "Runtime AMS Net ID. Empty derives it from an IPv4 listen address.",
        )),
        number_field("ads_port", "ADS port", 851, 1, 65535, "Logical ADS port."),
        boolean_field(
            "insecure_transport",
            "Plain ADS transport",
            true,
            "Required acknowledgement for plain ADS server transport.",
        ),
        boolean_field(
            "writes_enabled",
            "Enable writes",
            false,
            "Allow writes only for symbols also listed in writable.",
        ),
        string_array_field(
            "expose",
            "Exposed globals",
            json!(["global.*"]),
            "Choose project globals to expose, or add a pattern such as global.*.",
        ),
        string_array_field(
            "writable",
            "Writable globals",
            json!([]),
            "Subset of exposed globals that ADS clients may write.",
        ),
        boolean_field(
            "allow_unpinned_clients",
            "Allow unpinned clients",
            false,
            "Lab override for clients without source IP/CIDR pins. Keep disabled in production.",
        ),
        field(
            "clients",
            "Allowed clients",
            "json_array",
            json!([]),
            false,
            "Allowed ADS clients as objects with ams_net_id plus source_ip or source_cidr.",
        ),
        advanced(number_field(
            "max_symbols",
            "Max symbols",
            4096,
            1,
            1_000_000,
            "Maximum exposed symbol count.",
        )),
        advanced(number_field(
            "max_clients",
            "Max clients",
            32,
            1,
            10000,
            "Maximum simultaneous ADS clients.",
        )),
        advanced(number_field(
            "max_subscriptions_per_client",
            "Max subscriptions per client",
            1024,
            1,
            1_000_000,
            "Per-client notification subscription cap.",
        )),
        advanced(number_field(
            "max_total_subscriptions",
            "Max total subscriptions",
            8192,
            1,
            10_000_000,
            "Global notification subscription cap.",
        )),
        advanced(number_field(
            "max_frame_bytes",
            "Max frame bytes",
            1_048_576,
            1024,
            64 * 1024 * 1024,
            "Maximum ADS frame payload bytes.",
        )),
        advanced(number_field(
            "max_sumup_items",
            "Max sum-up items",
            256,
            1,
            65535,
            "Maximum items in ADS sum-up requests.",
        )),
        advanced(number_field(
            "max_write_bytes",
            "Max write bytes",
            262_144,
            1,
            64 * 1024 * 1024,
            "Maximum write payload bytes.",
        )),
    ]
}
