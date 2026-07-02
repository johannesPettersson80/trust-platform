// ide-settings.js - Settings tab: categorized runtime settings form and
// runtime.toml direct editor. Implements US-7.1 and US-7.2.

// -- Constants ---------------------------------------------------------------

const SETTINGS_RUNTIME_LINK_TRANSPORTS = Object.freeze([
  "realtime",
  "zenoh",
  "mesh",
  "mqtt",
  "modbus-tcp",
  "opcua",
  "discovery",
  "web",
]);

const SETTINGS_CATEGORIES = [
  {
    id: "all",
    label: "All Settings",
    fields: [],
  },
  {
    id: "general",
    label: "General",
    fields: [
      { key: "resource.name", label: "Resource Name", type: "text", default: "trust-runtime" },
      { key: "log.level", label: "Log Level", type: "select", options: ["error", "warn", "info", "debug", "trace"], default: "info" },
      { key: "control.endpoint", label: "Control Endpoint", type: "text", default: "unix:///tmp/trust-runtime.sock" },
      { key: "control.mode", label: "Control Mode", type: "select", options: ["production", "debug"], default: "production" },
      { key: "control.debug_enabled", label: "Debug Enabled", type: "toggle", default: false },
    ],
  },
  {
    id: "execution",
    label: "Execution",
    fields: [
      { key: "resource.cycle_interval_ms", label: "Cycle Time (ms)", type: "number", default: 20, min: 1, max: 60000 },
      { key: "resource.tasks_json", label: "Resource Tasks (JSON)", type: "json", default: "[]" },
      { key: "watchdog.enabled", label: "Watchdog Enabled", type: "toggle", default: false },
      { key: "watchdog.timeout_ms", label: "Watchdog Timeout (ms)", type: "number", default: 5000, min: 1, max: 60000 },
      { key: "watchdog.action", label: "Watchdog Action", type: "select", options: ["halt", "safe_halt", "restart"], default: "halt" },
      { key: "fault.policy", label: "Fault Policy", type: "select", options: ["halt", "safe_halt", "restart"], default: "halt" },
    ],
  },
  {
    id: "retention",
    label: "Retention",
    fields: [
      { key: "retain.mode", label: "Retain Mode", type: "select", options: ["none", "file"], default: "none" },
      { key: "retain.path", label: "Retain File Path", type: "text", default: "retain.dat" },
      { key: "retain.save_interval_ms", label: "Retain Save Interval (ms)", type: "number", default: 1000, min: 1, max: 120000 },
    ],
  },
  {
    id: "communication",
    label: "Communication",
    fields: [
      { key: "web.enabled", label: "Web API Enabled", type: "toggle", default: true },
      { key: "web.listen", label: "Web Listen Address", type: "text", default: "0.0.0.0:8080" },
      { key: "discovery.enabled", label: "Discovery Enabled", type: "toggle", default: true },
      { key: "discovery.service_name", label: "Discovery Service Name", type: "text", default: "trust-runtime" },
      { key: "discovery.advertise", label: "Discovery Advertise", type: "toggle", default: true },
      { key: "discovery.host_group", label: "Discovery Host Group", type: "text", default: "" },
      { key: "discovery.interfaces_json", label: "Discovery Interfaces (JSON)", type: "json", default: "[]" },
      { key: "mesh.enabled", label: "Mesh Enabled", type: "toggle", default: false },
      { key: "mesh.role", label: "Mesh Role", type: "select", options: ["peer", "client", "router"], default: "peer" },
      { key: "mesh.listen", label: "Mesh Listen Address", type: "text", default: "0.0.0.0:5200" },
      { key: "mesh.tls", label: "Mesh TLS", type: "toggle", default: false },
      { key: "mesh.auth_token", label: "Mesh Auth Token", type: "text", default: "" },
      { key: "mesh.connect_json", label: "Mesh Connect Peers (JSON)", type: "json", default: "[]" },
      { key: "mesh.publish_json", label: "Mesh Publish Topics (JSON)", type: "json", default: "[]" },
      { key: "mesh.subscribe_json", label: "Mesh Subscribe Map (JSON)", type: "json", default: "{}" },
      { key: "mesh.plugin_versions_json", label: "Mesh Plugin Versions (JSON)", type: "json", default: "{}" },
      { key: "mesh.zenohd_version", label: "Zenohd Version", type: "text", default: "1.7.2" },
      { key: "runtime_cloud.profile", label: "Runtime Cloud Profile", type: "select", options: ["dev", "plant", "wan"], default: "dev" },
      {
        key: "runtime_cloud.wan.allow_write_json",
        label: "Runtime Cloud WAN Allow Write Rules (JSON)",
        type: "json",
        default: "[]",
        hint: "Format: [{\"action\":\"deploy\",\"target\":\"line/plc-a\"}]",
      },
      {
        key: "runtime_cloud.links.transports_json",
        label: "Runtime Link Transport Rules (JSON)",
        type: "json",
        default: "[]",
        hint: "Format: [{\"source\":\"line/plc-a\",\"target\":\"line/plc-b\",\"transport\":\"realtime\"}] (transport: realtime|zenoh|mesh|mqtt|modbus-tcp|opcua|discovery|web)",
      },
      { key: "opcua.enabled", label: "OPC UA Enabled", type: "toggle", default: false },
      { key: "opcua.listen", label: "OPC UA Listen Address", type: "text", default: "0.0.0.0:4840" },
      { key: "opcua.endpoint_path", label: "OPC UA Endpoint Path", type: "text", default: "/" },
      { key: "opcua.namespace_uri", label: "OPC UA Namespace URI", type: "text", default: "urn:trust:runtime" },
      { key: "opcua.publish_interval_ms", label: "OPC UA Publish Interval (ms)", type: "number", default: 250, min: 20, max: 60000 },
      { key: "opcua.max_nodes", label: "OPC UA Max Nodes", type: "number", default: 128, min: 1, max: 16384 },
      { key: "opcua.expose_json", label: "OPC UA Expose Allowlist (JSON)", type: "json", default: "[]" },
      { key: "opcua.security_policy", label: "OPC UA Security Policy", type: "select", options: ["none", "basic256sha256", "aes128sha256rsaoaep"], default: "basic256sha256" },
      { key: "opcua.security_mode", label: "OPC UA Security Mode", type: "select", options: ["none", "sign", "sign_and_encrypt"], default: "sign_and_encrypt" },
      { key: "opcua.allow_anonymous", label: "OPC UA Allow Anonymous", type: "toggle", default: false },
      { key: "opcua.username", label: "OPC UA Username", type: "text", default: "" },
      { key: "opcua.password", label: "OPC UA Password", type: "password", default: "" },
      { key: "io.mqtt.broker", label: "MQTT Broker", type: "text", default: "127.0.0.1:1883" },
      { key: "io.mqtt.client_id", label: "MQTT Client ID", type: "text", default: "" },
      { key: "io.mqtt.topic_in", label: "MQTT Topic In", type: "text", default: "trust/io/in" },
      { key: "io.mqtt.topic_out", label: "MQTT Topic Out", type: "text", default: "trust/io/out" },
      { key: "io.mqtt.username", label: "MQTT Username", type: "text", default: "" },
      { key: "io.mqtt.password", label: "MQTT Password", type: "password", default: "" },
      { key: "io.mqtt.tls", label: "MQTT TLS", type: "toggle", default: false },
      { key: "io.mqtt.keep_alive_s", label: "MQTT Keep Alive (s)", type: "number", default: 5, min: 1, max: 600 },
      { key: "io.mqtt.reconnect_ms", label: "MQTT Reconnect (ms)", type: "number", default: 500, min: 10, max: 60000 },
      { key: "io.mqtt.allow_insecure_remote", label: "MQTT Allow Insecure Remote", type: "toggle", default: false },
      { key: "io.modbus.address", label: "Modbus Address", type: "text", default: "127.0.0.1:502" },
      { key: "io.modbus.unit_id", label: "Modbus Unit ID", type: "number", default: 1, min: 0, max: 255 },
      { key: "io.modbus.input_start", label: "Modbus Input Start", type: "number", default: 0, min: 0, max: 65535 },
      { key: "io.modbus.output_start", label: "Modbus Output Start", type: "number", default: 0, min: 0, max: 65535 },
      { key: "io.modbus.timeout_ms", label: "Modbus Timeout (ms)", type: "number", default: 500, min: 10, max: 60000 },
      { key: "io.modbus.on_error", label: "Modbus On Error", type: "select", options: ["fault", "warn", "ignore"], default: "fault" },
      { key: "io.gpio.backend", label: "GPIO Backend", type: "select", options: ["libgpiod", "sysfs"], default: "libgpiod" },
      { key: "io.gpio.chip", label: "GPIO Chip", type: "text", default: "/dev/gpiochip0" },
      { key: "io.gpio.sysfs_base", label: "GPIO Sysfs Base", type: "text", default: "/sys/class/gpio" },
      { key: "io.gpio.inputs_json", label: "GPIO Inputs (JSON)", type: "json", default: "[]" },
      { key: "io.gpio.outputs_json", label: "GPIO Outputs (JSON)", type: "json", default: "[]" },
      { key: "io.simulated.inputs", label: "Simulated Inputs", type: "number", default: 8, min: 0, max: 4096 },
      { key: "io.simulated.outputs", label: "Simulated Outputs", type: "number", default: 8, min: 0, max: 4096 },
      { key: "io.simulated.scan_ms", label: "Simulated Scan (ms)", type: "number", default: 20, min: 1, max: 600000 },
      { key: "io.ethercat.adapter", label: "EtherCAT Adapter", type: "text", default: "mock" },
      { key: "io.ethercat.timeout_ms", label: "EtherCAT Timeout (ms)", type: "number", default: 250, min: 1, max: 60000 },
      { key: "io.ethercat.cycle_warn_ms", label: "EtherCAT Cycle Warn (ms)", type: "number", default: 5, min: 1, max: 60000 },
      { key: "io.ethercat.on_error", label: "EtherCAT On Error", type: "select", options: ["fault", "warn", "ignore"], default: "fault" },
      { key: "io.ethercat.modules_json", label: "EtherCAT Modules (JSON)", type: "json", default: "[]" },
      { key: "io.ethercat.mock_inputs_json", label: "EtherCAT Mock Inputs (JSON)", type: "json", default: "[]" },
      { key: "io.ethercat.mock_latency_ms", label: "EtherCAT Mock Latency (ms)", type: "number", default: 0, min: 0, max: 60000 },
      { key: "io.ethercat.mock_fail_read", label: "EtherCAT Mock Fail Read", type: "toggle", default: false },
      { key: "io.ethercat.mock_fail_write", label: "EtherCAT Mock Fail Write", type: "toggle", default: false },
      { key: "io.safe_state_json", label: "I/O Safe State (JSON)", type: "json", default: "[]" },
    ],
  },
  {
    id: "security",
    label: "Security",
    fields: [
      { key: "web.auth", label: "Web Auth Mode", type: "select", options: ["local", "token"], default: "local" },
      { key: "web.tls", label: "Web TLS Enabled", type: "toggle", default: false },
      { key: "tls.mode", label: "TLS Mode", type: "select", options: ["disabled", "self-managed", "provisioned"], default: "disabled" },
      { key: "tls.require_remote", label: "Require TLS for Remote", type: "toggle", default: false },
      { key: "tls.cert_path", label: "TLS Certificate Path", type: "text", default: "" },
      { key: "tls.key_path", label: "TLS Key Path", type: "text", default: "" },
      { key: "tls.ca_path", label: "TLS CA Path", type: "text", default: "" },
      { key: "control.auth_token", label: "Control Auth Token", type: "text", default: "" },
      { key: "deploy.require_signed", label: "Require Signed Deploy", type: "toggle", default: false },
      { key: "deploy.keyring_path", label: "Deploy Keyring Path", type: "text", default: "" },
    ],
  },
  {
    id: "observability",
    label: "Observability",
    fields: [
      { key: "observability.enabled", label: "Historian Enabled", type: "toggle", default: false },
      { key: "observability.sample_interval_ms", label: "Sample Interval (ms)", type: "number", default: 1000, min: 10, max: 600000 },
      { key: "observability.mode", label: "Recording Mode", type: "select", options: ["all", "allowlist"], default: "all" },
      { key: "observability.include_json", label: "Include Patterns (JSON)", type: "json", default: "[]" },
      { key: "observability.alerts_json", label: "Alert Rules (JSON)", type: "json", default: "[]" },
      { key: "observability.history_path", label: "History Path", type: "text", default: "history/historian.jsonl" },
      { key: "observability.max_entries", label: "Max Entries", type: "number", default: 20000, min: 100, max: 1000000 },
      { key: "observability.prometheus_enabled", label: "Prometheus Enabled", type: "toggle", default: true },
      { key: "observability.prometheus_path", label: "Prometheus Path", type: "text", default: "/metrics" },
    ],
  },
  {
    id: "simulation",
    label: "Simulation",
    fields: [
      { key: "simulation.enabled", label: "Simulation Enabled", type: "toggle", default: false },
      { key: "simulation.seed", label: "Simulation Seed", type: "number", default: 0, min: 0, max: 4294967295 },
      { key: "simulation.time_scale", label: "Simulation Time Scale", type: "number", default: 1, min: 1, max: 1000000 },
    ],
  },
  {
    id: "advanced",
    label: "Advanced",
    fields: [],
  },
];

const SETTINGS_COMMUNICATION_GROUPS = [
  {
    id: "web",
    label: "Web & Discovery",
    note: "Web API endpoint and local service discovery.",
    fieldKeys: [
      "web.enabled",
      "web.listen",
      "discovery.enabled",
      "discovery.service_name",
      "discovery.advertise",
      "discovery.host_group",
      "discovery.interfaces_json",
    ],
  },
  {
    id: "mesh",
    label: "Mesh & Cloud",
    note: "Peer mesh transport and runtime-cloud profile.",
    fieldKeys: [
      "mesh.enabled",
      "mesh.role",
      "mesh.listen",
      "mesh.tls",
      "mesh.auth_token",
      "mesh.connect_json",
      "mesh.publish_json",
      "mesh.subscribe_json",
      "mesh.plugin_versions_json",
      "mesh.zenohd_version",
      "runtime_cloud.profile",
      "runtime_cloud.wan.allow_write_json",
      "runtime_cloud.links.transports_json",
    ],
  },
  {
    id: "opcua",
    label: "OPC UA",
    note: "OPC UA server endpoint and security behavior.",
    fieldKeys: [
      "opcua.enabled",
      "opcua.listen",
      "opcua.endpoint_path",
      "opcua.namespace_uri",
      "opcua.publish_interval_ms",
      "opcua.max_nodes",
      "opcua.expose_json",
      "opcua.security_policy",
      "opcua.security_mode",
      "opcua.allow_anonymous",
      "opcua.username",
      "opcua.password",
    ],
  },
  {
    id: "mqtt",
    label: "MQTT",
    note: "Broker address, topics, and reconnect behavior.",
    fieldKeys: [
      "io.mqtt.broker",
      "io.mqtt.client_id",
      "io.mqtt.topic_in",
      "io.mqtt.topic_out",
      "io.mqtt.username",
      "io.mqtt.password",
      "io.mqtt.tls",
      "io.mqtt.keep_alive_s",
      "io.mqtt.reconnect_ms",
      "io.mqtt.allow_insecure_remote",
    ],
  },
  {
    id: "modbus",
    label: "PLC / Modbus TCP",
    note: "PLC endpoint, unit-id, and register mapping.",
    fieldKeys: [
      "io.modbus.address",
      "io.modbus.unit_id",
      "io.modbus.input_start",
      "io.modbus.output_start",
      "io.modbus.timeout_ms",
      "io.modbus.on_error",
    ],
  },
  {
    id: "gpio",
    label: "GPIO",
    note: "GPIO backend and channel-to-address mappings.",
    fieldKeys: [
      "io.gpio.backend",
      "io.gpio.sysfs_base",
      "io.gpio.inputs_json",
      "io.gpio.outputs_json",
    ],
  },
  {
    id: "simulated",
    label: "Simulated I/O",
    note: "Standalone simulation process image dimensions and scan rate.",
    fieldKeys: [
      "io.simulated.inputs",
      "io.simulated.outputs",
      "io.simulated.scan_ms",
    ],
  },
  {
    id: "ethercat",
    label: "EtherCAT",
    note: "Adapter, bus modules, and mock behavior.",
    fieldKeys: [
      "io.ethercat.adapter",
      "io.ethercat.timeout_ms",
      "io.ethercat.cycle_warn_ms",
      "io.ethercat.on_error",
      "io.ethercat.modules_json",
      "io.ethercat.mock_inputs_json",
      "io.ethercat.mock_latency_ms",
      "io.ethercat.mock_fail_read",
      "io.ethercat.mock_fail_write",
    ],
  },
  {
    id: "safe_state",
    label: "I/O Safe State",
    note: "Output fallback values applied on watchdog/fault transitions.",
    fieldKeys: [
      "io.safe_state_json",
    ],
  },
];

const SETTINGS_COMM_GROUP_BY_KEY = (() => {
  const mapping = {};
  for (const group of SETTINGS_COMMUNICATION_GROUPS) {
    for (const key of group.fieldKeys) {
      mapping[key] = group.id;
    }
  }
  return mapping;
})();

const SETTINGS_QUICK_ACTIONS = [
  {
    id: "debug",
    label: "Debug",
    note: "Control mode and debug toggle",
    category: "general",
    key: "control.debug_enabled",
  },
  {
    id: "mqtt",
    label: "MQTT",
    note: "Broker, topics, reconnect",
    category: "communication",
    key: "io.mqtt.broker",
  },
  {
    id: "web-discovery",
    label: "Web/Discovery",
    note: "Web API endpoint and discovery",
    category: "communication",
    key: "web.listen",
  },
  {
    id: "mesh",
    label: "Mesh",
    note: "Peers, TLS, and cloud profile",
    category: "communication",
    key: "mesh.enabled",
  },
  {
    id: "plc",
    label: "PLC",
    note: "Modbus endpoint and unit-id",
    category: "communication",
    key: "io.modbus.address",
  },
  {
    id: "opcua",
    label: "OPC UA",
    note: "Server endpoint and security",
    category: "communication",
    key: "opcua.enabled",
  },
  {
    id: "security",
    label: "TLS/Auth",
    note: "Web auth, certs, tokens",
    category: "security",
    key: "tls.mode",
  },
  {
    id: "realtime",
    label: "Realtime",
    note: "Runtime-cloud link transport rules",
    category: "communication",
    key: "runtime_cloud.links.transports_json",
  },
  {
    id: "ethercat",
    label: "EtherCAT",
    note: "Adapter and bus module settings",
    category: "communication",
    key: "io.ethercat.adapter",
  },
  {
    id: "gpio",
    label: "GPIO",
    note: "Backend and channel maps",
    category: "communication",
    key: "io.gpio.backend",
  },
  {
    id: "safe-state",
    label: "Safe State",
    note: "Output fallback behavior",
    category: "communication",
    key: "io.safe_state_json",
  },
];

const SETTINGS_IO_BINDINGS = {
  "io.mqtt.broker": { driver: "mqtt", param: "broker", type: "text" },
  "io.mqtt.client_id": { driver: "mqtt", param: "client_id", type: "text" },
  "io.mqtt.topic_in": { driver: "mqtt", param: "topic_in", type: "text" },
  "io.mqtt.topic_out": { driver: "mqtt", param: "topic_out", type: "text" },
  "io.mqtt.username": { driver: "mqtt", param: "username", type: "text" },
  "io.mqtt.password": { driver: "mqtt", param: "password", type: "text" },
  "io.mqtt.tls": { driver: "mqtt", param: "tls", type: "toggle" },
  "io.mqtt.keep_alive_s": { driver: "mqtt", param: "keep_alive_s", type: "number" },
  "io.mqtt.reconnect_ms": { driver: "mqtt", param: "reconnect_ms", type: "number" },
  "io.mqtt.allow_insecure_remote": { driver: "mqtt", param: "allow_insecure_remote", type: "toggle" },
  "io.modbus.address": { driver: "modbus-tcp", param: "address", type: "text" },
  "io.modbus.unit_id": { driver: "modbus-tcp", param: "unit_id", type: "number" },
  "io.modbus.input_start": { driver: "modbus-tcp", param: "input_start", type: "number" },
  "io.modbus.output_start": { driver: "modbus-tcp", param: "output_start", type: "number" },
  "io.modbus.timeout_ms": { driver: "modbus-tcp", param: "timeout_ms", type: "number" },
  "io.modbus.on_error": { driver: "modbus-tcp", param: "on_error", type: "text" },
  "io.gpio.backend": { driver: "gpio", param: "backend", type: "text" },
  "io.gpio.chip": { driver: "gpio", param: "chip", type: "text" },
  "io.gpio.sysfs_base": { driver: "gpio", param: "sysfs_base", type: "text" },
  "io.gpio.inputs_json": { driver: "gpio", param: "inputs", type: "json-array" },
  "io.gpio.outputs_json": { driver: "gpio", param: "outputs", type: "json-array" },
  "io.simulated.inputs": { driver: "simulated", param: "inputs", type: "number" },
  "io.simulated.outputs": { driver: "simulated", param: "outputs", type: "number" },
  "io.simulated.scan_ms": { driver: "simulated", param: "scan_ms", type: "number" },
  "io.ethercat.adapter": { driver: "ethercat", param: "adapter", type: "text" },
  "io.ethercat.timeout_ms": { driver: "ethercat", param: "timeout_ms", type: "number" },
  "io.ethercat.cycle_warn_ms": { driver: "ethercat", param: "cycle_warn_ms", type: "number" },
  "io.ethercat.on_error": { driver: "ethercat", param: "on_error", type: "text" },
  "io.ethercat.modules_json": { driver: "ethercat", param: "modules", type: "json-array" },
  "io.ethercat.mock_inputs_json": { driver: "ethercat", param: "mock_inputs", type: "json-array" },
  "io.ethercat.mock_latency_ms": { driver: "ethercat", param: "mock_latency_ms", type: "number" },
  "io.ethercat.mock_fail_read": { driver: "ethercat", param: "mock_fail_read", type: "toggle" },
  "io.ethercat.mock_fail_write": { driver: "ethercat", param: "mock_fail_write", type: "toggle" },
};

const SETTINGS_IO_GLOBAL_KEYS = new Set([
  "io.safe_state_json",
]);

const SETTINGS_RUNTIME_BINDINGS = {
  "resource.name": { section: "resource", key: "name" },
  "log.level": { section: "runtime.log", key: "level" },
  "control.endpoint": { section: "runtime.control", key: "endpoint" },
  "control.mode": { section: "runtime.control", key: "mode" },
  "control.debug_enabled": { section: "runtime.control", key: "debug_enabled" },
  "control.auth_token": { section: "runtime.control", key: "auth_token" },
  "resource.cycle_interval_ms": { section: "resource", key: "cycle_interval_ms" },
  "resource.tasks_json": { section: "resource", key: "tasks", format: "resource-task-rules-json" },
  "watchdog.enabled": { section: "runtime.watchdog", key: "enabled" },
  "watchdog.timeout_ms": { section: "runtime.watchdog", key: "timeout_ms" },
  "watchdog.action": { section: "runtime.watchdog", key: "action" },
  "fault.policy": { section: "runtime.fault", key: "policy" },
  "retain.mode": { section: "runtime.retain", key: "mode" },
  "retain.path": { section: "runtime.retain", key: "path" },
  "retain.save_interval_ms": { section: "runtime.retain", key: "save_interval_ms" },
  "web.enabled": { section: "runtime.web", key: "enabled" },
  "web.listen": { section: "runtime.web", key: "listen" },
  "web.auth": { section: "runtime.web", key: "auth" },
  "web.tls": { section: "runtime.web", key: "tls" },
  "tls.mode": { section: "runtime.tls", key: "mode" },
  "tls.require_remote": { section: "runtime.tls", key: "require_remote" },
  "tls.cert_path": { section: "runtime.tls", key: "cert_path" },
  "tls.key_path": { section: "runtime.tls", key: "key_path" },
  "tls.ca_path": { section: "runtime.tls", key: "ca_path" },
  "deploy.require_signed": { section: "runtime.deploy", key: "require_signed" },
  "deploy.keyring_path": { section: "runtime.deploy", key: "keyring_path" },
  "discovery.enabled": { section: "runtime.discovery", key: "enabled" },
  "discovery.service_name": { section: "runtime.discovery", key: "service_name" },
  "discovery.advertise": { section: "runtime.discovery", key: "advertise" },
  "discovery.host_group": { section: "runtime.discovery", key: "host_group" },
  "discovery.interfaces_json": { section: "runtime.discovery", key: "interfaces", format: "string-array-json" },
  "mesh.enabled": { section: "runtime.mesh", key: "enabled" },
  "mesh.role": { section: "runtime.mesh", key: "role" },
  "mesh.listen": { section: "runtime.mesh", key: "listen" },
  "mesh.tls": { section: "runtime.mesh", key: "tls" },
  "mesh.auth_token": { section: "runtime.mesh", key: "auth_token" },
  "mesh.connect_json": { section: "runtime.mesh", key: "connect", format: "string-array-json" },
  "mesh.publish_json": { section: "runtime.mesh", key: "publish", format: "string-array-json" },
  "mesh.subscribe_json": { section: "runtime.mesh", key: "subscribe", format: "string-map-json" },
  "mesh.plugin_versions_json": { section: "runtime.mesh", key: "plugin_versions", format: "string-map-json" },
  "mesh.zenohd_version": { section: "runtime.mesh", key: "zenohd_version" },
  "runtime_cloud.profile": { section: "runtime.cloud", key: "profile" },
  "runtime_cloud.wan.allow_write_json": { section: "runtime.cloud.wan", key: "allow_write", format: "cloud-wan-rules-json" },
  "runtime_cloud.links.transports_json": { section: "runtime.cloud.links", key: "transports", format: "cloud-link-rules-json" },
  "opcua.enabled": { section: "runtime.opcua", key: "enabled" },
  "opcua.listen": { section: "runtime.opcua", key: "listen" },
  "opcua.endpoint_path": { section: "runtime.opcua", key: "endpoint_path" },
  "opcua.namespace_uri": { section: "runtime.opcua", key: "namespace_uri" },
  "opcua.publish_interval_ms": { section: "runtime.opcua", key: "publish_interval_ms" },
  "opcua.max_nodes": { section: "runtime.opcua", key: "max_nodes" },
  "opcua.expose_json": { section: "runtime.opcua", key: "expose", format: "string-array-json" },
  "opcua.security_policy": { section: "runtime.opcua", key: "security_policy" },
  "opcua.security_mode": { section: "runtime.opcua", key: "security_mode" },
  "opcua.allow_anonymous": { section: "runtime.opcua", key: "allow_anonymous" },
  "opcua.username": { section: "runtime.opcua", key: "username" },
  "opcua.password": { section: "runtime.opcua", key: "password" },
  "observability.enabled": { section: "runtime.observability", key: "enabled" },
  "observability.sample_interval_ms": { section: "runtime.observability", key: "sample_interval_ms" },
  "observability.mode": { section: "runtime.observability", key: "mode" },
  "observability.include_json": { section: "runtime.observability", key: "include", format: "string-array-json" },
  "observability.alerts_json": { section: "runtime.observability", key: "alerts", format: "observability-alert-rules-json" },
  "observability.history_path": { section: "runtime.observability", key: "history_path" },
  "observability.max_entries": { section: "runtime.observability", key: "max_entries" },
  "observability.prometheus_enabled": { section: "runtime.observability", key: "prometheus_enabled" },
  "observability.prometheus_path": { section: "runtime.observability", key: "prometheus_path" },
};

const SETTINGS_SIMULATION_BINDINGS = {
  "simulation.enabled": { section: "simulation", key: "enabled" },
  "simulation.seed": { section: "simulation", key: "seed" },
  "simulation.time_scale": { section: "simulation", key: "time_scale" },
};

const SETTINGS_ONLINE_KEY_MAP = Object.freeze({
  "discovery.interfaces_json": "discovery.interfaces",
  "mesh.connect_json": "mesh.connect",
  "mesh.publish_json": "mesh.publish",
  "mesh.subscribe_json": "mesh.subscribe",
  "mesh.plugin_versions_json": "mesh.plugin_versions",
  "runtime_cloud.wan.allow_write_json": "runtime_cloud.wan.allow_write",
  "runtime_cloud.links.transports_json": "runtime_cloud.links.transports",
});

const SETTINGS_RUNTIME_CONTROL_KEY_MAP = Object.freeze({
  "discovery.interfaces": "discovery.interfaces_json",
  "mesh.connect": "mesh.connect_json",
  "mesh.publish": "mesh.publish_json",
  "mesh.subscribe": "mesh.subscribe_json",
  "mesh.plugin_versions": "mesh.plugin_versions_json",
  "runtime_cloud.wan.allow_write": "runtime_cloud.wan.allow_write_json",
  "runtime_cloud.links.transports": "runtime_cloud.links.transports_json",
  "resource.tasks": "resource.tasks_json",
  "opcua.expose": "opcua.expose_json",
  "observability.include": "observability.include_json",
  "observability.alerts": "observability.alerts_json",
});

const SETTINGS_ONLINE_KEYS = new Set([
  "log.level",
  "control.mode",
  "control.debug_enabled",
  "control.auth_token",
  "watchdog.enabled",
  "watchdog.timeout_ms",
  "watchdog.action",
  "fault.policy",
  "retain.mode",
  "retain.save_interval_ms",
  "web.enabled",
  "web.listen",
  "web.auth",
  "web.tls",
  "discovery.enabled",
  "discovery.service_name",
  "discovery.advertise",
  "discovery.interfaces_json",
  "mesh.enabled",
  "mesh.role",
  "mesh.listen",
  "mesh.tls",
  "mesh.auth_token",
  "mesh.connect_json",
  "mesh.publish_json",
  "mesh.subscribe_json",
  "mesh.zenohd_version",
  "mesh.plugin_versions_json",
  "runtime_cloud.profile",
  "runtime_cloud.wan.allow_write_json",
  "runtime_cloud.links.transports_json",
]);

const SETTINGS_RESTART_REQUIRED_KEYS = new Set([
  "resource.name",
  "control.endpoint",
  "control.mode",
  "resource.tasks_json",
  "retain.mode",
  "web.enabled",
  "web.listen",
  "web.auth",
  "web.tls",
  "discovery.enabled",
  "discovery.service_name",
  "discovery.advertise",
  "discovery.host_group",
  "discovery.interfaces_json",
  "mesh.enabled",
  "mesh.role",
  "mesh.listen",
  "mesh.tls",
  "mesh.auth_token",
  "mesh.connect_json",
  "mesh.publish_json",
  "mesh.subscribe_json",
  "mesh.plugin_versions_json",
  "mesh.zenohd_version",
  "runtime_cloud.profile",
  "runtime_cloud.wan.allow_write_json",
  "runtime_cloud.links.transports_json",
  "tls.mode",
  "tls.require_remote",
  "tls.cert_path",
  "tls.key_path",
  "tls.ca_path",
  "deploy.require_signed",
  "deploy.keyring_path",
  "opcua.enabled",
  "opcua.listen",
  "opcua.endpoint_path",
  "opcua.namespace_uri",
  "opcua.publish_interval_ms",
  "opcua.max_nodes",
  "opcua.expose_json",
  "opcua.security_policy",
  "opcua.security_mode",
  "opcua.allow_anonymous",
  "opcua.username",
  "opcua.password",
  "observability.enabled",
  "observability.sample_interval_ms",
  "observability.mode",
  "observability.include_json",
  "observability.alerts_json",
  "observability.history_path",
  "observability.max_entries",
  "observability.prometheus_enabled",
  "observability.prometheus_path",
  "simulation.enabled",
  "simulation.seed",
  "simulation.time_scale",
]);

// -- State -------------------------------------------------------------------

const SETTINGS_ACTIVE_RUNTIME_STORAGE_KEY = "trust.ide.hw.activeRuntimeId";
const SETTINGS_RUNTIME_SELECTION_EVENT = "ide-runtime-selection-changed";

const settingsState = {
  activeCategory: "all",
  pendingFocusKey: null,
  searchQuery: "",
  values: {},
  loaded: false,
  loadedRuntimeScope: "",
  editingToml: false,
  tomlContent: "",
  tomlOpenedFromSettings: false,
  ioConfig: null,
  ioConfigText: "",
  ioRevision: null,
  ioRuntimeId: null,
  runtimeTargets: [],
  selectedRuntimeId: "",
  runtimeConfigText: "",
  runtimeRevision: null,
  runtimeId: null,
  runtimeFileVersion: null,
  runtimeSnapshotSource: null,
  simulationConfigText: "",
  simulationVersion: null,
  runtimeControlSnapshot: null,
};
let settingsSaveQueue = Promise.resolve();

// -- Helpers -----------------------------------------------------------------

function settingsIsStandaloneIdeMode() {
  if (typeof state !== "object" || !state) return false;
  return !!state.standaloneMode;
}

function settingsReadStoredRuntimeId() {
  try {
    return String(localStorage.getItem(SETTINGS_ACTIVE_RUNTIME_STORAGE_KEY) || "").trim();
  } catch {
    return "";
  }
}

function settingsStoreRuntimeId(runtimeId) {
  const value = String(runtimeId || "").trim();
  if (!value) return;
  try {
    localStorage.setItem(SETTINGS_ACTIVE_RUNTIME_STORAGE_KEY, value);
  } catch {
    // ignore localStorage errors
  }
}

function settingsNormalizeRuntimeTargets(items) {
  const entries = Array.isArray(items) ? items : [];
  const targets = [];
  for (const item of entries) {
    const runtimeId = String(item?.runtime_id || item?.runtimeId || "").trim();
    if (!runtimeId) continue;
    targets.push({
      runtimeId,
      runtimeRoot: String(item?.runtime_root || item?.runtimeRoot || "").trim(),
      hostGroup: String(item?.host_group || item?.hostGroup || "").trim(),
      webListen: String(item?.web_listen || item?.webListen || "").trim(),
    });
  }
  return targets;
}

function settingsFindRuntimeTarget(runtimeId) {
  const key = String(runtimeId || "").trim();
  if (!key) return null;
  return settingsState.runtimeTargets.find((target) => target.runtimeId === key) || null;
}

function settingsResolveRuntimeIdFromTargets(targets) {
  const list = Array.isArray(targets) ? targets : [];
  if (list.length === 0) return "";
  const ids = list.map((entry) => entry.runtimeId).filter(Boolean);
  if (ids.length === 0) return "";

  const selected = String(settingsState.selectedRuntimeId || "").trim();
  if (selected && ids.includes(selected)) return selected;

  const stored = settingsReadStoredRuntimeId();
  if (stored && ids.includes(stored)) return stored;

  return ids[0];
}

function settingsResolveRuntimeScope() {
  if (!settingsIsStandaloneIdeMode()) return "";
  return String(settingsState.selectedRuntimeId || "").trim();
}

function settingsNormalizePathForCompare(value) {
  return String(value || "").replace(/\\/g, "/").replace(/\/+$/, "");
}

function settingsRuntimeTomlPathForSelectedRuntime() {
  const fallback = "runtime.toml";
  if (!settingsIsStandaloneIdeMode()) return fallback;
  const runtimeId = settingsResolveRuntimeScope();
  if (!runtimeId) return fallback;
  const target = settingsFindRuntimeTarget(runtimeId);
  if (!target?.runtimeRoot) return fallback;
  const runtimeRoot = settingsNormalizePathForCompare(target.runtimeRoot);
  const projectRoot = settingsNormalizePathForCompare(state?.activeProject || "");
  if (!runtimeRoot || !projectRoot) return fallback;
  if (runtimeRoot === projectRoot) return fallback;
  if (!runtimeRoot.startsWith(`${projectRoot}/`)) return fallback;
  const relativeRoot = runtimeRoot.slice(projectRoot.length + 1);
  if (!relativeRoot) return fallback;
  return `${relativeRoot}/runtime.toml`;
}

function settingsBroadcastRuntimeSelection(runtimeId, source) {
  const value = String(runtimeId || "").trim();
  if (!value) return;
  document.dispatchEvent(new CustomEvent(SETTINGS_RUNTIME_SELECTION_EVENT, {
    detail: {
      runtimeId: value,
      source: source || "settings",
    },
  }));
}

function settingsSetSelectedRuntimeId(runtimeId, options) {
  const opts = options || {};
  const value = String(runtimeId || "").trim();
  const previous = String(settingsState.selectedRuntimeId || "").trim();
  if (value) {
    settingsState.selectedRuntimeId = value;
    settingsStoreRuntimeId(value);
  } else {
    settingsState.selectedRuntimeId = "";
  }
  const changed = previous !== settingsState.selectedRuntimeId;
  if (changed && opts.broadcast !== false) {
    settingsBroadcastRuntimeSelection(settingsState.selectedRuntimeId, opts.source || "settings");
  }
  return changed;
}

async function settingsLoadRuntimeTargets() {
  if (!settingsIsStandaloneIdeMode()) {
    settingsState.runtimeTargets = [];
    settingsSetSelectedRuntimeId("", { broadcast: false });
    return;
  }

  let targets = [];
  try {
    const result = await apiJson("/api/config-ui/runtime/lifecycle", {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 4000,
    });
    targets = settingsNormalizeRuntimeTargets(result?.items);
  } catch {
    targets = [];
  }

  settingsState.runtimeTargets = targets;
  const nextRuntimeId = targets.length > 0
    ? settingsResolveRuntimeIdFromTargets(targets)
    : settingsReadStoredRuntimeId();
  settingsSetSelectedRuntimeId(nextRuntimeId, {
    broadcast: false,
  });
}

function settingsFieldByKey(key) {
  for (const cat of SETTINGS_CATEGORIES) {
    for (const field of cat.fields) {
      if (field.key === key) return field;
    }
  }
  return null;
}

function settingsCategoryById(catId) {
  return SETTINGS_CATEGORIES.find((cat) => cat.id === catId) || null;
}

function settingsCategoryLabelById(catId) {
  return settingsCategoryById(catId)?.label || catId;
}

function settingsCategoryIdForFieldKey(key) {
  const target = String(key || "").trim();
  if (!target) return null;
  for (const cat of SETTINGS_CATEGORIES) {
    if (!Array.isArray(cat.fields) || cat.fields.length === 0) continue;
    if (cat.fields.some((field) => field.key === target)) {
      return cat.id;
    }
  }
  return null;
}

function settingsAllFields() {
  const fields = [];
  for (const cat of SETTINGS_CATEGORIES) {
    for (const field of cat.fields) fields.push(field);
  }
  return fields;
}

function settingsAttrSelector(name, value) {
  const attr = String(name || "").trim();
  const rawValue = String(value ?? "");
  if (window.CSS && typeof window.CSS.escape === "function") {
    return `[${attr}="${window.CSS.escape(rawValue)}"]`;
  }
  const escaped = rawValue
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"');
  return `[${attr}="${escaped}"]`;
}

function settingsNormalizeSearchQuery(query) {
  return String(query || "").trim().toLowerCase();
}

function settingsFieldMatchesQuery(field, normalizedQuery) {
  if (!field) return false;
  if (!normalizedQuery) return true;
  const label = String(field.label || "").toLowerCase();
  const key = String(field.key || "").toLowerCase();
  return label.includes(normalizedQuery) || key.includes(normalizedQuery);
}

function settingsFilterFieldsByQuery(fields, normalizedQuery) {
  const list = Array.isArray(fields) ? fields : [];
  if (!normalizedQuery) return list.slice();
  return list.filter((field) => settingsFieldMatchesQuery(field, normalizedQuery));
}

function settingsBuildCommunicationGroups(fields, options) {
  const opts = options || {};
  const prefixCategory = !!opts.prefixCategory;
  const groups = SETTINGS_COMMUNICATION_GROUPS.map((group) => ({
    id: group.id,
    label: prefixCategory ? `Communication / ${group.label}` : group.label,
    note: group.note,
    fields: [],
  }));
  const byId = new Map(groups.map((group) => [group.id, group]));
  const fallback = {
    id: "other",
    label: prefixCategory ? "Communication / Other" : "Other",
    note: null,
    fields: [],
  };

  for (const field of fields) {
    const groupId = SETTINGS_COMM_GROUP_BY_KEY[field.key];
    const target = groupId ? byId.get(groupId) : fallback;
    if (target) {
      target.fields.push(field);
    } else {
      fallback.fields.push(field);
    }
  }

  const visible = groups.filter((group) => group.fields.length > 0);
  if (fallback.fields.length > 0) visible.push(fallback);
  return visible;
}

function settingsGroupsForAllCategory(normalizedQuery) {
  const groups = [];
  for (const category of SETTINGS_CATEGORIES) {
    if (category.id === "all" || category.id === "advanced") continue;
    const filtered = settingsFilterFieldsByQuery(category.fields, normalizedQuery);
    if (filtered.length === 0) continue;

    if (category.id === "communication") {
      groups.push(...settingsBuildCommunicationGroups(filtered, { prefixCategory: true }));
      continue;
    }

    groups.push({
      id: category.id,
      label: category.label,
      note: null,
      fields: filtered,
    });
  }
  return groups;
}

function settingsGroupsForCategory(cat) {
  if (!cat) return [];
  const normalizedQuery = settingsNormalizeSearchQuery(settingsState.searchQuery);

  if (cat.id === "all") {
    return settingsGroupsForAllCategory(normalizedQuery);
  }

  if (!Array.isArray(cat.fields) || cat.fields.length === 0) return [];
  const filteredFields = settingsFilterFieldsByQuery(cat.fields, normalizedQuery);
  if (filteredFields.length === 0) return [];

  if (cat.id !== "communication") {
    return [{
      id: cat.id,
      label: null,
      note: null,
      fields: filteredFields,
    }];
  }

  return settingsBuildCommunicationGroups(filteredFields, { prefixCategory: false });
}

function settingsVisibleFieldCount(groups) {
  return (Array.isArray(groups) ? groups : []).reduce(
    (sum, group) => sum + (Array.isArray(group?.fields) ? group.fields.length : 0),
    0,
  );
}

function settingsCategoryTotalFields(cat) {
  if (!cat) return 0;
  if (cat.id === "all") {
    return settingsAllFields().length;
  }
  return Array.isArray(cat.fields) ? cat.fields.length : 0;
}

function settingsRenderQuickActions(activeCategory) {
  let html = '<section class="settings-quick-actions" aria-label="Quick settings">';
  for (const action of SETTINGS_QUICK_ACTIONS) {
    const active = action.category === activeCategory;
    html += `<button type="button" class="settings-quick-btn${active ? " active" : ""}" data-settings-jump-category="${escapeAttr(action.category)}" data-settings-jump-key="${escapeAttr(action.key)}">`;
    html += `<span class="settings-quick-label">${escapeHtml(action.label)}</span>`;
    html += `<span class="settings-quick-note">${escapeHtml(action.note)} \u2022 ${escapeHtml(settingsCategoryLabelById(action.category))}</span>`;
    html += "</button>";
  }
  html += "</section>";
  return html;
}

function settingsRenderRuntimeScopeBar() {
  if (!settingsIsStandaloneIdeMode()) return "";

  const runtimeId = settingsResolveRuntimeScope();
  const targets = Array.isArray(settingsState.runtimeTargets) ? settingsState.runtimeTargets : [];
  if (targets.length === 0) {
    return `<section class="settings-runtime-scope">
      <div class="settings-runtime-scope-head">
        <span class="settings-runtime-scope-label">Runtime Scope</span>
      </div>
      <div class="settings-runtime-scope-note">No standalone runtime profiles discovered yet.</div>
    </section>`;
  }

  const options = targets.map((target) => {
    const selected = target.runtimeId === runtimeId ? " selected" : "";
    return `<option value="${escapeAttr(target.runtimeId)}"${selected}>${escapeHtml(target.runtimeId)}</option>`;
  }).join("");
  const selectedTarget = settingsFindRuntimeTarget(runtimeId);
  const detail = [];
  if (selectedTarget?.hostGroup) detail.push(`host ${selectedTarget.hostGroup}`);
  if (selectedTarget?.webListen) detail.push(`web ${selectedTarget.webListen}`);
  const note = detail.length > 0
    ? detail.join(" \u2022 ")
    : "Edits apply to the selected runtime profile.";

  return `<section class="settings-runtime-scope">
    <div class="settings-runtime-scope-head">
      <span class="settings-runtime-scope-label">Runtime Scope</span>
      <select id="settingsRuntimeSelect" aria-label="Settings runtime scope">
        ${options}
      </select>
    </div>
    <div class="settings-runtime-scope-note">${escapeHtml(note)}</div>
  </section>`;
}

function settingsBindRuntimeScope(panel) {
  const runtimeSelect = panel.querySelector("#settingsRuntimeSelect");
  if (!runtimeSelect) return;
  runtimeSelect.addEventListener("change", () => {
    const nextRuntimeId = String(runtimeSelect.value || "").trim();
    if (!nextRuntimeId) return;
    const changed = settingsSetSelectedRuntimeId(nextRuntimeId, {
      source: "settings",
      broadcast: true,
    });
    if (!changed) return;
    settingsState.loaded = false;
    settingsState.runtimeControlSnapshot = null;
    void settingsLoad();
  });
}

function settingsRenderField(field, value) {
  let html = '<div class="field" style="margin-bottom:10px">';
  html += `<label style="font-size:12px;color:var(--muted-strong);font-weight:500">${escapeHtml(field.label)}</label>`;

  switch (field.type) {
    case "select":
      html += `<select data-settings-key="${escapeAttr(field.key)}" style="font-size:12px">`;
      for (const opt of field.options) {
        html += `<option value="${escapeAttr(opt)}"${String(value) === opt ? " selected" : ""}>${escapeHtml(opt)}</option>`;
      }
      html += "</select>";
      break;
    case "toggle":
      html += `<label class="settings-toggle"><input type="checkbox" data-settings-key="${escapeAttr(field.key)}"${value ? " checked" : ""}/><span class="settings-toggle-slider"></span></label>`;
      break;
    case "number":
      html += `<input type="number" value="${escapeAttr(String(value))}" data-settings-key="${escapeAttr(field.key)}"${field.min != null ? ` min="${field.min}"` : ""}${field.max != null ? ` max="${field.max}"` : ""} style="font-size:12px;width:180px"/>`;
      break;
    case "json":
      html += `<textarea data-settings-key="${escapeAttr(field.key)}" class="settings-json-input" spellcheck="false">${escapeHtml(String(value ?? ""))}</textarea>`;
      break;
    case "password":
      html += `<input type="password" value="${escapeAttr(String(value))}" data-settings-key="${escapeAttr(field.key)}" style="font-size:12px"/>`;
      break;
    case "text":
    default:
      html += `<input type="text" value="${escapeAttr(String(value))}" data-settings-key="${escapeAttr(field.key)}" style="font-size:12px"/>`;
      break;
  }

  if (field.hint) {
    html += `<p class="muted" style="margin:4px 0 0;font-size:11px">${escapeHtml(field.hint)}</p>`;
  }

  html += "</div>";
  return html;
}

function settingsBindQuickActions(panel) {
  panel.querySelectorAll("[data-settings-jump-category]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const category = String(btn.dataset.settingsJumpCategory || "").trim();
      const key = String(btn.dataset.settingsJumpKey || "").trim();
      if (!category) return;
      settingsSelectCategory(category, {
        focusKey: key || null,
        clearSearch: true,
      });
    });
  });
}

function settingsFocusPendingField(panel) {
  const key = settingsState.pendingFocusKey;
  if (!key) return;

  const selector = settingsAttrSelector("data-settings-key", key);
  let input = panel.querySelector(selector);
  if (!input) {
    if (settingsState.searchQuery) {
      settingsState.searchQuery = "";
      settingsRenderCategories();
      settingsRenderForm();
      return;
    }
    const ownerCategory = settingsCategoryIdForFieldKey(key);
    if (ownerCategory && ownerCategory !== settingsState.activeCategory) {
      settingsSelectCategory(ownerCategory, {
        focusKey: key,
        clearSearch: true,
      });
      return;
    }
    if (settingsState.activeCategory !== "all") {
      settingsSelectCategory("all", {
        focusKey: key,
        clearSearch: true,
      });
      return;
    }
    settingsState.pendingFocusKey = null;
    return;
  }

  settingsState.pendingFocusKey = null;

  const field = input.closest(".field");
  if (field) {
    field.classList.remove("settings-field-highlight");
    field.classList.add("settings-field-highlight");
    setTimeout(() => {
      field.classList.remove("settings-field-highlight");
    }, 1200);
  }

  requestAnimationFrame(() => {
    input.scrollIntoView({ behavior: "smooth", block: "center", inline: "nearest" });
    try {
      input.focus({ preventScroll: true });
    } catch {
      input.focus();
    }
  });
}

function settingsNormalizeSelectValue(field, value) {
  const raw = String(value ?? "").trim();
  for (const option of field.options || []) {
    if (option.toLowerCase() === raw.toLowerCase()) return option;
  }
  return field.default;
}

function settingsNormalizeValue(field, value) {
  if (!field) return value;
  if (field.type === "toggle") {
    return !!value;
  }
  if (field.type === "number") {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) return Number(field.default) || 0;
    if (field.min != null && parsed < field.min) return field.min;
    if (field.max != null && parsed > field.max) return field.max;
    return parsed;
  }
  if (field.type === "select") {
    return settingsNormalizeSelectValue(field, value);
  }
  if (field.type === "json") {
    if (typeof value === "string") {
      const text = value.trim();
      if (!text) return String(field.default || "");
      try {
        return JSON.stringify(JSON.parse(text), null, 2);
      } catch {
        return value;
      }
    }
    if (Array.isArray(value) || (value && typeof value === "object")) {
      try {
        return JSON.stringify(value, null, 2);
      } catch {
        return String(field.default || "");
      }
    }
    return String(field.default || "");
  }
  return String(value ?? "");
}

function settingsFormatTomlString(value) {
  return `"${String(value ?? "")
    .replace(/\\/g, "\\\\")
    .replace(/\"/g, '\\\"')}"`;
}

function settingsFormatTomlValue(field, value) {
  if (!field) return settingsFormatTomlString(String(value ?? ""));
  if (field.type === "toggle") return value ? "true" : "false";
  if (field.type === "number") return String(Number(value) || 0);
  return settingsFormatTomlString(String(value ?? ""));
}

function settingsEscapeRegex(text) {
  return String(text).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function settingsTomlParseValue(raw) {
  const text = String(raw || "").trim();
  if (!text) return "";
  if (text.startsWith('"') && text.endsWith('"')) {
    return text.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  }
  if (text === "true") return true;
  if (text === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(text)) return Number(text);
  if (text.startsWith("{") && text.endsWith("}")) {
    const table = settingsTomlParseInlineTable(text);
    if (table && typeof table === "object") return table;
  }
  if (text.startsWith("[") && text.endsWith("]")) {
    const inner = text.slice(1, -1).trim();
    if (!inner) return [];
    return settingsTomlSplitTopLevel(inner)
      .map((part) => settingsTomlParseValue(part))
      .filter((part) => part !== "");
  }
  return text;
}

function settingsTomlParseSections(text) {
  const sections = {};
  const lines = String(text || "").split(/\r?\n/);
  let currentSection = "";

  for (const lineRaw of lines) {
    const line = lineRaw.replace(/#.*$/, "").trim();
    if (!line) continue;

    const sectionMatch = line.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1].trim();
      if (!sections[currentSection]) sections[currentSection] = {};
      continue;
    }

    if (!currentSection) continue;
    const eqIndex = line.indexOf("=");
    if (eqIndex <= 0) continue;

    const key = line.slice(0, eqIndex).trim();
    const valueRaw = line.slice(eqIndex + 1).trim();
    sections[currentSection][key] = settingsTomlParseValue(valueRaw);
  }

  return sections;
}

function settingsTomlStripComment(line) {
  let inString = false;
  let quote = "";
  let escape = false;
  let out = "";
  for (const ch of String(line || "")) {
    if (inString) {
      out += ch;
      if (escape) {
        escape = false;
        continue;
      }
      if (ch === "\\") {
        escape = true;
        continue;
      }
      if (ch === quote) {
        inString = false;
        quote = "";
      }
      continue;
    }
    if (ch === "\"" || ch === "'") {
      inString = true;
      quote = ch;
      out += ch;
      continue;
    }
    if (ch === "#") break;
    out += ch;
  }
  return out.trim();
}

function settingsTomlNeedsContinuation(raw) {
  const text = String(raw || "");
  let inString = false;
  let quote = "";
  let escape = false;
  let bracketDepth = 0;
  let braceDepth = 0;

  for (const ch of text) {
    if (inString) {
      if (escape) {
        escape = false;
        continue;
      }
      if (ch === "\\") {
        escape = true;
        continue;
      }
      if (ch === quote) {
        inString = false;
        quote = "";
      }
      continue;
    }
    if (ch === "\"" || ch === "'") {
      inString = true;
      quote = ch;
      continue;
    }
    if (ch === "[") bracketDepth += 1;
    if (ch === "]") bracketDepth = Math.max(0, bracketDepth - 1);
    if (ch === "{") braceDepth += 1;
    if (ch === "}") braceDepth = Math.max(0, braceDepth - 1);
  }
  return bracketDepth > 0 || braceDepth > 0 || inString;
}

function settingsTomlSplitTopLevel(text) {
  const input = String(text || "");
  const parts = [];
  let current = "";
  let inString = false;
  let quote = "";
  let escape = false;
  let bracketDepth = 0;
  let braceDepth = 0;

  for (const ch of input) {
    if (inString) {
      current += ch;
      if (escape) {
        escape = false;
        continue;
      }
      if (ch === "\\") {
        escape = true;
        continue;
      }
      if (ch === quote) {
        inString = false;
        quote = "";
      }
      continue;
    }

    if (ch === "\"" || ch === "'") {
      inString = true;
      quote = ch;
      current += ch;
      continue;
    }
    if (ch === "[") {
      bracketDepth += 1;
      current += ch;
      continue;
    }
    if (ch === "]") {
      bracketDepth = Math.max(0, bracketDepth - 1);
      current += ch;
      continue;
    }
    if (ch === "{") {
      braceDepth += 1;
      current += ch;
      continue;
    }
    if (ch === "}") {
      braceDepth = Math.max(0, braceDepth - 1);
      current += ch;
      continue;
    }

    if (ch === "," && bracketDepth === 0 && braceDepth === 0) {
      const trimmed = current.trim();
      if (trimmed) parts.push(trimmed);
      current = "";
      continue;
    }

    current += ch;
  }

  const tail = current.trim();
  if (tail) parts.push(tail);
  return parts;
}

function settingsTomlUnquote(text) {
  const value = String(text || "").trim();
  if (value.startsWith("\"") && value.endsWith("\"")) {
    return settingsTomlParseValue(value);
  }
  if (value.startsWith("'") && value.endsWith("'")) {
    return value.slice(1, -1);
  }
  return value;
}

function settingsTomlReadRawAssignment(text, section, key) {
  const lines = String(text || "").replace(/\r\n/g, "\n").split("\n");
  let sectionStart = -1;
  let sectionEnd = lines.length;

  for (let i = 0; i < lines.length; i += 1) {
    const match = lines[i].match(/^\s*\[([^\]]+)\]\s*$/);
    if (!match) continue;
    const sectionName = match[1].trim();
    if (sectionName === section) {
      sectionStart = i;
      for (let j = i + 1; j < lines.length; j += 1) {
        if (/^\s*\[[^\]]+\]\s*$/.test(lines[j])) {
          sectionEnd = j;
          break;
        }
      }
      break;
    }
  }
  if (sectionStart < 0) return null;

  const keyRegex = new RegExp(`^\\s*${settingsEscapeRegex(key)}\\s*=\\s*(.*)$`);
  for (let i = sectionStart + 1; i < sectionEnd; i += 1) {
    const line = lines[i];
    const match = line.match(keyRegex);
    if (!match) continue;

    let rawValue = settingsTomlStripComment(match[1]);
    let cursor = i;
    while (settingsTomlNeedsContinuation(rawValue) && cursor + 1 < sectionEnd) {
      cursor += 1;
      const continuation = settingsTomlStripComment(lines[cursor]);
      if (!continuation) continue;
      rawValue = `${rawValue}\n${continuation}`;
    }
    return rawValue.trim();
  }
  return null;
}

function settingsTomlParseInlineTable(raw) {
  const text = String(raw || "").trim();
  if (!text.startsWith("{") || !text.endsWith("}")) return null;
  const inner = text.slice(1, -1).trim();
  if (!inner) return {};
  const out = {};
  const entries = settingsTomlSplitTopLevel(inner);
  for (const entry of entries) {
    const eqIndex = entry.indexOf("=");
    if (eqIndex <= 0) continue;
    const key = settingsTomlUnquote(entry.slice(0, eqIndex));
    const valueRaw = entry.slice(eqIndex + 1).trim();
    out[String(key).trim()] = settingsTomlParseValue(valueRaw);
  }
  return out;
}

function settingsTomlParseStringArrayRaw(raw) {
  const text = String(raw || "").trim();
  if (!text.startsWith("[") || !text.endsWith("]")) return null;
  const inner = text.slice(1, -1).trim();
  if (!inner) return [];
  return settingsTomlSplitTopLevel(inner)
    .map((part) => settingsTomlUnquote(part))
    .map((item) => String(item || "").trim())
    .filter((item) => item.length > 0);
}

function settingsTomlParseStringMapRaw(raw) {
  const map = settingsTomlParseInlineTable(raw);
  if (!map || Array.isArray(map) || typeof map !== "object") return null;
  const out = {};
  for (const [key, value] of Object.entries(map)) {
    out[String(key)] = String(value ?? "");
  }
  return out;
}

function settingsTomlParseRuleArrayRaw(raw, requiredKeys) {
  const text = String(raw || "").trim();
  if (!text.startsWith("[") || !text.endsWith("]")) return null;
  const inner = text.slice(1, -1).trim();
  if (!inner) return [];

  const rules = [];
  for (const entry of settingsTomlSplitTopLevel(inner)) {
    const table = settingsTomlParseInlineTable(entry);
    if (!table || typeof table !== "object" || Array.isArray(table)) continue;
    const rule = {};
    let valid = true;
    for (const key of requiredKeys) {
      const value = String(table[key] ?? "").trim();
      if (!value) {
        valid = false;
        break;
      }
      rule[key] = value;
    }
    if (valid) rules.push(rule);
  }
  return rules;
}

function settingsTomlNormalizeLinkTransport(transport) {
  const value = String(transport || "").trim().toLowerCase();
  if (value === "modbus_tcp") return "modbus-tcp";
  if (SETTINGS_RUNTIME_LINK_TRANSPORTS.includes(value)) return value;
  throw new Error(`Link transport must be one of: ${SETTINGS_RUNTIME_LINK_TRANSPORTS.join(", ")}`);
}

function settingsIsFiniteNumber(value) {
  return typeof value === "number" && Number.isFinite(value);
}

function settingsNormalizeObservabilityAlertRule(entry) {
  const name = String(entry?.name ?? "").trim();
  const variable = String(entry?.variable ?? "").trim();
  const hasAbove = settingsIsFiniteNumber(entry?.above);
  const hasBelow = settingsIsFiniteNumber(entry?.below);
  if (!name) throw new Error("Alert rule requires non-empty name");
  if (!variable) throw new Error("Alert rule requires non-empty variable");
  if (!hasAbove && !hasBelow) throw new Error("Alert rule requires above and/or below");

  const out = {
    name,
    variable,
    debounce_samples: 1,
  };
  if (hasAbove) out.above = Number(entry.above);
  if (hasBelow) out.below = Number(entry.below);

  const debounceRaw = entry?.debounce_samples;
  if (debounceRaw != null && debounceRaw !== "") {
    const debounce = Number(debounceRaw);
    if (!Number.isInteger(debounce) || debounce < 1) {
      throw new Error("Alert rule debounce_samples must be an integer >= 1");
    }
    out.debounce_samples = debounce;
  }

  const hook = String(entry?.hook ?? "").trim();
  if (hook.length > 0) out.hook = hook;

  return out;
}

function settingsNormalizeResourceTaskRule(entry) {
  const name = String(entry?.name ?? "").trim();
  if (!name) throw new Error("Task rule requires non-empty name");

  const interval = Number(entry?.interval_ms);
  if (!Number.isInteger(interval) || interval < 1) {
    throw new Error("Task rule interval_ms must be an integer >= 1");
  }

  const priority = Number(entry?.priority);
  if (!Number.isInteger(priority) || priority < 0 || priority > 255) {
    throw new Error("Task rule priority must be an integer between 0 and 255");
  }

  const programsRaw = Array.isArray(entry?.programs) ? entry.programs : [];
  const programs = programsRaw
    .map((item) => String(item ?? "").trim())
    .filter((item) => item.length > 0);
  if (programs.length === 0) {
    throw new Error("Task rule requires at least one program");
  }

  const out = {
    name,
    interval_ms: interval,
    priority,
    programs,
  };
  const single = String(entry?.single ?? "").trim();
  if (single.length > 0) out.single = single;
  return out;
}

function settingsFormatResourceTaskRuleToml(rule) {
  const parts = [
    `name = ${settingsFormatTomlString(rule.name)}`,
    `interval_ms = ${rule.interval_ms}`,
    `priority = ${rule.priority}`,
    `programs = [${rule.programs.map((program) => settingsFormatTomlString(program)).join(", ")}]`,
  ];
  if (rule.single) {
    parts.push(`single = ${settingsFormatTomlString(rule.single)}`);
  }
  return `{ ${parts.join(", ")} }`;
}

function settingsFormatObservabilityAlertRuleToml(rule) {
  const parts = [
    `name = ${settingsFormatTomlString(rule.name)}`,
    `variable = ${settingsFormatTomlString(rule.variable)}`,
  ];
  if (settingsIsFiniteNumber(rule.above)) {
    parts.push(`above = ${Number(rule.above)}`);
  }
  if (settingsIsFiniteNumber(rule.below)) {
    parts.push(`below = ${Number(rule.below)}`);
  }
  if (rule.debounce_samples != null) {
    parts.push(`debounce_samples = ${Math.max(1, Number(rule.debounce_samples) || 1)}`);
  }
  if (rule.hook) {
    parts.push(`hook = ${settingsFormatTomlString(rule.hook)}`);
  }
  return `{ ${parts.join(", ")} }`;
}

function settingsTomlParseObservabilityAlertRulesRaw(raw) {
  const text = String(raw || "").trim();
  if (!text.startsWith("[") || !text.endsWith("]")) return null;
  const inner = text.slice(1, -1).trim();
  if (!inner) return [];

  const rules = [];
  for (const entry of settingsTomlSplitTopLevel(inner)) {
    const table = settingsTomlParseInlineTable(entry);
    if (!table || typeof table !== "object" || Array.isArray(table)) continue;
    try {
      rules.push(settingsNormalizeObservabilityAlertRule(table));
    } catch {
      // Ignore malformed fragments when decoding TOML.
    }
  }
  return rules;
}

function settingsTomlParseResourceTaskRulesRaw(raw) {
  const text = String(raw || "").trim();
  if (!text.startsWith("[") || !text.endsWith("]")) return null;
  const inner = text.slice(1, -1).trim();
  if (!inner) return [];

  const rules = [];
  for (const entry of settingsTomlSplitTopLevel(inner)) {
    const table = settingsTomlParseInlineTable(entry);
    if (!table || typeof table !== "object" || Array.isArray(table)) continue;
    try {
      rules.push(settingsNormalizeResourceTaskRule(table));
    } catch {
      // Ignore malformed fragments when decoding TOML.
    }
  }
  return rules;
}

function settingsTomlDecodeBindingRaw(binding, rawValue) {
  const format = binding?.format || "";
  if (!rawValue || !format) return undefined;

  if (format === "string-array-json") {
    const values = settingsTomlParseStringArrayRaw(rawValue);
    if (!Array.isArray(values)) return undefined;
    return JSON.stringify(values, null, 2);
  }
  if (format === "string-map-json") {
    const values = settingsTomlParseStringMapRaw(rawValue);
    if (!values || typeof values !== "object" || Array.isArray(values)) return undefined;
    return JSON.stringify(values, null, 2);
  }
  if (format === "cloud-wan-rules-json") {
    const values = settingsTomlParseRuleArrayRaw(rawValue, ["action", "target"]);
    if (!Array.isArray(values)) return undefined;
    return JSON.stringify(values, null, 2);
  }
  if (format === "cloud-link-rules-json") {
    const values = settingsTomlParseRuleArrayRaw(rawValue, ["source", "target", "transport"]);
    if (!Array.isArray(values)) return undefined;
    for (const item of values) {
      try {
        item.transport = settingsTomlNormalizeLinkTransport(item.transport);
      } catch {
        return undefined;
      }
    }
    return JSON.stringify(values, null, 2);
  }
  if (format === "observability-alert-rules-json") {
    const values = settingsTomlParseObservabilityAlertRulesRaw(rawValue);
    if (!Array.isArray(values)) return undefined;
    return JSON.stringify(values, null, 2);
  }
  if (format === "resource-task-rules-json") {
    const values = settingsTomlParseResourceTaskRulesRaw(rawValue);
    if (!Array.isArray(values)) return undefined;
    return JSON.stringify(values, null, 2);
  }
  return undefined;
}

function settingsParseJsonTextOrThrow(value, fallbackText) {
  const text = String(value ?? "").trim();
  if (!text) return JSON.parse(String(fallbackText || "[]"));
  try {
    return JSON.parse(text);
  } catch {
    throw new Error("Value must be valid JSON");
  }
}

function settingsNormalizeCloudLinkRulesOrThrow(value) {
  const parsed = settingsParseJsonTextOrThrow(value, "[]");
  if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
  const rules = parsed.map((entry) => {
    const source = String(entry?.source ?? entry?.from ?? entry?.pattern ?? "").trim();
    const target = String(entry?.target ?? entry?.to ?? "*").trim() || "*";
    const transport = settingsTomlNormalizeLinkTransport(entry?.transport);
    return { source, target, transport };
  });
  if (rules.some((entry) => !entry.source || !entry.target)) {
    throw new Error("Each link rule requires source and target");
  }
  return rules;
}

function settingsTomlEncodeBindingValue(binding, value) {
  const format = binding?.format || "";
  if (!format) return null;

  if (format === "string-array-json") {
    const parsed = settingsParseJsonTextOrThrow(value, "[]");
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
    const values = parsed
      .map((entry) => String(entry ?? "").trim())
      .filter((entry) => entry.length > 0);
    return `[${values.map((entry) => settingsFormatTomlString(entry)).join(", ")}]`;
  }

  if (format === "string-map-json") {
    const parsed = settingsParseJsonTextOrThrow(value, "{}");
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("Expected JSON object");
    }
    const entries = Object.entries(parsed)
      .map(([key, entryValue]) => [String(key).trim(), String(entryValue ?? "").trim()])
      .filter(([key, entryValue]) => key.length > 0 && entryValue.length > 0);
    if (entries.length === 0) return "{}";
    const pairs = entries.map(([key, entryValue]) => (
      `${settingsFormatTomlString(key)} = ${settingsFormatTomlString(entryValue)}`
    ));
    return `{ ${pairs.join(", ")} }`;
  }

  if (format === "cloud-wan-rules-json") {
    const parsed = settingsParseJsonTextOrThrow(value, "[]");
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
    const rules = parsed.map((entry) => ({
      action: String(entry?.action ?? "").trim(),
      target: String(entry?.target ?? "").trim(),
    }));
    if (rules.some((entry) => !entry.action || !entry.target)) {
      throw new Error("Each rule requires action and target");
    }
    if (rules.length === 0) return "[]";
    return `[${rules.map((entry) => (
      `{ action = ${settingsFormatTomlString(entry.action)}, target = ${settingsFormatTomlString(entry.target)} }`
    )).join(", ")}]`;
  }

  if (format === "cloud-link-rules-json") {
    const rules = settingsNormalizeCloudLinkRulesOrThrow(value);
    if (rules.length === 0) return "[]";
    return `[${rules.map((entry) => (
      `{ source = ${settingsFormatTomlString(entry.source)}, target = ${settingsFormatTomlString(entry.target)}, transport = ${settingsFormatTomlString(entry.transport)} }`
    )).join(", ")}]`;
  }
  if (format === "observability-alert-rules-json") {
    const parsed = settingsParseJsonTextOrThrow(value, "[]");
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
    const rules = parsed.map((entry) => settingsNormalizeObservabilityAlertRule(entry));
    if (rules.length === 0) return "[]";
    return `[${rules.map((entry) => settingsFormatObservabilityAlertRuleToml(entry)).join(", ")}]`;
  }
  if (format === "resource-task-rules-json") {
    const parsed = settingsParseJsonTextOrThrow(value, "[]");
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
    const rules = parsed.map((entry) => settingsNormalizeResourceTaskRule(entry));
    if (rules.length === 0) return "[]";
    return `[${rules.map((entry) => settingsFormatResourceTaskRuleToml(entry)).join(", ")}]`;
  }

  return null;
}

function settingsTomlUpsert(text, section, key, formattedValue) {
  const rawText = String(text || "");
  const newline = rawText.includes("\r\n") ? "\r\n" : "\n";
  const lines = rawText.length > 0 ? rawText.replace(/\r\n/g, "\n").split("\n") : [];

  let sectionStart = -1;
  let sectionEnd = lines.length;
  for (let i = 0; i < lines.length; i += 1) {
    const header = lines[i].match(/^\s*\[([^\]]+)\]\s*$/);
    if (!header) continue;
    if (header[1].trim() === section) {
      sectionStart = i;
      for (let j = i + 1; j < lines.length; j += 1) {
        if (/^\s*\[[^\]]+\]\s*$/.test(lines[j])) {
          sectionEnd = j;
          break;
        }
      }
      break;
    }
  }

  const assignment = `${key} = ${formattedValue}`;

  if (sectionStart < 0) {
    if (lines.length > 0 && lines[lines.length - 1].trim() !== "") lines.push("");
    lines.push(`[${section}]`);
    lines.push(assignment);
    return `${lines.join(newline)}${newline}`;
  }

  const keyRegex = new RegExp(`^\\s*${settingsEscapeRegex(key)}\\s*=`);
  for (let i = sectionStart + 1; i < sectionEnd; i += 1) {
    if (keyRegex.test(lines[i])) {
      lines[i] = assignment;
      return `${lines.join(newline)}${newline}`;
    }
  }

  lines.splice(sectionEnd, 0, assignment);
  return `${lines.join(newline)}${newline}`;
}

function settingsIsConflictError(err) {
  const message = String(err?.message || err || "").toLowerCase();
  return message.includes("409") || message.includes("conflict") || message.includes("stale write");
}

function settingsIsUnsupportedOnlineKeyError(err) {
  const message = String(err?.message || err || "").toLowerCase();
  return message.includes("unknown config key");
}

function settingsSetRestartWarning(message) {
  const warn = document.getElementById("settingsRestartWarn");
  if (!warn) return;
  warn.hidden = false;
  warn.textContent = message;
}

function settingsClearRestartWarning() {
  const warn = document.getElementById("settingsRestartWarn");
  if (!warn) return;
  warn.hidden = true;
  warn.textContent = "";
}

function settingsBuildConfigSetValue(key, value) {
  if (key === "control.auth_token" || key === "mesh.auth_token") {
    const token = String(value ?? "").trim();
    return token.length > 0 ? token : null;
  }
  if (
    key === "discovery.interfaces_json"
    || key === "mesh.connect_json"
    || key === "mesh.publish_json"
    || key === "observability.include_json"
  ) {
    const parsed = settingsParseJsonTextOrThrow(value, "[]");
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
    return parsed
      .map((entry) => String(entry ?? "").trim())
      .filter((entry) => entry.length > 0);
  }
  if (key === "mesh.subscribe_json" || key === "mesh.plugin_versions_json") {
    const parsed = settingsParseJsonTextOrThrow(value, "{}");
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("Expected JSON object");
    }
    const out = {};
    for (const [entryKey, entryValue] of Object.entries(parsed)) {
      const cleanKey = String(entryKey || "").trim();
      const cleanValue = String(entryValue ?? "").trim();
      if (!cleanKey || !cleanValue) continue;
      out[cleanKey] = cleanValue;
    }
    return out;
  }
  if (key === "runtime_cloud.wan.allow_write_json") {
    const parsed = settingsParseJsonTextOrThrow(value, "[]");
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
    const rules = parsed.map((entry) => ({
      action: String(entry?.action ?? "").trim(),
      target: String(entry?.target ?? "").trim(),
    }));
    if (rules.some((entry) => !entry.action || !entry.target)) {
      throw new Error("Each rule requires action and target");
    }
    return rules;
  }
  if (key === "runtime_cloud.links.transports_json") {
    return settingsNormalizeCloudLinkRulesOrThrow(value);
  }
  if (key === "observability.alerts_json") {
    const parsed = settingsParseJsonTextOrThrow(value, "[]");
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
    return parsed.map((entry) => settingsNormalizeObservabilityAlertRule(entry));
  }
  if (
    key === "control.mode"
    || key === "watchdog.action"
    || key === "fault.policy"
    || key === "retain.mode"
    || key === "web.auth"
    || key === "mesh.role"
    || key === "runtime_cloud.profile"
    || key === "opcua.security_policy"
    || key === "opcua.security_mode"
  ) {
    return String(value || "").toLowerCase();
  }
  return value;
}

function settingsResolveOnlineKey(key) {
  return SETTINGS_ONLINE_KEY_MAP[key] || key;
}

function settingsEnqueueSave(task) {
  const run = () => Promise.resolve().then(task);
  const pending = settingsSaveQueue.then(run, run);
  settingsSaveQueue = pending.catch(() => {});
  return pending;
}

function settingsNotifyRuntimeConfigUpdated(source) {
  document.dispatchEvent(new CustomEvent("ide-runtime-config-updated", {
    detail: {
      source: source || "settings",
    },
  }));
}

function settingsApplyDefaults() {
  settingsState.values = {};
  for (const field of settingsAllFields()) {
    settingsState.values[field.key] = field.default;
  }
}

function settingsReadIoDriverParam(ioConfig, driverName, paramName) {
  if (!ioConfig || typeof ioConfig !== "object") return undefined;
  const drivers = Array.isArray(ioConfig.drivers) ? ioConfig.drivers : [];
  const match = drivers.find((driver) => String(driver?.name || "").toLowerCase() === driverName);
  if (match && match.params && typeof match.params === "object" && !Array.isArray(match.params)) {
    return match.params[paramName];
  }
  if (String(ioConfig.driver || "").toLowerCase() === driverName) {
    const params = (ioConfig.params && typeof ioConfig.params === "object") ? ioConfig.params : {};
    return params[paramName];
  }
  return undefined;
}

function settingsMergeIoConfigValues(ioConfig) {
  for (const [key, binding] of Object.entries(SETTINGS_IO_BINDINGS)) {
    const field = settingsFieldByKey(key);
    const raw = settingsReadIoDriverParam(ioConfig, binding.driver, binding.param);
    if (raw !== undefined) {
      settingsState.values[key] = settingsNormalizeValue(field, raw);
    }
  }
  const safeStateField = settingsFieldByKey("io.safe_state_json");
  if (safeStateField) {
    settingsState.values["io.safe_state_json"] = settingsNormalizeValue(
      safeStateField,
      settingsNormalizeSafeState(ioConfig?.safe_state || []),
    );
  }
}

function settingsMergeRuntimeConfigValues(runtimeTomlText) {
  const sections = settingsTomlParseSections(runtimeTomlText);
  for (const [key, binding] of Object.entries(SETTINGS_RUNTIME_BINDINGS)) {
    const field = settingsFieldByKey(key);
    if (binding.format) {
      const rawValue = settingsTomlReadRawAssignment(runtimeTomlText, binding.section, binding.key);
      const decoded = settingsTomlDecodeBindingRaw(binding, rawValue);
      if (decoded !== undefined) {
        settingsState.values[key] = settingsNormalizeValue(field, decoded);
      }
      continue;
    }
    const sectionValues = sections[binding.section];
    if (!sectionValues || typeof sectionValues !== "object") continue;
    if (!Object.prototype.hasOwnProperty.call(sectionValues, binding.key)) continue;
    settingsState.values[key] = settingsNormalizeValue(field, sectionValues[binding.key]);
  }
}

function settingsMergeRuntimeControlValues(runtimeConfig) {
  if (!runtimeConfig || typeof runtimeConfig !== "object") return;
  for (const [runtimeKey, runtimeValue] of Object.entries(runtimeConfig)) {
    const key = SETTINGS_RUNTIME_CONTROL_KEY_MAP[runtimeKey] || runtimeKey;
    if (String(key).startsWith("io.")) continue;
    const field = settingsFieldByKey(key);
    if (!field) continue;
    settingsState.values[key] = settingsNormalizeValue(field, runtimeValue);
  }
}

function settingsSafeStateIsBitAddress(address) {
  return /^%[IQM]X\d+\.\d+$/i.test(String(address || "").trim());
}

function settingsNormalizeSafeStateValue(address, value) {
  const isBitAddress = settingsSafeStateIsBitAddress(address);
  if (value === true) return isBitAddress ? "TRUE" : "1";
  if (value === false) return isBitAddress ? "FALSE" : "0";
  const text = String(value ?? "").trim();
  if (!text) return isBitAddress ? "FALSE" : "0";
  const upper = text.toUpperCase();
  if (upper === "TRUE") return isBitAddress ? "TRUE" : "1";
  if (upper === "FALSE") return isBitAddress ? "FALSE" : "0";
  if (upper === "1") return isBitAddress ? "TRUE" : "1";
  if (upper === "0") return isBitAddress ? "FALSE" : "0";
  const typed = text.match(/^[A-Za-z_][A-Za-z0-9_]*\(([-+]?\d+)\)$/);
  if (typed) return typed[1];
  return text;
}

function settingsNormalizeSafeState(entries) {
  if (!Array.isArray(entries)) return [];
  return entries
    .map((entry) => {
      const address = String(entry?.address || "").trim();
      return {
        address,
        value: settingsNormalizeSafeStateValue(address, entry?.value),
      };
    })
    .filter((entry) => entry.address.length > 0);
}

function settingsParseSafeStateJsonOrThrow(value) {
  const parsed = settingsParseJsonTextOrThrow(value, "[]");
  if (!Array.isArray(parsed)) throw new Error("Expected JSON array");
  return settingsNormalizeSafeState(parsed);
}

// -- Runtime config snapshot I/O ---------------------------------------------

async function settingsLoadRuntimeConfigSnapshot() {
  if (settingsIsStandaloneIdeMode()) {
    const runtimeId = settingsResolveRuntimeScope();
    const configUiSnapshot = await settingsLoadRuntimeConfigFromConfigUi(runtimeId);
    if (configUiSnapshot) return configUiSnapshot;

    const snapshot = await settingsLoadRuntimeConfigFromIdeFile();
    if (snapshot) return snapshot;
    return {
      text: "",
      revision: null,
      runtimeId: runtimeId || null,
      fileVersion: null,
      source: null,
    };
  }

  const configUiSnapshot = await settingsLoadRuntimeConfigFromConfigUi();
  if (configUiSnapshot) {
    return configUiSnapshot;
  }

  const ideSnapshot = await settingsLoadRuntimeConfigFromIdeFile();
  if (ideSnapshot) {
    return ideSnapshot;
  }

  return {
    text: "",
    revision: null,
    runtimeId: null,
    fileVersion: null,
    source: null,
  };
}

async function settingsLoadRuntimeConfigFromConfigUi(runtimeId) {
  const runtime = String(runtimeId || "").trim();
  const query = runtime ? `?runtime_id=${encodeURIComponent(runtime)}` : "";
  try {
    const result = await apiJson(`/api/config-ui/runtime/config${query}`, {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 4000,
    });
    if (result && typeof result.text === "string") {
      return {
        text: result.text,
        revision: result.revision ? String(result.revision) : null,
        runtimeId: result.runtime_id || runtime || null,
        fileVersion: null,
        source: "config-ui",
      };
    }
  } catch {
    // handled by caller fallback
  }
  return null;
}

async function settingsLoadRuntimeConfigFromIdeFile() {
  const runtimePath = settingsRuntimeTomlPathForSelectedRuntime();
  try {
    const result = await apiJson(`/api/ide/file?path=${encodeURIComponent(runtimePath)}`, {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 3000,
    });
    return {
      text: typeof result?.content === "string" ? result.content : "",
      revision: null,
      runtimeId: null,
      fileVersion: Number.isFinite(result?.version) ? result.version : null,
      source: "ide-file",
    };
  } catch {
    return null;
  }
}

async function settingsWriteRuntimeConfigSnapshot(
  text,
  expectedRevision,
  runtimeId,
  expectedFileVersion,
  sourceHint,
) {
  const source = String(sourceHint || "").toLowerCase();
  if (settingsIsStandaloneIdeMode() && source !== "ide-file") {
    const scopedRuntime = String(runtimeId || settingsResolveRuntimeScope() || "").trim();
    return settingsWriteRuntimeConfigSnapshotConfigUi(text, expectedRevision, scopedRuntime);
  }
  if (source === "ide-file") {
    return settingsWriteRuntimeConfigSnapshotIdeFile(text, expectedFileVersion);
  }

  try {
    return await settingsWriteRuntimeConfigSnapshotConfigUi(text, expectedRevision, runtimeId);
  } catch (err) {
    const message = String(err?.message || err || "").toLowerCase();
    if (message.includes("/api/config-ui/runtime/config")) {
      throw err;
    }
    return settingsWriteRuntimeConfigSnapshotIdeFile(text, expectedFileVersion);
  }
}

async function settingsWriteRuntimeConfigSnapshotConfigUi(text, expectedRevision, runtimeId) {
  const payload = { text: String(text || "") };
  if (expectedRevision) payload.expected_revision = expectedRevision;
  if (runtimeId) payload.runtime_id = runtimeId;

  const result = await apiJson("/api/config-ui/runtime/config", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify(payload),
    timeoutMs: 6000,
  });
  return {
    revision: result?.revision ? String(result.revision) : null,
    runtimeId: result?.runtime_id || runtimeId || null,
    fileVersion: null,
    source: "config-ui",
  };
}

async function settingsWriteRuntimeConfigSnapshotIdeFile(text, expectedFileVersion) {
  const payload = {
    path: settingsRuntimeTomlPathForSelectedRuntime(),
    content: String(text || ""),
  };
  if (Number.isFinite(expectedFileVersion)) {
    payload.expected_version = expectedFileVersion;
  }
  const result = await apiJson("/api/ide/file", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify(payload),
    timeoutMs: 6000,
  });
  return {
    revision: null,
    runtimeId: null,
    fileVersion: Number.isFinite(result?.version) ? result.version : null,
    source: "ide-file",
  };
}

async function settingsLoadSimulationConfigSnapshot() {
  try {
    const result = await apiJson(`/api/ide/file?path=${encodeURIComponent("simulation.toml")}`, {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 3000,
    });
    return {
      text: typeof result?.content === "string" ? result.content : "",
      version: Number.isFinite(result?.version) ? result.version : null,
    };
  } catch {
    return {
      text: "",
      version: null,
    };
  }
}

async function settingsWriteSimulationConfigSnapshot(text, expectedVersion) {
  if (Number.isFinite(expectedVersion)) {
    try {
      const updated = await apiJson("/api/ide/file", {
        method: "POST",
        headers: apiHeaders(),
        body: JSON.stringify({
          path: "simulation.toml",
          expected_version: expectedVersion,
          content: String(text || ""),
        }),
        timeoutMs: 6000,
      });
      return Number.isFinite(updated?.version) ? updated.version : expectedVersion;
    } catch (err) {
      const message = String(err?.message || err || "").toLowerCase();
      const missingSource = message.includes("request failed (404)")
        || message.includes("source file not found");
      if (!missingSource) throw err;
    }
  }

  try {
    const created = await apiJson("/api/ide/fs/create", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify({
        path: "simulation.toml",
        kind: "file",
        content: String(text || ""),
      }),
      timeoutMs: 6000,
    });
    return Number.isFinite(created?.version) ? created.version : 1;
  } catch (err) {
    const message = String(err?.message || err || "").toLowerCase();
    const alreadyExists = message.includes("409") || message.includes("exists");
    if (!alreadyExists) throw err;
    const fresh = await settingsLoadSimulationConfigSnapshot();
    if (!Number.isFinite(fresh?.version)) throw err;
    const updated = await apiJson("/api/ide/file", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify({
        path: "simulation.toml",
        expected_version: fresh.version,
        content: String(text || ""),
      }),
      timeoutMs: 6000,
    });
    return Number.isFinite(updated?.version) ? updated.version : fresh.version;
  }
}

async function settingsPersistRuntimeValue(key, value) {
  const binding = SETTINGS_RUNTIME_BINDINGS[key];
  if (!binding) return;

  const field = settingsFieldByKey(key);

  for (let attempt = 0; attempt < 2; attempt += 1) {
    if (!settingsState.runtimeConfigText) {
      const snapshot = await settingsLoadRuntimeConfigSnapshot();
      settingsState.runtimeConfigText = snapshot.text;
      settingsState.runtimeRevision = snapshot.revision;
      settingsState.runtimeId = snapshot.runtimeId;
      settingsState.runtimeFileVersion = snapshot.fileVersion;
      settingsState.runtimeSnapshotSource = snapshot.source;
    }

    const formatted = settingsTomlEncodeBindingValue(binding, value)
      ?? settingsFormatTomlValue(field, value);
    const nextText = settingsTomlUpsert(
      settingsState.runtimeConfigText,
      binding.section,
      binding.key,
      formatted,
    );

    try {
      const saved = await settingsWriteRuntimeConfigSnapshot(
        nextText,
        settingsState.runtimeRevision,
        settingsState.runtimeId,
        settingsState.runtimeFileVersion,
        settingsState.runtimeSnapshotSource,
      );
      settingsState.runtimeConfigText = nextText;
      settingsState.runtimeRevision = saved.revision;
      settingsState.runtimeId = saved.runtimeId;
      settingsState.runtimeFileVersion = saved.fileVersion;
      settingsState.runtimeSnapshotSource = saved.source;
      settingsNotifyRuntimeConfigUpdated("settings");
      return;
    } catch (err) {
      if (attempt === 0 && settingsIsConflictError(err)) {
        const fresh = await settingsLoadRuntimeConfigSnapshot();
        settingsState.runtimeConfigText = fresh.text;
        settingsState.runtimeRevision = fresh.revision;
        settingsState.runtimeId = fresh.runtimeId;
        settingsState.runtimeFileVersion = fresh.fileVersion;
        settingsState.runtimeSnapshotSource = fresh.source;
        continue;
      }
      throw err;
    }
  }
}

function settingsMergeSimulationConfigValues(simulationTomlText) {
  const sections = settingsTomlParseSections(simulationTomlText);
  for (const [key, binding] of Object.entries(SETTINGS_SIMULATION_BINDINGS)) {
    const field = settingsFieldByKey(key);
    const sectionValues = sections[binding.section];
    if (!sectionValues || !Object.prototype.hasOwnProperty.call(sectionValues, binding.key)) {
      continue;
    }
    settingsState.values[key] = settingsNormalizeValue(field, sectionValues[binding.key]);
  }
}

async function settingsPersistSimulationValue(key, value) {
  const binding = SETTINGS_SIMULATION_BINDINGS[key];
  if (!binding) return;

  const field = settingsFieldByKey(key);
  if (settingsState.simulationVersion == null && !settingsState.simulationConfigText) {
    const snapshot = await settingsLoadSimulationConfigSnapshot();
    settingsState.simulationConfigText = snapshot.text;
    settingsState.simulationVersion = snapshot.version;
  }

  const formatted = settingsFormatTomlValue(field, value);
  const nextText = settingsTomlUpsert(
    settingsState.simulationConfigText,
    binding.section,
    binding.key,
    formatted,
  );
  const nextVersion = await settingsWriteSimulationConfigSnapshot(
    nextText,
    settingsState.simulationVersion,
  );
  settingsState.simulationConfigText = nextText;
  settingsState.simulationVersion = Number.isFinite(nextVersion) ? nextVersion : settingsState.simulationVersion;
}

// -- I/O config I/O ----------------------------------------------------------

function settingsFormatTomlRawValue(value) {
  if (value == null) return settingsFormatTomlString("");
  if (typeof value === "string") return settingsFormatTomlString(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return "0";
    return Number.isInteger(value) ? String(value) : String(value);
  }
  if (typeof value === "boolean") return value ? "true" : "false";
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    return `[${value.map((entry) => settingsFormatTomlRawValue(entry)).join(", ")}]`;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value)
      .filter(([key]) => String(key || "").trim().length > 0);
    if (entries.length === 0) return "{}";
    return `{ ${entries
      .map(([key, entryValue]) => `${String(key).trim()} = ${settingsFormatTomlRawValue(entryValue)}`)
      .join(", ")} }`;
  }
  return settingsFormatTomlString(String(value));
}

function settingsParseIoSafeStateBlocks(text) {
  const lines = String(text || "").replace(/\r\n/g, "\n").split("\n");
  const entries = [];
  let current = null;
  for (const lineRaw of lines) {
    const line = settingsTomlStripComment(lineRaw);
    if (!line) continue;
    if (/^\s*\[\[io\.safe_state\]\]\s*$/.test(line)) {
      if (current && current.address) {
        entries.push({
          address: String(current.address),
          value: current.value == null ? "FALSE" : String(current.value),
        });
      }
      current = {};
      continue;
    }
    if (/^\s*\[[^\]]+\]\s*$/.test(line)) {
      if (current && current.address) {
        entries.push({
          address: String(current.address),
          value: current.value == null ? "FALSE" : String(current.value),
        });
      }
      current = null;
      continue;
    }
    if (!current) continue;
    const eqIndex = line.indexOf("=");
    if (eqIndex <= 0) continue;
    const key = line.slice(0, eqIndex).trim();
    const valueRaw = line.slice(eqIndex + 1).trim();
    current[key] = settingsTomlParseValue(valueRaw);
  }
  if (current && current.address) {
    entries.push({
      address: String(current.address),
      value: current.value == null ? "FALSE" : String(current.value),
    });
  }
  return entries;
}

function settingsParseIoDriversFromText(text) {
  const driversRaw = settingsTomlReadRawAssignment(text, "io", "drivers");
  if (driversRaw && driversRaw.startsWith("[") && driversRaw.endsWith("]")) {
    const inner = driversRaw.slice(1, -1).trim();
    if (!inner) return [];
    return settingsTomlSplitTopLevel(inner)
      .map((entry) => settingsTomlParseInlineTable(entry))
      .filter((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
      .map((entry) => ({
        name: String(entry.name || "").trim(),
        params: (entry.params && typeof entry.params === "object" && !Array.isArray(entry.params))
          ? entry.params
          : {},
      }))
      .filter((entry) => entry.name.length > 0);
  }

  const sections = settingsTomlParseSections(text);
  const io = sections.io || {};
  const driverName = String(io.driver || "").trim();
  if (!driverName) return [];
  const params = (sections["io.params"] && typeof sections["io.params"] === "object")
    ? sections["io.params"]
    : {};
  return [{
    name: driverName,
    params,
  }];
}

function settingsParseIoTomlText(text) {
  const sections = settingsTomlParseSections(text);
  const io = sections.io || {};
  const drivers = settingsParseIoDriversFromText(text);
  const safeState = settingsParseIoSafeStateBlocks(text);
  return {
    drivers,
    driver: drivers[0]?.name || "simulated",
    params: drivers[0]?.params || {},
    safe_state: safeState,
    use_system_io: !!io.use_system_io,
  };
}

function settingsRenderIoTomlText(ioConfig) {
  const config = (ioConfig && typeof ioConfig === "object") ? ioConfig : {};
  const drivers = Array.isArray(config.drivers) ? config.drivers : [];
  const safeState = Array.isArray(config.safe_state) ? config.safe_state : [];

  const lines = ["[io]"];
  if (config.use_system_io) {
    lines.push("use_system_io = true");
  }
  lines.push("drivers = [");
  for (const driver of drivers) {
    const name = String(driver?.name || "").trim();
    if (!name) continue;
    const params = (driver && typeof driver.params === "object" && !Array.isArray(driver.params))
      ? driver.params
      : {};
    lines.push(`  { name = ${settingsFormatTomlString(name)}, params = ${settingsFormatTomlRawValue(params)} },`);
  }
  lines.push("]");

  for (const entry of safeState) {
    const address = String(entry?.address || "").trim();
    if (!address) continue;
    lines.push("");
    lines.push("[[io.safe_state]]");
    lines.push(`address = ${settingsFormatTomlString(address)}`);
    lines.push(`value = ${settingsFormatTomlString(String(entry?.value ?? "FALSE"))}`);
  }

  return `${lines.join("\n")}\n`;
}

function settingsIoConfigFromConfigUiPayload(payload, runtimeId) {
  if (!payload || typeof payload !== "object" || typeof payload.text !== "string") return null;
  const parsed = settingsParseIoTomlText(payload.text);
  parsed._source = "config-ui";
  parsed._revision = payload.revision ? String(payload.revision) : null;
  parsed._runtime_id = String(payload.runtime_id || runtimeId || "").trim() || null;
  parsed._text = payload.text;
  return parsed;
}

async function settingsLoadIoConfigFromConfigUi(runtimeId) {
  const runtime = String(runtimeId || "").trim();
  if (!runtime) return null;
  try {
    const result = await apiJson(
      `/api/config-ui/io/config?runtime_id=${encodeURIComponent(runtime)}`,
      {
        method: "GET",
        headers: apiHeaders(),
        timeoutMs: 4000,
      },
    );
    return settingsIoConfigFromConfigUiPayload(result, runtime);
  } catch {
    return null;
  }
}

async function settingsLoadIoConfig() {
  if (settingsIsStandaloneIdeMode()) {
    const runtime = settingsResolveRuntimeScope();
    const scopedConfig = await settingsLoadIoConfigFromConfigUi(runtime);
    if (scopedConfig) return scopedConfig;
  }

  try {
    return await apiJson("/api/ide/io/config", {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 3000,
    });
  } catch (err) {
    const message = String(err?.message || err || "").toLowerCase();
    if (!message.includes("request failed (404)")) {
      return null;
    }
    try {
      return await apiJson("/api/io/config", {
        method: "GET",
        timeoutMs: 3000,
      });
    } catch {
      return null;
    }
  }
}

async function settingsSaveIoConfig(payload) {
  if (settingsIsStandaloneIdeMode()) {
    const runtime = settingsResolveRuntimeScope();
    if (runtime) {
      const text = settingsRenderIoTomlText(payload);
      const body = {
        runtime_id: runtime,
        text,
      };
      if (settingsState.ioRevision) {
        body.expected_revision = settingsState.ioRevision;
      }
      const result = await apiJson("/api/config-ui/io/config", {
        method: "POST",
        headers: apiHeaders(),
        body: JSON.stringify(body),
        timeoutMs: 6000,
      });
      const parsed = settingsIoConfigFromConfigUiPayload({
        ...(result || {}),
        text,
      }, runtime);
      return parsed || {
        ...payload,
        _source: "config-ui",
        _revision: result?.revision ? String(result.revision) : null,
        _runtime_id: runtime,
        _text: text,
      };
    }
  }

  try {
    return await apiJson("/api/ide/io/config", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify(payload),
      timeoutMs: 5000,
    });
  } catch (err) {
    const message = String(err?.message || err || "").toLowerCase();
    if (!message.includes("request failed (404)")) throw err;
    return await apiJson("/api/io/config", {
      method: "POST",
      body: JSON.stringify(payload),
      timeoutMs: 5000,
    });
  }
}

function settingsBuildEditableIoDrivers(ioConfig, fallbackDriver) {
  const drivers = Array.isArray(ioConfig.drivers) && ioConfig.drivers.length > 0
    ? ioConfig.drivers.map((driver) => ({
      name: String(driver?.name || "").trim() || "simulated",
      params: (driver?.params && typeof driver.params === "object" && !Array.isArray(driver.params))
        ? { ...driver.params }
        : {},
    }))
    : [{
      name: String(ioConfig.driver || fallbackDriver).trim() || fallbackDriver,
      params: (ioConfig.params && typeof ioConfig.params === "object" && !Array.isArray(ioConfig.params))
        ? { ...ioConfig.params }
        : {},
    }];
  return drivers;
}

async function settingsApplyIoValue(key, value) {
  const binding = SETTINGS_IO_BINDINGS[key];
  const isSafeStateEdit = key === "io.safe_state_json";
  if (!binding && !isSafeStateEdit) return;

  let ioConfig = settingsState.ioConfig;
  if (!ioConfig) {
    ioConfig = await settingsLoadIoConfig();
  }
  if (!ioConfig || typeof ioConfig !== "object") {
    const fallbackDriver = binding?.driver || "simulated";
    ioConfig = {
      driver: fallbackDriver,
      params: {},
      drivers: [{ name: fallbackDriver, params: {} }],
      safe_state: [],
      use_system_io: false,
    };
  }

  const drivers = settingsBuildEditableIoDrivers(ioConfig, binding?.driver || "simulated");
  const safeState = isSafeStateEdit
    ? settingsParseSafeStateJsonOrThrow(value)
    : settingsNormalizeSafeState(ioConfig.safe_state);
  const useSystemIo = !!ioConfig.use_system_io;

  if (binding) {
    let driver = drivers.find((entry) => String(entry.name || "").toLowerCase() === binding.driver);
    if (!driver) {
      driver = { name: binding.driver, params: {} };
      drivers.push(driver);
    }
    if (!driver.params || typeof driver.params !== "object" || Array.isArray(driver.params)) {
      driver.params = {};
    }

    let coerced;
    if (binding.type === "number") {
      coerced = Number(value) || 0;
    } else if (binding.type === "toggle") {
      coerced = !!value;
    } else if (binding.type === "json-array") {
      const parsed = settingsParseJsonTextOrThrow(value, "[]");
      if (!Array.isArray(parsed)) {
        throw new Error(`Expected JSON array for ${binding.driver}.${binding.param}`);
      }
      coerced = parsed;
    } else if (binding.type === "json-object") {
      const parsed = settingsParseJsonTextOrThrow(value, "{}");
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new Error(`Expected JSON object for ${binding.driver}.${binding.param}`);
      }
      coerced = parsed;
    } else {
      coerced = String(value ?? "");
    }
    driver.params[binding.param] = coerced;
  }

  const payload = {
    drivers,
    safe_state: safeState,
    use_system_io: useSystemIo,
  };

  let savedResult = null;
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      savedResult = await settingsSaveIoConfig(payload);
      break;
    } catch (err) {
      if (attempt === 0 && settingsIsConflictError(err)) {
        const freshIo = await settingsLoadIoConfig();
        if (freshIo && typeof freshIo === "object") {
          settingsState.ioConfig = freshIo;
          settingsState.ioConfigText = String(freshIo._text || "");
          settingsState.ioRevision = freshIo._revision || null;
          settingsState.ioRuntimeId = freshIo._runtime_id || null;
        }
        continue;
      }
      throw err;
    }
  }

  const mergedIoConfig = {
    ...(ioConfig || {}),
    driver: drivers[0]?.name || binding?.driver || "simulated",
    params: drivers[0]?.params || {},
    drivers,
    safe_state: payload.safe_state,
    use_system_io: useSystemIo,
  };
  if (savedResult && typeof savedResult === "object") {
    settingsState.ioConfigText = String(savedResult._text || "");
    settingsState.ioRevision = savedResult._revision || null;
    settingsState.ioRuntimeId = savedResult._runtime_id || settingsResolveRuntimeScope() || null;
    settingsState.ioConfig = {
      ...mergedIoConfig,
      _source: savedResult._source || mergedIoConfig._source || null,
      _revision: settingsState.ioRevision,
      _runtime_id: settingsState.ioRuntimeId,
      _text: settingsState.ioConfigText,
    };
  } else {
    settingsState.ioConfig = mergedIoConfig;
  }

  if (settingsFieldByKey("io.safe_state_json")) {
    settingsState.values["io.safe_state_json"] = settingsNormalizeValue(
      settingsFieldByKey("io.safe_state_json"),
      payload.safe_state,
    );
  }

  document.dispatchEvent(new CustomEvent("ide-io-config-updated", {
    detail: {
      source: "settings",
    },
  }));
}

// -- Category sidebar --------------------------------------------------------

function settingsRenderCategories() {
  const container = el.settingsCategories;
  if (!container) return;
  container.innerHTML = "";

  const searchWrap = document.createElement("div");
  searchWrap.className = "settings-category-search-wrap";
  const searchInput = document.createElement("input");
  searchInput.type = "search";
  searchInput.className = "settings-category-search";
  searchInput.placeholder = "Filter settings (mqtt, plc, tls...)";
  searchInput.value = settingsState.searchQuery;
  searchInput.addEventListener("input", (event) => {
    settingsState.searchQuery = String(event?.target?.value || "");
    if (settingsState.activeCategory === "advanced") return;
    settingsRenderForm();
  });
  searchWrap.appendChild(searchInput);
  container.appendChild(searchWrap);

  for (const cat of SETTINGS_CATEGORIES) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "settings-category-btn";
    btn.textContent = cat.label;
    btn.dataset.category = cat.id;
    if (cat.id === settingsState.activeCategory) btn.classList.add("active");
    btn.addEventListener("click", () => settingsSelectCategory(cat.id));
    container.appendChild(btn);
  }
}

function settingsSelectCategory(catId, options) {
  const opts = options || {};
  settingsState.activeCategory = catId;
  settingsState.pendingFocusKey = typeof opts.focusKey === "string" ? opts.focusKey : null;
  if (opts.clearSearch) {
    settingsState.searchQuery = "";
  }
  settingsRenderCategories();

  if (catId === "advanced") {
    settingsRenderAdvanced();
  } else {
    settingsState.editingToml = false;
    settingsRenderForm();
  }
}

// -- Settings form -----------------------------------------------------------

function settingsRenderForm() {
  const panel = el.settingsFormPanel;
  if (!panel) return;

  const cat = SETTINGS_CATEGORIES.find((c) => c.id === settingsState.activeCategory);
  if (!cat) {
    panel.innerHTML = '<p class="muted" style="padding:16px;font-size:13px">Select a category to view settings.</p>';
    return;
  }

  const groups = settingsGroupsForCategory(cat);
  let html = settingsRenderRuntimeScopeBar();
  html += settingsRenderQuickActions(cat.id);
  html += `<h3 style="margin:0 0 12px;font-size:15px;font-weight:600;color:var(--text)">${escapeHtml(cat.label)}</h3>`;
  const normalizedQuery = settingsNormalizeSearchQuery(settingsState.searchQuery);
  const visibleCount = settingsVisibleFieldCount(groups);
  const totalCount = settingsCategoryTotalFields(cat);
  if (normalizedQuery) {
    html += '<section class="settings-filter-summary">';
    html += `<span>Filter active: <strong>${escapeHtml(settingsState.searchQuery)}</strong> (${visibleCount} / ${totalCount})</span>`;
    html += '<button type="button" class="settings-filter-clear" data-settings-clear-filter>Clear</button>';
    html += "</section>";
  }

  if (groups.length === 0) {
    const note = normalizedQuery
      ? `No settings match "${escapeHtml(settingsState.searchQuery)}".`
      : "No settings available in this category.";
    html += `<section class="settings-subsection"><p class="muted" style="margin:0;font-size:12px">${note}</p></section>`;
  } else {
    for (const group of groups) {
      if (group.label) {
        html += `<section class="settings-subsection" data-settings-group="${escapeAttr(group.id)}">`;
        html += `<header class="settings-subsection-head"><h4>${escapeHtml(group.label)}</h4>`;
        if (group.note) {
          html += `<p class="settings-subsection-note">${escapeHtml(group.note)}</p>`;
        }
        html += "</header>";
      }

      for (const field of group.fields) {
        const value = settingsState.values[field.key] ?? field.default;
        html += settingsRenderField(field, value);
      }

      if (group.label) {
        html += "</section>";
      }
    }
  }

  html += '<div id="settingsRestartWarn" class="settings-restart-warn" hidden></div>';
  panel.innerHTML = html;
  settingsBindRuntimeScope(panel);
  settingsBindQuickActions(panel);
  const clearFilterBtn = panel.querySelector("[data-settings-clear-filter]");
  if (clearFilterBtn) {
    clearFilterBtn.addEventListener("click", () => {
      settingsState.searchQuery = "";
      settingsRenderCategories();
      settingsRenderForm();
    });
  }

  panel.querySelectorAll("[data-settings-key]").forEach((input) => {
    input.addEventListener("change", (event) => {
      const key = event.target.dataset.settingsKey;
      const field = settingsFieldByKey(key);
      const nextValue = (field && field.type === "toggle")
        ? event.target.checked
        : event.target.value;
      void settingsApplyValue(key, nextValue);
    });
  });

  settingsFocusPendingField(panel);
}

async function settingsApplyValue(key, value, options) {
  const opts = options || {};
  const field = settingsFieldByKey(key);
  const normalized = settingsNormalizeValue(field, value);
  settingsState.values[key] = normalized;

  return settingsEnqueueSave(async () => {
    try {
      const ioBacked = Object.prototype.hasOwnProperty.call(SETTINGS_IO_BINDINGS, key)
        || SETTINGS_IO_GLOBAL_KEYS.has(key);
      if (ioBacked) {
        await settingsApplyIoValue(key, normalized);
        settingsSetRestartWarning("Saved to io.toml. Restart required for this change to take full effect.");
        if (!opts.silent && typeof showIdeToast === "function") {
          showIdeToast(`Saved ${field?.label || key}`, "success");
        }
        return;
      }

      const simulationBacked = Object.prototype.hasOwnProperty.call(SETTINGS_SIMULATION_BINDINGS, key);
      if (simulationBacked) {
        await settingsPersistSimulationValue(key, normalized);
        settingsSetRestartWarning("Saved to simulation.toml. Restart required for this change to take full effect.");
        if (!opts.silent && typeof showIdeToast === "function") {
          showIdeToast(`Saved ${field?.label || key}`, "success");
        }
        return;
      }

      await settingsPersistRuntimeValue(key, normalized);

      let restartRequired = SETTINGS_RESTART_REQUIRED_KEYS.has(key);
      if (onlineState && onlineState.connected && SETTINGS_ONLINE_KEYS.has(key)) {
        const params = {};
        params[settingsResolveOnlineKey(key)] = settingsBuildConfigSetValue(key, normalized);
        try {
          const result = await runtimeControlRequest({
            id: 1,
            type: "config.set",
            params,
          }, { timeoutMs: 3000 });
          const restartList = Array.isArray(result?.restart_required) ? result.restart_required : [];
          if (restartList.length > 0) restartRequired = true;
        } catch (onlineErr) {
          if (!settingsIsUnsupportedOnlineKeyError(onlineErr)) {
            throw onlineErr;
          }
          restartRequired = true;
        }
      }

      if (restartRequired) {
        settingsSetRestartWarning("Saved to runtime.toml. Restart required for this change to take full effect.");
      } else {
        settingsClearRestartWarning();
      }

      if (!opts.silent && typeof showIdeToast === "function") {
        showIdeToast(`Saved ${field?.label || key}`, "success");
      }
    } catch (err) {
      if (!opts.silent && typeof showIdeToast === "function") {
        showIdeToast(`Failed to save ${field?.label || key}: ${err.message || err}`, "error");
      }
    }
  });
}

// -- Load settings -----------------------------------------------------------

async function settingsLoad() {
  settingsApplyDefaults();
  settingsState.ioConfig = null;
  settingsState.ioConfigText = "";
  settingsState.ioRevision = null;
  settingsState.ioRuntimeId = null;
  settingsState.runtimeRevision = null;
  settingsState.runtimeId = null;
  settingsState.runtimeFileVersion = null;
  settingsState.runtimeSnapshotSource = null;
  settingsState.loadedRuntimeScope = "";

  await settingsLoadRuntimeTargets();

  const runtimeSnapshot = await settingsLoadRuntimeConfigSnapshot();
  settingsState.runtimeConfigText = runtimeSnapshot.text;
  settingsState.runtimeRevision = runtimeSnapshot.revision;
  settingsState.runtimeId = runtimeSnapshot.runtimeId;
  settingsState.runtimeFileVersion = runtimeSnapshot.fileVersion;
  settingsState.runtimeSnapshotSource = runtimeSnapshot.source;
  if (settingsIsStandaloneIdeMode() && runtimeSnapshot.runtimeId) {
    settingsSetSelectedRuntimeId(runtimeSnapshot.runtimeId, {
      broadcast: false,
    });
  }

  if (runtimeSnapshot.text) {
    settingsMergeRuntimeConfigValues(runtimeSnapshot.text);
  }

  const simulationSnapshot = await settingsLoadSimulationConfigSnapshot();
  settingsState.simulationConfigText = simulationSnapshot.text;
  settingsState.simulationVersion = simulationSnapshot.version;
  if (simulationSnapshot.text) {
    settingsMergeSimulationConfigValues(simulationSnapshot.text);
  }

  if (onlineState && onlineState.connected) {
    try {
      const runtimeConfig = await runtimeControlRequest({
        id: 1,
        type: "config.get",
      }, { timeoutMs: 3000 });
      settingsState.runtimeControlSnapshot = runtimeConfig;
      settingsMergeRuntimeControlValues(runtimeConfig);
    } catch {
      settingsState.runtimeControlSnapshot = null;
      // Keep offline runtime.toml values when runtime config cannot be fetched.
    }
  } else {
    settingsState.runtimeControlSnapshot = null;
  }

  try {
    const ioConfig = await settingsLoadIoConfig();
    if (ioConfig) {
      settingsState.ioConfig = ioConfig;
      settingsState.ioConfigText = String(ioConfig._text || "");
      settingsState.ioRevision = ioConfig._revision || null;
      settingsState.ioRuntimeId = ioConfig._runtime_id || settingsResolveRuntimeScope() || null;
      settingsMergeIoConfigValues(ioConfig);
    }
  } catch {
    // Keep defaults when io.toml cannot be loaded.
  }

  settingsState.loaded = true;
  settingsState.loadedRuntimeScope = settingsResolveRuntimeScope();
  if (settingsState.activeCategory === "advanced") {
    settingsRenderAdvanced();
  } else {
    settingsRenderForm();
  }
}

// -- Advanced panel ----------------------------------------------------------

function settingsRenderAdvanced() {
  const panel = el.settingsFormPanel;
  if (!panel) return;

  let html = settingsRenderRuntimeScopeBar();
  html += settingsRenderQuickActions("advanced");
  html += '<h3 style="margin:0 0 12px;font-size:15px;font-weight:600;color:var(--text)">Advanced</h3>';
  html += '<div style="display:flex;flex-direction:column;gap:6px">';
  html += '<button type="button" class="btn secondary" id="settingsEditTomlBtn">Edit runtime.toml</button>';
  html += '<button type="button" class="btn ghost" id="settingsExportBtn">Export Settings</button>';
  html += '<button type="button" class="btn ghost" id="settingsImportBtn">Import Settings</button>';
  html += '<button type="button" class="btn ghost" id="settingsResetBtn" style="color:var(--danger)">Reset to Defaults</button>';
  html += '<input id="settingsImportInput" type="file" accept="application/json,.json" hidden/>';
  html += '</div>';
  html += `<section class="settings-subsection" style="margin-top:12px">
    <p class="muted" style="margin:0;font-size:12px">
      runtime.toml opens in the main code editor for full syntax support.
      Use the toolbar button "Back to form view" to return here.
    </p>
  </section>`;
  html += settingsRenderRuntimeControlSnapshotCard();

  panel.innerHTML = html;
  settingsBindRuntimeScope(panel);
  settingsBindQuickActions(panel);

  const editBtn = document.getElementById("settingsEditTomlBtn");
  if (editBtn) editBtn.addEventListener("click", () => { void settingsOpenToml(); });

  const exportBtn = document.getElementById("settingsExportBtn");
  if (exportBtn) exportBtn.addEventListener("click", settingsExport);

  const importBtn = document.getElementById("settingsImportBtn");
  const importInput = document.getElementById("settingsImportInput");
  if (importBtn && importInput) {
    importBtn.addEventListener("click", () => {
      importInput.click();
    });
    importInput.addEventListener("change", () => {
      void settingsImportFromFile(importInput);
    });
  }

  const resetBtn = document.getElementById("settingsResetBtn");
  if (resetBtn) resetBtn.addEventListener("click", () => { void settingsReset(); });
}

async function settingsOpenToml() {
  try {
    const snapshot = await settingsLoadRuntimeConfigSnapshot();
    settingsState.runtimeConfigText = snapshot.text;
    settingsState.runtimeRevision = snapshot.revision;
    settingsState.runtimeId = snapshot.runtimeId;
    settingsState.runtimeFileVersion = snapshot.fileVersion;
    settingsState.runtimeSnapshotSource = snapshot.source;
    settingsState.tomlOpenedFromSettings = true;
    settingsSyncBackToFormButton();
    if (typeof switchIdeTab === "function") {
      switchIdeTab("code");
    }
    if (typeof openFile === "function") {
      await openFile(settingsRuntimeTomlPathForSelectedRuntime());
    } else {
      throw new Error("Code editor is not ready yet");
    }
    setStatus("Editing runtime.toml in Code tab.");
  } catch (err) {
    settingsState.tomlOpenedFromSettings = false;
    settingsSyncBackToFormButton();
    if (typeof showIdeToast === "function") showIdeToast(`Failed to open runtime.toml: ${err.message || err}`, "error");
  }
}

function settingsBackToFormView() {
  settingsState.tomlOpenedFromSettings = false;
  settingsState.activeCategory = "advanced";
  settingsState.pendingFocusKey = null;
  settingsSyncBackToFormButton();
  if (typeof switchIdeTab === "function") {
    switchIdeTab("settings");
  }
  settingsSelectCategory("advanced");
}

function settingsSyncBackToFormButton() {
  if (!el.settingsBackToFormBtn) return;
  const activeTab = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : (document.querySelector(".ide-tab-btn.active")?.dataset?.tab || "code");
  const activePath = String(state?.activePath || "").trim();
  const expectedPath = settingsRuntimeTomlPathForSelectedRuntime();
  const show = settingsState.tomlOpenedFromSettings && activeTab === "code" && activePath === expectedPath;
  el.settingsBackToFormBtn.hidden = !show;
}

function settingsExport() {
  const text = JSON.stringify(settingsState.values, null, 2);
  const blob = new Blob([text], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "trust-settings.json";
  link.click();
  URL.revokeObjectURL(url);
}

async function settingsImportFromFile(fileInput) {
  const input = fileInput;
  const file = input?.files && input.files.length > 0 ? input.files[0] : null;
  if (!file) return;

  try {
    const text = await file.text();
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("Imported file must be a JSON object");
    }

    let applied = 0;
    let failed = 0;
    for (const field of settingsAllFields()) {
      if (!Object.prototype.hasOwnProperty.call(parsed, field.key)) continue;
      try {
        await settingsApplyValue(field.key, parsed[field.key], { silent: true });
        applied += 1;
      } catch {
        failed += 1;
      }
    }

    if (applied === 0) {
      throw new Error("No known settings keys found in import file");
    }
    if (settingsState.activeCategory === "advanced") {
      settingsRenderAdvanced();
    } else {
      settingsRenderForm();
    }
    if (typeof showIdeToast === "function") {
      const suffix = failed > 0 ? ` (${failed} keys failed)` : "";
      showIdeToast(`Imported ${applied} settings${suffix}.`, failed > 0 ? "warn" : "success");
    }
  } catch (err) {
    if (typeof showIdeToast === "function") {
      showIdeToast(`Import failed: ${err.message || err}`, "error");
    }
  } finally {
    if (input) input.value = "";
  }
}

async function settingsReset() {
  const proceed = await ideConfirm("Reset Settings", "Reset all settings to defaults?");
  if (!proceed) return;

  settingsApplyDefaults();
  for (const field of settingsAllFields()) {
    await settingsApplyValue(field.key, field.default, { silent: true });
  }

  if (settingsState.activeCategory === "advanced") {
    settingsRenderAdvanced();
  } else {
    settingsRenderForm();
  }

  if (typeof showIdeToast === "function") showIdeToast("Settings reset to defaults.", "success");
}

function settingsFormatRuntimeControlValue(value) {
  if (value == null) return "null";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function settingsRenderRuntimeControlSnapshotCard() {
  const snapshot = settingsState.runtimeControlSnapshot;
  let html = '<section class="settings-runtime-snapshot card" style="margin-top:12px;padding:12px">';
  html += '<h4 style="margin:0 0 8px;font-size:13px;color:var(--text)">Runtime State (Read-only)</h4>';
  if (!snapshot || typeof snapshot !== "object") {
    html += '<p class="muted" style="margin:0;font-size:12px">Connect to a runtime to inspect live config/status fields.</p>';
    html += "</section>";
    return html;
  }

  const rows = Object.entries(snapshot)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => {
      return `<tr>
        <td class="mono" style="font-size:11px;vertical-align:top;padding:4px 6px 4px 0">${escapeHtml(key)}</td>
        <td class="mono" style="font-size:11px;vertical-align:top;padding:4px 0;word-break:break-word">${escapeHtml(settingsFormatRuntimeControlValue(value))}</td>
      </tr>`;
    })
    .join("");
  html += `<div style="max-height:240px;overflow:auto"><table class="data-table" style="font-size:11px"><tbody>${rows}</tbody></table></div>`;
  html += "</section>";
  return html;
}

// -- Init and tab changes ----------------------------------------------------

function settingsInit() {
  settingsRenderCategories();
  settingsSyncBackToFormButton();
  if (el.settingsBackToFormBtn && !el.settingsBackToFormBtn.dataset.settingsBound) {
    el.settingsBackToFormBtn.dataset.settingsBound = "1";
    el.settingsBackToFormBtn.addEventListener("click", settingsBackToFormView);
  }
}

function settingsActivate() {
  settingsInit();
  const runtimeScope = settingsResolveRuntimeScope();
  const scopeChanged = settingsIsStandaloneIdeMode()
    && settingsState.loadedRuntimeScope !== runtimeScope;
  if (!settingsState.loaded || scopeChanged) {
    void settingsLoad();
    return;
  }
  if (settingsState.activeCategory === "advanced") {
    settingsRenderAdvanced();
  } else {
    settingsRenderForm();
  }
}

document.addEventListener("ide-tab-change", (event) => {
  if (event.detail?.tab === "settings") {
    settingsActivate();
  }
  settingsSyncBackToFormButton();
});

document.addEventListener("ide-active-path-change", () => {
  settingsSyncBackToFormButton();
});

document.addEventListener("ide-runtime-connected", () => {
  settingsState.loaded = false;
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "settings") {
    void settingsLoad();
  }
});

document.addEventListener("ide-runtime-disconnected", () => {
  settingsState.runtimeControlSnapshot = null;
  settingsState.loaded = false;
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "settings") {
    void settingsLoad();
  }
});

document.addEventListener("ide-project-changed", () => {
  settingsState.tomlOpenedFromSettings = false;
  settingsSyncBackToFormButton();
  settingsState.loaded = false;
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "settings") {
    void settingsLoad();
  }
});

document.addEventListener(SETTINGS_RUNTIME_SELECTION_EVENT, (event) => {
  const runtimeId = String(event?.detail?.runtimeId || "").trim();
  if (!runtimeId) return;
  if (!settingsIsStandaloneIdeMode()) return;
  const changed = settingsSetSelectedRuntimeId(runtimeId, {
    source: event?.detail?.source || "external",
    broadcast: false,
  });
  if (!changed) return;
  settingsState.loaded = false;
  settingsState.runtimeControlSnapshot = null;
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "settings") {
    void settingsLoad();
  }
});

document.addEventListener("ide-settings-focus-request", (event) => {
  const runtimeId = String(event?.detail?.runtimeId || "").trim();
  if (runtimeId && settingsIsStandaloneIdeMode()) {
    const changed = settingsSetSelectedRuntimeId(runtimeId, {
      source: event?.detail?.source || "external",
      broadcast: false,
    });
    if (changed) {
      settingsState.loaded = false;
      settingsState.runtimeControlSnapshot = null;
    }
  }

  const category = String(event?.detail?.category || "").trim();
  const key = String(event?.detail?.key || "").trim();
  if (category && settingsCategoryById(category)) {
    settingsState.activeCategory = category;
  }
  settingsState.pendingFocusKey = key || null;
  if (settingsState.pendingFocusKey) {
    settingsState.searchQuery = "";
  }

  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "settings") {
    settingsActivate();
  }
});

function settingsSyncInitialTabActivation(retryCount) {
  const attempts = Number(retryCount) || 0;
  if (typeof el !== "object" || !el || !el.settingsCategories || !el.settingsFormPanel) {
    if (attempts < 80) {
      setTimeout(() => settingsSyncInitialTabActivation(attempts + 1), 25);
    }
    return;
  }

  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "settings" || window.location.pathname.startsWith("/ide/settings")) {
    settingsActivate();
  } else {
    settingsSyncBackToFormButton();
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    setTimeout(() => settingsSyncInitialTabActivation(0), 0);
  });
} else {
  setTimeout(() => settingsSyncInitialTabActivation(0), 0);
}
