// ide-hardware.js — Hardware tab: Cytoscape canvas, component palette, module
// property editor, I/O address map table, and live I/O values.
// Implements US-4.1 through US-4.5 from web-ui-redesign-user-stories.md.

// ── Constants ──────────────────────────────────────────

const HW_PALETTE_CATEGORIES = [
  {
    label: "Controllers",
    items: [
      { type: "cpu", label: "CPU Module", icon: "cpu", meta: "Runtime controller", driver: null },
    ],
  },
  {
    label: "Digital I/O",
    items: [
      { type: "di-8", label: "DI 8ch", icon: "di", meta: "8 digital inputs", driver: "gpio", channels: 8, direction: "input" },
      { type: "di-16", label: "DI 16ch", icon: "di", meta: "16 digital inputs", driver: "gpio", channels: 16, direction: "input" },
      { type: "do-8", label: "DO 8ch", icon: "do", meta: "8 digital outputs", driver: "gpio", channels: 8, direction: "output" },
      { type: "do-16", label: "DO 16ch", icon: "do", meta: "16 digital outputs", driver: "gpio", channels: 16, direction: "output" },
    ],
  },
  {
    label: "Analog I/O",
    items: [
      { type: "ai-4", label: "AI 4ch", icon: "ai", meta: "4 analog inputs", driver: "simulated", channels: 4, direction: "input" },
      { type: "ai-8", label: "AI 8ch", icon: "ai", meta: "8 analog inputs", driver: "simulated", channels: 8, direction: "input" },
      { type: "ao-4", label: "AO 4ch", icon: "ao", meta: "4 analog outputs", driver: "simulated", channels: 4, direction: "output" },
    ],
  },
  {
    label: "Communication",
    items: [
      { type: "modbus-tcp", label: "Modbus TCP", icon: "modbus", meta: "PLC register bridge", driver: "modbus-tcp", channels: 16, direction: "input" },
      { type: "mqtt-bridge", label: "MQTT Bridge", icon: "mqtt", meta: "Brokered pub/sub bridge", driver: "mqtt", channels: 8, direction: "input" },
      { type: "opcua", label: "OPC UA", icon: "opcua", meta: "Industrial namespace bridge", driver: "opcua", channels: 8, direction: "input" },
    ],
  },
  {
    label: "Fieldbus",
    items: [
      { type: "ethercat", label: "EtherCAT Coupler", icon: "ethercat", meta: "Deterministic fieldbus coupler", driver: "ethercat", channels: 16, direction: "input" },
    ],
  },
];

const HW_ICON_PATHS = Object.freeze({
  cpu: '<rect x="1.9" y="1.9" width="12.2" height="12.2" rx="2.6" fill="currentColor" fill-opacity="0.11"/><rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2.2"/><rect x="5.05" y="5.05" width="5.9" height="5.9" rx="1.45" fill="currentColor" fill-opacity="0.18"/><path d="M5.8 1.05v1.7M10.2 1.05v1.7M5.8 13.25v1.7M10.2 13.25v1.7M1.05 5.8h1.7M1.05 10.2h1.7M13.25 5.8h1.7M13.25 10.2h1.7"/><path d="M6.45 7.95h3.1M7.95 6.45v3.1"/>',
  di: '<rect x="2.1" y="2.1" width="11.8" height="11.8" rx="2.45" fill="currentColor" fill-opacity="0.08"/><path d="M4.9 4.9h6.3M4.9 8h6.3M4.9 11.1h6.3"/><circle cx="3.55" cy="4.9" r="0.58" fill="currentColor"/><circle cx="3.55" cy="8" r="0.58" fill="currentColor"/><circle cx="3.55" cy="11.1" r="0.58" fill="currentColor"/><path d="M12.2 4.2v1.4M12.2 7.3v1.4M12.2 10.4v1.4"/>',
  do: '<rect x="2.1" y="2.1" width="11.8" height="11.8" rx="2.45" fill="currentColor" fill-opacity="0.08"/><path d="M4.6 5h4.7M4.6 8h4.7M4.6 11h4.7"/><path d="M9.9 4l2.25 1.04L9.9 6.08M9.9 7.05l2.25 1.04L9.9 9.13M9.9 10.1l2.25 1.04L9.9 12.18"/><circle cx="3.55" cy="5" r="0.5" fill="currentColor"/><circle cx="3.55" cy="8" r="0.5" fill="currentColor"/><circle cx="3.55" cy="11" r="0.5" fill="currentColor"/>',
  ai: '<rect x="2.1" y="2.1" width="11.8" height="11.8" rx="2.45" fill="currentColor" fill-opacity="0.08"/><path d="M4.05 11.4l2.08-2.45 1.88 1.58 3.84-4.92"/><path d="M4.05 12.25h8"/><path d="M4.05 5.25h2.05M6.95 5.25h1.12"/>',
  ao: '<rect x="2.1" y="2.1" width="11.8" height="11.8" rx="2.45" fill="currentColor" fill-opacity="0.08"/><path d="M4.25 11.1h2.95M7.2 11.1V7.1M7.2 7.1h2.95M10.15 7.1V4.85"/><path d="M10.15 4.85l1.78 1.1M10.15 4.85l1.78-1.1"/><circle cx="3.6" cy="11.1" r="0.55" fill="currentColor"/>',
  modbus: '<rect x="2.1" y="2.1" width="11.8" height="11.8" rx="2.45" fill="currentColor" fill-opacity="0.08"/><rect x="4.3" y="4.2" width="2.25" height="2.25" rx="0.58"/><rect x="7.15" y="4.2" width="2.25" height="2.25" rx="0.58"/><rect x="10" y="4.2" width="1.65" height="2.25" rx="0.5"/><rect x="4.3" y="7.25" width="2.25" height="2.25" rx="0.58"/><rect x="7.15" y="7.25" width="2.25" height="2.25" rx="0.58"/><rect x="10" y="7.25" width="1.65" height="2.25" rx="0.5"/><path d="M4.3 11.2h7.35"/>',
  mqtt: '<path d="M2.75 10.95a5.25 5.25 0 0 1 10.5 0"/><path d="M4.8 10.95a3.2 3.2 0 0 1 6.4 0"/><circle cx="8" cy="10.95" r="1.35" fill="currentColor" fill-opacity="0.2"/><circle cx="8" cy="10.95" r="0.78" fill="currentColor"/><path d="M8 4.35v1.65"/><path d="M6.2 3.45h3.6"/>',
  opcua: '<circle cx="4.5" cy="5" r="1.5" fill="currentColor" fill-opacity="0.14"/><circle cx="11.5" cy="5" r="1.5" fill="currentColor" fill-opacity="0.14"/><circle cx="8" cy="11.1" r="1.5" fill="currentColor" fill-opacity="0.14"/><path d="M6.05 5.95L7.2 9.1M9.95 5.95L8.8 9.1M6.3 5h3.4"/><circle cx="4.5" cy="5" r="0.5" fill="currentColor"/><circle cx="11.5" cy="5" r="0.5" fill="currentColor"/><circle cx="8" cy="11.1" r="0.5" fill="currentColor"/>',
  ethercat: '<path d="M2.7 8h2.45l1.55-2.15L8.9 10.1l1.95-2.75h2.45"/><circle cx="2.7" cy="8" r="1.02" fill="currentColor" fill-opacity="0.16"/><circle cx="13.3" cy="8" r="1.02" fill="currentColor" fill-opacity="0.16"/><path d="M6.7 5.85l2.2 4.25"/><path d="M5.15 12h5.7"/>',
  runtime: '<rect x="2.1" y="2.45" width="11.8" height="3.7" rx="1.35" fill="currentColor" fill-opacity="0.12"/><rect x="2.1" y="7.05" width="11.8" height="3.7" rx="1.35" fill="currentColor" fill-opacity="0.08"/><rect x="2.1" y="11.65" width="11.8" height="1.95" rx="0.95" fill="currentColor" fill-opacity="0.12"/><path d="M4.15 4.3h1.95M4.15 8.9h1.95M4.15 12.6h1.95"/>',
  endpoint: '<circle cx="8" cy="8" r="5.2" fill="currentColor" fill-opacity="0.09"/><circle cx="8" cy="8" r="4.95"/><path d="M8 3.15v9.7M3.15 8h9.7"/><circle cx="8" cy="8" r="1.2" fill="currentColor" fill-opacity="0.16"/>',
  mesh: '<circle cx="4" cy="8" r="1.12" fill="currentColor" fill-opacity="0.2"/><circle cx="8" cy="4" r="1.12" fill="currentColor" fill-opacity="0.2"/><circle cx="12" cy="8" r="1.12" fill="currentColor" fill-opacity="0.2"/><circle cx="8" cy="12" r="1.12" fill="currentColor" fill-opacity="0.2"/><path d="M4.9 7.1l2.2-2.2M8.9 4.9l2.2 2.2M11.1 8.9l-2.2 2.2M7.1 11.1l-2.2-2.2M4.95 8h6.1"/>',
  discovery: '<circle cx="8" cy="8" r="5.05"/><circle cx="8" cy="8" r="3.1"/><circle cx="8" cy="8" r="1.2" fill="currentColor"/><path d="M8 2.35v1.55M8 12.1v1.55M2.35 8h1.55M12.1 8h1.55"/>',
  web: '<circle cx="8" cy="8" r="5.45" fill="currentColor" fill-opacity="0.07"/><circle cx="8" cy="8" r="5.1"/><path d="M2.9 8h10.2M8 2.9c1.68 1.7 1.68 8.5 0 10.2M8 2.9c-1.68 1.7-1.68 8.5 0 10.2M4.55 5.2h6.9M4.55 10.8h6.9"/><path d="M3.7 4.2l1 1M12.3 4.2l-1 1"/>',
  external: '<rect x="2.25" y="2.15" width="8.85" height="11.7" rx="1.9" fill="currentColor" fill-opacity="0.08"/><path d="M6.9 8h6M10.7 5.7L13 8l-2.3 2.3"/><path d="M5.05 5.05h2.6M5.05 10.95h2.6"/><path d="M3.45 3.95h6.4"/>',
});

const HW_ICON_CACHE = new Map();
const HW_NODE_CARD_CACHE = new Map();

const HW_PRESETS = [
  { label: "Raspberry Pi GPIO", driver: "gpio", modules: [{ type: "cpu" }, { type: "di-8", driver: "gpio" }, { type: "do-8", driver: "gpio" }] },
  { label: "Modbus TCP", driver: "modbus-tcp", modules: [{ type: "cpu" }, { type: "modbus-tcp" }] },
  { label: "EtherCAT", driver: "ethercat", modules: [{ type: "cpu" }, { type: "ethercat" }] },
  { label: "Simulated", driver: "simulated", modules: [{ type: "cpu" }, { type: "di-8", driver: "simulated" }, { type: "do-8", driver: "simulated" }] },
];

const HW_COMM_DRIVERS = new Set([
  "modbus-tcp",
  "mqtt",
  "opcua",
  "mesh",
  "discovery",
  "web",
  "cloud",
  "cloud-wan",
  "cloud-links",
  "ethercat",
  "control",
  "tls",
  "deploy",
  "watchdog",
  "fault",
  "retain",
  "observability",
]);
const HW_RUNTIME_COMM_SECTIONS = [
  { section: "runtime.control", label: "Runtime Control", id: "control", keys: ["mode", "debug_enabled", "endpoint"] },
  { section: "runtime.mesh", label: "Mesh Network", id: "mesh", keys: ["role", "listen", "tls", "connect", "publish", "subscribe", "zenohd_version"] },
  { section: "runtime.discovery", label: "Discovery", id: "discovery", keys: ["enabled", "service_name", "advertise", "host_group", "interfaces"] },
  { section: "runtime.opcua", label: "OPC UA", id: "opcua", keys: ["enabled", "listen", "endpoint_path", "security_policy", "security_mode"] },
  { section: "runtime.web", label: "Web API", id: "web", keys: ["enabled", "listen", "tls", "auth"] },
  { section: "runtime.tls", label: "TLS", id: "tls", keys: ["mode", "require_remote", "cert_path", "key_path", "ca_path"] },
  { section: "runtime.deploy", label: "Deploy Security", id: "deploy", keys: ["require_signed", "keyring_path"] },
  { section: "runtime.watchdog", label: "Watchdog", id: "watchdog", keys: ["enabled", "timeout_ms", "action"] },
  { section: "runtime.fault", label: "Fault Policy", id: "fault", keys: ["policy"] },
  { section: "runtime.retain", label: "Retention", id: "retain", keys: ["mode", "path", "save_interval_ms"] },
  { section: "runtime.observability", label: "Observability", id: "observability", keys: ["enabled", "sample_interval_ms", "mode", "prometheus_enabled"] },
  { section: "runtime.cloud", label: "Cloud Profile", id: "cloud", keys: ["profile"] },
];
const HW_RUNTIME_LINK_TRANSPORTS = Object.freeze([
  { id: "realtime", label: "Realtime" },
  { id: "zenoh", label: "Zenoh" },
  { id: "mesh", label: "Mesh" },
  { id: "mqtt", label: "MQTT" },
  { id: "modbus-tcp", label: "Modbus TCP" },
  { id: "opcua", label: "OPC UA" },
  { id: "discovery", label: "Discovery" },
  { id: "web", label: "Web API" },
]);
const HW_RUNTIME_LINK_TRANSPORT_IDS = new Set(
  HW_RUNTIME_LINK_TRANSPORTS.map((entry) => entry.id),
);
const HW_RUNTIME_LINK_TRANSPORT_NOTES = Object.freeze({
  realtime: "Deterministic low-latency runtime-to-runtime lane for time-critical control.",
  zenoh: "Brokerless pub/sub routing for distributed runtime data and control.",
  mesh: "Generic mesh peer routing over runtime mesh endpoints.",
  mqtt: "Broker-routed runtime bridge for cloud and external integrations.",
  "modbus-tcp": "PLC/SCADA register bridge over Modbus TCP channels.",
  opcua: "Structured industrial namespace bridge via OPC UA transport.",
  discovery: "Service discovery and reachability signaling between runtimes.",
  web: "HTTP/Web control plane bridge for web-facing runtime integrations.",
});

const HW_DRIVER_SETTINGS_KEYS = Object.freeze({
  simulated: ["io.simulated.inputs", "io.simulated.outputs", "io.simulated.scan_ms"],
  loopback: ["io.simulated.inputs", "io.simulated.outputs", "io.simulated.scan_ms"],
  mqtt: [
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
  "modbus-tcp": [
    "io.modbus.address",
    "io.modbus.unit_id",
    "io.modbus.input_start",
    "io.modbus.output_start",
    "io.modbus.timeout_ms",
    "io.modbus.on_error",
  ],
  gpio: [
    "io.gpio.backend",
    "io.gpio.sysfs_base",
    "io.gpio.inputs_json",
    "io.gpio.outputs_json",
  ],
  ethercat: [
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
  opcua: [
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
  discovery: [
    "discovery.enabled",
    "discovery.service_name",
    "discovery.advertise",
    "discovery.host_group",
    "discovery.interfaces_json",
  ],
  web: [
    "web.enabled",
    "web.listen",
    "web.auth",
    "web.tls",
  ],
  mesh: [
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
  ],
  cloud: ["runtime_cloud.profile"],
  "cloud-wan": ["runtime_cloud.wan.allow_write_json"],
  "cloud-links": ["runtime_cloud.links.transports_json"],
  control: ["control.mode", "control.debug_enabled", "control.endpoint", "control.auth_token"],
  tls: ["tls.mode", "tls.require_remote", "tls.cert_path", "tls.key_path", "tls.ca_path"],
  deploy: ["deploy.require_signed", "deploy.keyring_path"],
  watchdog: ["watchdog.enabled", "watchdog.timeout_ms", "watchdog.action"],
  fault: ["fault.policy"],
  retain: ["retain.mode", "retain.path", "retain.save_interval_ms"],
  observability: [
    "observability.enabled",
    "observability.sample_interval_ms",
    "observability.mode",
    "observability.include_json",
    "observability.alerts_json",
    "observability.history_path",
    "observability.max_entries",
    "observability.prometheus_enabled",
    "observability.prometheus_path",
  ],
});

const HW_SETTINGS_LABEL_OVERRIDES = Object.freeze({
  "io.modbus.address": "PLC endpoint",
  "io.mqtt.broker": "MQTT broker",
  "control.debug_enabled": "Debug enable",
  "runtime_cloud.links.transports_json": "Runtime links",
  "runtime_cloud.wan.allow_write_json": "WAN write rules",
});

function hwIconPath(iconId) {
  const key = String(iconId || "").trim().toLowerCase();
  return HW_ICON_PATHS[key] || HW_ICON_PATHS.endpoint;
}

function hwHexToRgb(color) {
  const text = String(color || "").trim();
  const short = text.match(/^#([0-9a-fA-F]{3})$/);
  if (short) {
    const [r, g, b] = short[1].split("").map((hex) => parseInt(`${hex}${hex}`, 16));
    return { r, g, b };
  }
  const full = text.match(/^#([0-9a-fA-F]{6})$/);
  if (full) {
    return {
      r: parseInt(full[1].slice(0, 2), 16),
      g: parseInt(full[1].slice(2, 4), 16),
      b: parseInt(full[1].slice(4, 6), 16),
    };
  }
  return null;
}

function hwColorAlpha(color, alpha, fallback) {
  const rgb = hwHexToRgb(color);
  if (!rgb) {
    return String(fallback || color || "rgba(15,23,42,0.2)");
  }
  return `rgba(${rgb.r},${rgb.g},${rgb.b},${alpha})`;
}

function hwHashSeed(value) {
  const text = String(value || "");
  let hash = 0;
  for (let i = 0; i < text.length; i += 1) {
    hash = ((hash << 5) - hash + text.charCodeAt(i)) >>> 0;
  }
  return hash.toString(36);
}

function hwIconSvgMarkup(iconId, opts = {}) {
  const stroke = String(opts.stroke || "currentColor");
  const size = Number.isFinite(Number(opts.size)) && Number(opts.size) > 0
    ? Number(opts.size)
    : 16;
  const strokeWidth = Number.isFinite(Number(opts.strokeWidth)) && Number(opts.strokeWidth) > 0
    ? Number(opts.strokeWidth)
    : 1.4;
  const chip = opts.chip === true;
  const chipFill = String(opts.chipFill || hwColorAlpha(stroke, 0.1, "rgba(15,118,110,0.1)"));
  const chipStroke = String(opts.chipStroke || stroke);
  const className = String(opts.className || "").trim();
  const title = String(opts.title || "").trim();
  const titleMarkup = title ? `<title>${escapeHtml(title)}</title>` : "";
  const safeClass = className ? ` class="${escapeAttr(className)}"` : "";
  const glyphMarkup = hwIconPath(iconId);
  const chipMarkup = chip
    ? `<rect x="1.05" y="1.05" width="13.9" height="13.9" rx="4.2" fill="${escapeAttr(chipFill)}" stroke="${escapeAttr(chipStroke)}" stroke-opacity="0.34" stroke-width="0.74"/>`
    : "";
  return `<svg${safeClass} width="${size}" height="${size}" viewBox="0 0 16 16" fill="none" stroke="${escapeAttr(stroke)}" stroke-width="${strokeWidth}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${titleMarkup}${chipMarkup}${glyphMarkup}</svg>`;
}

function hwIconDataUri(iconId, stroke, options = {}) {
  const safeStroke = String(stroke || "#0f766e");
  const useChip = options.chip !== false;
  const key = `${String(iconId || "").trim().toLowerCase()}|${safeStroke}|${useChip ? "chip" : "plain"}`;
  if (HW_ICON_CACHE.has(key)) {
    return HW_ICON_CACHE.get(key);
  }
  const markup = hwIconSvgMarkup(iconId, {
    stroke: safeStroke,
    strokeWidth: 1.7,
    chip: useChip,
    chipFill: hwColorAlpha(safeStroke, 0.15, "rgba(255,255,255,0.08)"),
    chipFillAlt: hwColorAlpha(safeStroke, 0.34, "rgba(255,255,255,0.14)"),
    chipGlow: hwColorAlpha(safeStroke, 0.42, "rgba(255,255,255,0.14)"),
  });
  let encoded = `data:image/svg+xml,${encodeURIComponent(markup)}`;
  if (typeof btoa === "function") {
    try {
      encoded = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(markup)))}`;
    } catch {
      // Keep percent-encoded variant for environments without btoa UTF-8 support.
    }
  }
  HW_ICON_CACHE.set(key, encoded);
  return encoded;
}

function hwCanvasIconDataUri(iconId, stroke) {
  // Cytoscape background-image rendering is stricter than inline HTML SVG usage.
  // Keep canvas node icons on a minimal no-filter SVG variant for consistent display.
  return hwIconDataUri(iconId, stroke, { chip: false });
}

function hwCardTrimText(value, limit = 32) {
  const text = String(value || "").replace(/\s+/g, " ").trim();
  if (!text) return "";
  if (!Number.isFinite(limit) || limit < 4 || text.length <= limit) return text;
  return `${text.slice(0, Math.max(1, limit - 1)).trimEnd()}\u2026`;
}

function hwCardLabelParts(raw) {
  const lines = String(raw || "")
    .split("\n")
    .map((line) => String(line || "").trim())
    .filter(Boolean);
  return {
    title: lines[0] || "",
    subtitle: lines.slice(1).join(" \u00b7 "),
  };
}

function hwNodeCardSvgMarkup(iconId, accent, options = {}) {
  const safeIcon = String(iconId || "endpoint").trim().toLowerCase() || "endpoint";
  const safeAccent = String(accent || "#0f766e").trim() || "#0f766e";
  const variant = String(options.variant || "module").trim().toLowerCase() || "module";
  const dark = options.dark === true;
  const active = options.active === true;
  const title = hwCardTrimText(options.title, variant === "runtime" ? 22 : 20);
  const uid = hwHashSeed(`${safeIcon}|${safeAccent}|${variant}|${dark ? "dark" : "light"}|${active ? "active" : "idle"}|${title}`);
  const width = variant === "runtime" ? 248 : (variant === "endpoint" ? 220 : 232);
  const height = variant === "runtime" ? 94 : (variant === "endpoint" ? 84 : 88);
  const radius = 16;
  const iconSize = 34;
  const iconX = 14;
  const iconY = Math.round((height - iconSize) / 2);
  const titleX = iconX + iconSize + 12;
  const titleY = Math.round(height / 2 + 5);
  const shellStroke = dark
    ? hwColorAlpha(safeAccent, active ? 0.56 : 0.42, "rgba(148,163,184,0.42)")
    : hwColorAlpha(safeAccent, active ? 0.42 : 0.28, "rgba(148,163,184,0.36)");
  const shellInset = dark ? "rgba(255,255,255,0.05)" : "rgba(255,255,255,0.58)";
  const fillStart = dark ? "rgba(8,14,26,0.95)" : "rgba(255,255,255,0.98)";
  const fillEnd = dark ? "rgba(8,12,22,0.93)" : "rgba(246,250,255,0.98)";
  const accentRail = dark
    ? hwColorAlpha(safeAccent, active ? 0.66 : 0.48, safeAccent)
    : hwColorAlpha(safeAccent, active ? 0.5 : 0.36, safeAccent);
  const chipFill = dark
    ? hwColorAlpha(safeAccent, 0.22, "rgba(30,41,59,0.58)")
    : hwColorAlpha(safeAccent, 0.14, "rgba(226,232,240,0.72)");
  const chipStroke = hwColorAlpha(safeAccent, active ? 0.64 : 0.48, safeAccent);
  const glyphColor = dark ? "#f8fafc" : hwColorAlpha(safeAccent, 0.96, safeAccent);
  const titleColor = dark ? "#f8fafc" : "#0f172a";
  const fillId = `hwCardFill-${uid}`;
  const chipId = `hwChipFill-${uid}`;
  return `<svg width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" fill="none" xmlns="http://www.w3.org/2000/svg" role="presentation" aria-hidden="true">
    <defs>
      <linearGradient id="${fillId}" x1="${Math.round(width / 2)}" y1="2" x2="${Math.round(width / 2)}" y2="${height - 2}" gradientUnits="userSpaceOnUse">
        <stop offset="0" stop-color="${escapeAttr(fillStart)}"/>
        <stop offset="1" stop-color="${escapeAttr(fillEnd)}"/>
      </linearGradient>
      <linearGradient id="${chipId}" x1="${iconX + Math.round(iconSize / 2)}" y1="${iconY}" x2="${iconX + Math.round(iconSize / 2)}" y2="${iconY + iconSize}" gradientUnits="userSpaceOnUse">
        <stop offset="0" stop-color="${escapeAttr(chipFill)}"/>
        <stop offset="1" stop-color="${escapeAttr(hwColorAlpha(safeAccent, dark ? 0.12 : 0.07, chipFill))}"/>
      </linearGradient>
    </defs>
    <rect x="1.2" y="1.2" width="${(width - 2.4).toFixed(1)}" height="${(height - 2.4).toFixed(1)}" rx="${radius}" fill="url(#${fillId})" stroke="${escapeAttr(shellStroke)}" stroke-width="1.25"/>
    <rect x="2.2" y="2.2" width="${(width - 4.4).toFixed(1)}" height="${(height - 4.4).toFixed(1)}" rx="${(radius - 1.2).toFixed(1)}" fill="none" stroke="${escapeAttr(shellInset)}" stroke-width="0.8"/>
    <rect x="2.2" y="2.2" width="5.2" height="${(height - 4.4).toFixed(1)}" rx="2.6" fill="${escapeAttr(accentRail)}"/>
    <rect x="${iconX}" y="${iconY}" width="${iconSize}" height="${iconSize}" rx="10" fill="url(#${chipId})" stroke="${escapeAttr(chipStroke)}" stroke-width="1.02"/>
    <g transform="translate(${iconX + 6} ${iconY + 6})" stroke="${escapeAttr(glyphColor)}" fill="none" color="${escapeAttr(glyphColor)}" stroke-width="1.32">${hwIconPath(safeIcon)}</g>
    <text x="${titleX}" y="${titleY}" fill="${titleColor}" font-size="15" font-weight="620" font-family="Sora, Segoe UI, sans-serif">${escapeHtml(title || "Node")}</text>
  </svg>`;
}

function hwNodeCardDataUri(iconId, accent, options = {}) {
  const safeAccent = String(accent || "#0f766e").trim() || "#0f766e";
  const variant = String(options.variant || "module").trim().toLowerCase() || "module";
  const dark = options.dark === true ? "dark" : "light";
  const active = options.active === true ? "active" : "idle";
  const contentHash = hwHashSeed(`${options.title || ""}|${options.subtitle || ""}|${options.badge || ""}`);
  const key = `${String(iconId || "").trim().toLowerCase()}|${safeAccent}|${variant}|${dark}|${active}|${contentHash}`;
  if (HW_NODE_CARD_CACHE.has(key)) {
    return HW_NODE_CARD_CACHE.get(key);
  }
  const markup = hwNodeCardSvgMarkup(iconId, safeAccent, options);
  let encoded = `data:image/svg+xml,${encodeURIComponent(markup)}`;
  if (typeof btoa === "function") {
    try {
      encoded = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(markup)))}`;
    } catch {
      // Keep percent-encoded fallback where UTF-8 btoa is unavailable.
    }
  }
  HW_NODE_CARD_CACHE.set(key, encoded);
  return encoded;
}

function hwPaletteMeta(item) {
  if (item && typeof item.meta === "string" && item.meta.trim()) {
    return item.meta.trim();
  }
  const parts = [];
  const channels = Number(item?.channels || 0);
  if (Number.isFinite(channels) && channels > 0) {
    parts.push(`${channels} channels`);
  }
  const driver = String(item?.driver || "").trim();
  if (driver) {
    parts.push(hwDriverDisplayName(driver));
  }
  return parts.join(" • ");
}

function hwProtocolIcon(proto) {
  const key = String(proto || "").trim().toLowerCase();
  if (key.includes("realtime")) return "runtime";
  if (key.includes("zenoh")) return "mesh";
  if (key.includes("mqtt")) return "mqtt";
  if (key.includes("modbus")) return "modbus";
  if (key.includes("opcua")) return "opcua";
  if (key.includes("ethercat")) return "ethercat";
  if (key.includes("mesh")) return "mesh";
  if (key.includes("discovery")) return "discovery";
  if (key.includes("web")) return "web";
  if (key.includes("runtime")) return "runtime";
  return "endpoint";
}

function hwProtocolIconStroke(proto, theme = hwResolveCytoscapeTheme()) {
  const key = String(proto || "").trim().toLowerCase();
  if (key.includes("realtime")) return "#0284c7";
  if (key.includes("zenoh")) return "#0ea5e9";
  if (key.includes("mqtt")) return "#f59e0b";
  if (key.includes("modbus")) return "#2563eb";
  if (key.includes("opcua")) return "#7c3aed";
  if (key.includes("ethercat")) return "#ca8a04";
  if (key.includes("mesh")) return "#10b981";
  if (key.includes("discovery")) return "#06b6d4";
  if (key.includes("web")) return "#14b8a6";
  if (key.includes("runtime")) return "#0284c7";
  return theme.iconMuted;
}

function hwFormatFabricLabel(proto, rawLabel) {
  const label = String(rawLabel || "").trim();
  if (!label) return "Endpoint";
  if (label.includes("\n")) return label;
  const knownPrefixes = [
    "Runtime Control",
    "Modbus TCP",
    "MQTT",
    "OPC UA",
    "EtherCAT",
    "Discovery",
    "Mesh",
    "Web",
    "PLC",
  ];
  for (const prefix of knownPrefixes) {
    const lowerPrefix = prefix.toLowerCase();
    if (!label.toLowerCase().startsWith(`${lowerPrefix} `)) continue;
    const detail = label.slice(prefix.length).trim();
    if (!detail) return prefix;
    return `${prefix}\n${detail}`;
  }
  if (label.length > 22) {
    const splitIdx = label.indexOf("://");
    if (splitIdx > 0) {
      const protoLabel = label.slice(0, splitIdx).trim();
      const detail = label.slice(splitIdx).trim();
      if (protoLabel && detail) {
        return `${protoLabel}\n${detail}`;
      }
    }
    const mid = Math.floor(label.length / 2);
    const leftBreak = label.lastIndexOf(" ", mid);
    const rightBreak = label.indexOf(" ", mid);
    const at = leftBreak > 8 ? leftBreak : rightBreak;
    if (at > 8 && at < label.length - 5) {
      return `${label.slice(0, at).trim()}\n${label.slice(at + 1).trim()}`;
    }
  }
  return label;
}

function hwIconForModule(mod) {
  const type = String(mod?.paletteType || mod?.type || "").trim().toLowerCase();
  const nodeType = String(mod?.nodeType || "").trim().toLowerCase();
  if (type === "cpu") return "cpu";
  if (type.startsWith("di-")) return "di";
  if (type.startsWith("do-")) return "do";
  if (type.startsWith("ai-")) return "ai";
  if (type.startsWith("ao-")) return "ao";
  if (type.includes("mqtt")) return "mqtt";
  if (type.includes("modbus")) return "modbus";
  if (type.includes("ethercat")) return "ethercat";
  if (type.includes("opcua")) return "opcua";
  if (nodeType === "cpu") return "cpu";
  if (nodeType === "output") return "do";
  if (nodeType === "comm") return "mesh";
  return "di";
}

function hwIconStrokeForModule(mod, theme = hwResolveCytoscapeTheme()) {
  const type = String(mod?.paletteType || mod?.type || "").trim().toLowerCase();
  if (type.includes("mqtt")) return "#f59e0b";
  if (type.includes("modbus")) return "#2563eb";
  if (type.includes("ethercat")) return "#ca8a04";
  if (type.includes("opcua")) return "#7c3aed";
  if (type.includes("mesh")) return "#10b981";
  if (type.includes("web")) return "#14b8a6";
  const nodeType = String(mod?.nodeType || "").trim().toLowerCase();
  if (nodeType === "cpu") return theme.accentStrong;
  if (nodeType === "input") return "#15803d";
  if (nodeType === "output") return "#b91c1c";
  if (nodeType === "comm") return "#4f46e5";
  return theme.iconMuted;
}

function hwNodeCardVariantForData(data) {
  const fabric = String(data?.fabric || "").trim() === "true";
  if (fabric) {
    return String(data?.kind || "").trim().toLowerCase() === "runtime"
      ? "runtime"
      : "endpoint";
  }
  const nodeType = String(data?.nodeType || "").trim().toLowerCase();
  if (nodeType === "cpu") return "runtime";
  if (nodeType === "comm") return "endpoint";
  return "module";
}

function hwNodeCardAccentForData(data, theme = hwResolveCytoscapeTheme()) {
  const fabric = String(data?.fabric || "").trim() === "true";
  if (fabric) {
    const proto = String(data?.proto || "").trim().toLowerCase();
    const kind = String(data?.kind || "").trim().toLowerCase();
    return hwProtocolIconStroke(proto || (kind === "runtime" ? "runtime" : "endpoint"), theme);
  }
  return hwIconStrokeForModule({
    paletteType: data?.paletteType,
    type: data?.type,
    nodeType: data?.nodeType,
    driver: data?.driver,
  }, theme);
}

function hwNodeCardTextForData(data) {
  const parts = hwCardLabelParts(data?.label);
  const fabric = String(data?.fabric || "").trim() === "true";
  const kind = String(data?.kind || "").trim().toLowerCase();
  let title = String(data?.cardTitle || parts.title || "").trim();
  let subtitle = "";

  if (kind === "runtime") {
    title = title || String(data?.runtimeId || parts.title || "Runtime").trim();
  } else if (fabric) {
    title = title || parts.title || "Endpoint";
  } else {
    title = title || parts.title || "Module";
  }

  return {
    title: hwCardTrimText(title, 24),
    subtitle: hwCardTrimText(subtitle, 20),
    badge: "",
  };
}

function hwNodeCardImageForData(data, theme = hwResolveCytoscapeTheme()) {
  const fabric = String(data?.fabric || "").trim() === "true";
  const proto = String(data?.proto || "").trim().toLowerCase();
  const kind = String(data?.kind || "").trim().toLowerCase();
  const nodeType = String(data?.nodeType || "").trim().toLowerCase();
  const icon = fabric
    ? hwProtocolIcon(proto || (kind === "runtime" ? "runtime" : "endpoint"))
    : hwIconForModule({
      paletteType: data?.paletteType,
      type: data?.type,
      nodeType,
      driver: data?.driver,
    });
  const accent = hwNodeCardAccentForData(data, theme);
  const variant = hwNodeCardVariantForData(data);
  const dark = String(document.body?.dataset?.theme || "").trim().toLowerCase() === "dark";
  const active = String(data?.activeRuntime || "").trim() === "true";
  const text = hwNodeCardTextForData(data);
  return hwNodeCardDataUri(icon, accent, {
    variant,
    dark,
    active,
    title: text.title,
    subtitle: text.subtitle,
    badge: text.badge,
  });
}

const HW_CYTOSCAPE_THEME_FALLBACK = Object.freeze({
  fontFamily: "sans-serif",
  text: "#0f172a",
  panel: "#e5ecf3",
  panelSoft: "#d8e2ec",
  border: "#cbd5e1",
  edge: "#94a3b8",
  accent: "#14b8a6",
  accentSoft: "rgba(20,184,166,0.12)",
  accentStrong: "#0d9488",
  icon: "#0f766e",
  iconMuted: "#475569",
  shadow: "rgba(15,23,42,0.24)",
});

function hwResolveThemeValue(cssVar, fallback) {
  const root = document.documentElement;
  const body = document.body;
  const rootStyles = getComputedStyle(root);
  const bodyStyles = body ? getComputedStyle(body) : rootStyles;
  const resolved = bodyStyles.getPropertyValue(cssVar).trim()
    || rootStyles.getPropertyValue(cssVar).trim();
  if (!resolved || resolved.includes("var(")) {
    return fallback;
  }
  return resolved;
}

function hwResolveCytoscapeTheme() {
  return {
    fontFamily: hwResolveThemeValue("--ide-font-sans", HW_CYTOSCAPE_THEME_FALLBACK.fontFamily),
    text: hwResolveThemeValue("--text", HW_CYTOSCAPE_THEME_FALLBACK.text),
    panel: hwResolveThemeValue("--panel-2", HW_CYTOSCAPE_THEME_FALLBACK.panel),
    panelSoft: hwResolveThemeValue("--panel", HW_CYTOSCAPE_THEME_FALLBACK.panelSoft),
    border: hwResolveThemeValue("--border", HW_CYTOSCAPE_THEME_FALLBACK.border),
    edge: hwResolveThemeValue("--border", HW_CYTOSCAPE_THEME_FALLBACK.edge),
    accent: hwResolveThemeValue("--accent", HW_CYTOSCAPE_THEME_FALLBACK.accent),
    accentSoft: hwResolveThemeValue("--accent-soft", HW_CYTOSCAPE_THEME_FALLBACK.accentSoft),
    accentStrong: hwResolveThemeValue("--accent-strong", HW_CYTOSCAPE_THEME_FALLBACK.accentStrong),
    icon: hwResolveThemeValue("--accent-strong", HW_CYTOSCAPE_THEME_FALLBACK.icon),
    iconMuted: hwResolveThemeValue("--muted-strong", HW_CYTOSCAPE_THEME_FALLBACK.iconMuted),
    shadow: HW_CYTOSCAPE_THEME_FALLBACK.shadow,
  };
}

function hwBuildCytoscapeStyles(theme = hwResolveCytoscapeTheme()) {
  return [
    {
      selector: "node",
      style: {
        label: "",
        "background-color": theme.panelSoft,
        "background-fill": "solid",
        shape: "roundrectangle",
        width: 232,
        height: "data(height)",
        padding: "0px",
        "border-width": 1.4,
        "border-color": theme.border,
        "border-opacity": 0.58,
        "background-image": "data(cardImage)",
        "background-fit": "cover",
        "background-repeat": "no-repeat",
        "background-width": "100%",
        "background-height": "100%",
        "background-position-x": "50%",
        "background-position-y": "50%",
        "background-image-opacity": 1,
        "shadow-blur": 12,
        "shadow-color": theme.shadow,
        "shadow-opacity": 0.1,
        "shadow-offset-y": 3,
      },
    },
    {
      selector: "node[fabric='true']",
      style: {
        "border-width": 1.6,
      },
    },
    {
      selector: "node[fabric='true'][kind='runtime']",
      style: {
        shape: "roundrectangle",
        width: 248,
        height: 94,
        "border-color": "#0284c7",
        "shadow-color": "rgba(2,132,199,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
        "shadow-offset-y": 5,
      },
    },
    {
      selector: "node[fabric='true'][kind='runtime'][activeRuntime='true']",
      style: {
        "border-color": theme.accentStrong,
        "border-width": 2.2,
        "shadow-color": "rgba(13,148,136,0.34)",
        "shadow-opacity": 0.2,
        "shadow-blur": 20,
      },
    },
    {
      selector: "node[fabric='true'][kind='endpoint']",
      style: {
        shape: "roundrectangle",
        width: 220,
        height: 84,
        "border-color": "#64748b",
        "shadow-color": "rgba(100,116,139,0.2)",
        "shadow-opacity": 0.12,
        "shadow-blur": 14,
      },
    },
    {
      selector: "node[fabric='true'][proto='runtime-external']",
      style: {
        "border-style": "dashed",
        "border-color": "#64748b",
      },
    },
    {
      selector: "node[fabric='true'][proto='mqtt']",
      style: {
        "border-color": "#d97706",
        "shadow-color": "rgba(217,119,6,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[fabric='true'][proto='modbus']",
      style: {
        "border-color": "#2563eb",
        "shadow-color": "rgba(37,99,235,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[fabric='true'][proto='opcua']",
      style: {
        "border-color": "#7c3aed",
        "shadow-color": "rgba(124,58,237,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[fabric='true'][proto='ethercat']",
      style: {
        "border-color": "#ca8a04",
        "shadow-color": "rgba(202,138,4,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[fabric='true'][proto='mesh']",
      style: {
        "border-color": "#10b981",
        "shadow-color": "rgba(16,185,129,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[fabric='true'][proto='discovery']",
      style: {
        "border-color": "#06b6d4",
        "shadow-color": "rgba(6,182,212,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[fabric='true'][proto='web']",
      style: {
        "border-color": "#0f766e",
        "shadow-color": "rgba(20,184,166,0.24)",
        "shadow-opacity": 0.16,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[nodeType='cpu']",
      style: {
        "border-color": theme.accent,
        "border-width": 2.2,
        "shadow-color": "rgba(13,148,136,0.3)",
        "shadow-opacity": 0.18,
        "shadow-blur": 18,
      },
    },
    {
      selector: "node[nodeType='input']",
      style: {
        "border-color": "#22c55e",
        "shadow-color": "rgba(21,128,61,0.24)",
        "shadow-opacity": 0.14,
        "shadow-blur": 16,
      },
    },
    {
      selector: "node[nodeType='output']",
      style: {
        "border-color": "#ef4444",
        "shadow-color": "rgba(185,28,28,0.24)",
        "shadow-opacity": 0.14,
        "shadow-blur": 16,
      },
    },
    {
      selector: "node[nodeType='comm']",
      style: {
        "border-color": "#6366f1",
        "shadow-color": "rgba(79,70,229,0.24)",
        "shadow-opacity": 0.14,
        "shadow-blur": 16,
      },
    },
    {
      selector: "node:selected",
      style: {
        "border-color": theme.accentStrong,
        "border-width": 2.6,
        "overlay-color": theme.accent,
        "overlay-opacity": 0.03,
      },
    },
    {
      selector: "node[fabric='true']:selected",
      style: {
        "overlay-opacity": 0.04,
      },
    },
    {
      selector: "edge",
      style: {
        "curve-style": "bezier",
        "target-arrow-shape": "triangle",
        "target-arrow-color": theme.edge,
        "line-color": theme.edge,
        width: 2.8,
        opacity: 0.92,
        "arrow-scale": 1.06,
        "line-cap": "round",
      },
    },
    {
      selector: "edge[fabric='true']",
      style: {
        label: "data(label)",
        "font-size": "9px",
        "font-family": theme.fontFamily,
        "font-weight": 600,
        color: theme.text,
        "text-wrap": "none",
        "text-background-opacity": 0.92,
        "text-background-color": theme.panel,
        "text-background-padding": 4,
        "text-border-width": 0.8,
        "text-border-color": theme.border,
        "text-border-opacity": 0.48,
        "text-rotation": "autorotate",
        "target-arrow-shape": "triangle",
        "target-arrow-fill": "filled",
        "curve-style": "unbundled-bezier",
        "control-point-distances": "22 -22",
        "control-point-weights": "0.25 0.75",
        width: 3.2,
        opacity: 0.95,
        "arrow-scale": 1.02,
        "line-cap": "round",
      },
    },
    {
      selector: "edge[fabric='true'][proto='internal']",
      style: {
        "line-style": "dashed",
        "line-color": "#94a3b8",
        "target-arrow-shape": "none",
        width: 2.2,
        "line-dash-pattern": [10, 6],
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud']",
      style: {
        "line-color": "#0ea5e9",
        "target-arrow-color": "#0ea5e9",
        width: 4.4,
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='realtime']",
      style: {
        "line-style": "solid",
        "line-dash-pattern": [1, 0],
        width: 4.9,
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='zenoh']",
      style: {
        "line-style": "dashed",
        "line-dash-pattern": [12, 6],
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='mqtt']",
      style: {
        "line-color": "#f59e0b",
        "target-arrow-color": "#f59e0b",
        "line-style": "dashed",
        "line-dash-pattern": [7, 4],
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='modbus-tcp']",
      style: {
        "line-color": "#2563eb",
        "target-arrow-color": "#2563eb",
        "line-dash-pattern": [14, 4],
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='opcua']",
      style: {
        "line-color": "#7c3aed",
        "target-arrow-color": "#7c3aed",
        "line-dash-pattern": [3, 4],
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='mesh']",
      style: {
        "line-color": "#10b981",
        "target-arrow-color": "#10b981",
        "line-style": "dashed",
        "line-dash-pattern": [7, 4],
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='discovery']",
      style: {
        "line-color": "#06b6d4",
        "target-arrow-color": "#06b6d4",
        "line-style": "dotted",
      },
    },
    {
      selector: "edge[fabric='true'][proto='runtime_cloud'][transport='web']",
      style: {
        "line-color": "#14b8a6",
        "target-arrow-color": "#14b8a6",
        "line-dash-pattern": [6, 4],
      },
    },
    {
      selector: "edge[fabric='true'][proto='mqtt']",
      style: {
        "line-color": "#f59e0b",
        "target-arrow-color": "#f59e0b",
      },
    },
    {
      selector: "edge[fabric='true'][proto='modbus']",
      style: {
        "line-color": "#3b82f6",
        "target-arrow-color": "#3b82f6",
      },
    },
    {
      selector: "edge[fabric='true'][proto='ethercat']",
      style: {
        "line-color": "#ca8a04",
        "target-arrow-color": "#ca8a04",
      },
    },
    {
      selector: "edge[fabric='true'][proto='opcua']",
      style: {
        "line-color": "#7c3aed",
        "target-arrow-color": "#7c3aed",
      },
    },
    {
      selector: "edge[fabric='true'][proto='mesh']",
      style: {
        "line-color": "#10b981",
        "target-arrow-color": "#10b981",
      },
    },
    {
      selector: "edge[fabric='true'][proto='discovery']",
      style: {
        "line-color": "#06b6d4",
        "target-arrow-color": "#06b6d4",
      },
    },
    {
      selector: "edge[fabric='true'][proto='web']",
      style: {
        "line-color": "#14b8a6",
        "target-arrow-color": "#14b8a6",
      },
    },
    {
      selector: "edge:selected",
      style: {
        "line-color": theme.accent,
        "target-arrow-color": theme.accent,
        width: 5.2,
        opacity: 1,
      },
    },
  ];
}

// ── Hardware State ─────────────────────────────────────

const hwState = {
  cy: null,
  themeObserver: null,
  modules: [],
  workspaceRuntimes: [],
  activeRuntimeId: "",
  linkCreateSourceRuntimeId: "",
  lastRuntimeLinkTransport: "realtime",
  fabricNodeMeta: new Map(),
  fabricEdgeMeta: new Map(),
  selectedFabricNodeId: "",
  selectedFabricEdgeId: "",
  contextRuntimeMeta: null,
  contextEdgeMeta: null,
  nextModuleId: 1,
  nextInputByte: 0,
  nextOutputByte: 0,
  selectedModuleId: null,
  viewMode: "canvas",
  legendVisible: false,
  inspectorCollapsed: true,
  driversCollapsed: true,
  isCanvasFullscreen: false,
  ioValues: {},
  forcedAddresses: new Set(),
  livePollingTimer: null,
  initialized: false,
  hydratedProject: null,
  hydrating: false,
  lastIoConfig: null,
  runtimeCommEntries: [],
  persistTimer: null,
  persistInFlight: false,
  persistQueued: false,
  lastPersistFingerprint: "",
};

const HW_CANVAS_DEFAULT_PADDING = 28;
const HW_CANVAS_RELAYOUT_DELAYS_MS = [0, 90, 260];
const HW_RUNTIME_SELECTION_EVENT = "ide-runtime-selection-changed";
let hwCanvasRelayoutTimers = [];

function hwMinZoomForNodeCount(nodeCount) {
  const count = Number(nodeCount) || 0;
  if (count >= 28) return 0.44;
  if (count >= 18) return 0.52;
  if (count >= 10) return 0.6;
  if (count >= 6) return 0.68;
  return 0.78;
}

function hwClearScheduledRelayouts() {
  if (hwCanvasRelayoutTimers.length === 0) return;
  for (const timer of hwCanvasRelayoutTimers) {
    clearTimeout(timer);
  }
  hwCanvasRelayoutTimers = [];
}

function hwCanvasIsVisible() {
  if (!el.hwCanvas) return false;
  if (el.hwCanvas.hidden) return false;
  const style = getComputedStyle(el.hwCanvas);
  if (style.display === "none" || style.visibility === "hidden") return false;
  const rect = el.hwCanvas.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
}

function hwRelayoutCanvas(options = {}) {
  if (!hwState.cy || hwState.viewMode !== "canvas") return;
  if (!hwCanvasIsVisible()) return;
  hwState.cy.resize();
  const visibleElements = hwState.cy.elements().filter((ele) => ele.visible());
  if (visibleElements.length > 0) {
    const requestedPadding = Number.isFinite(options.padding)
      ? Number(options.padding)
      : HW_CANVAS_DEFAULT_PADDING;
    const padding = Math.max(16, Math.min(56, requestedPadding));
    hwState.cy.fit(visibleElements, padding);
    const nodeCount = visibleElements.nodes().length;
    const fallbackMinZoom = hwMinZoomForNodeCount(nodeCount);
    const minZoom = Number.isFinite(options.minZoom)
      ? Math.max(0.2, Number(options.minZoom))
      : fallbackMinZoom;
    if (hwState.cy.zoom() < minZoom) {
      hwState.cy.zoom(minZoom);
      hwState.cy.center(visibleElements);
    }
  }
}

function hwScheduleCanvasRelayout(options = {}) {
  if (!hwState.cy) return;
  hwClearScheduledRelayouts();
  requestAnimationFrame(() => {
    for (const delay of HW_CANVAS_RELAYOUT_DELAYS_MS) {
      const timer = setTimeout(() => {
        hwRelayoutCanvas(options);
      }, delay);
      hwCanvasRelayoutTimers.push(timer);
    }
  });
}

function hwRefreshNodeIconImages(theme = hwResolveCytoscapeTheme()) {
  if (!hwState.cy) return;
  const moduleById = new Map(hwState.modules.map((mod) => [mod.id, mod]));
  hwState.cy.nodes().forEach((node) => {
    const data = node.data() || {};
    if (String(data.fabric || "") === "true") {
      const proto = String(data.proto || "").trim().toLowerCase();
      const icon = hwProtocolIcon(proto || (data.kind === "runtime" ? "runtime" : "endpoint"));
      const stroke = hwProtocolIconStroke(proto || (data.kind === "runtime" ? "runtime" : "endpoint"), theme);
      node.data("iconImage", hwCanvasIconDataUri(icon, stroke));
      node.data("cardImage", hwNodeCardImageForData(data, theme));
      return;
    }
    const mod = moduleById.get(String(data.moduleRef || data.id || ""));
    if (!mod) return;
    const iconKey = hwIconForModule(mod);
    const iconStroke = hwIconStrokeForModule(mod, theme);
    node.data("iconImage", hwCanvasIconDataUri(iconKey, iconStroke));
    node.data("cardImage", hwNodeCardDataUri(iconKey, iconStroke, {
      variant: hwNodeCardVariantForData({
        nodeType: mod.nodeType,
      }),
      dark: String(document.body?.dataset?.theme || "").trim().toLowerCase() === "dark",
      active: false,
    }));
  });
}

function hwApplyCytoscapeTheme() {
  if (!hwState.cy) return;
  const theme = hwResolveCytoscapeTheme();
  hwRefreshNodeIconImages(theme);
  hwState.cy.style(hwBuildCytoscapeStyles(theme));
  hwState.cy.style().update();
}

function hwEnsureThemeObserver() {
  if (hwState.themeObserver || typeof MutationObserver === "undefined") return;
  const target = document.body || document.documentElement;
  if (!target) return;
  hwState.themeObserver = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      if (mutation.type === "attributes" && mutation.attributeName === "data-theme") {
        hwApplyCytoscapeTheme();
        return;
      }
    }
  });
  hwState.themeObserver.observe(target, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });
}

// ── Palette Rendering ──────────────────────────────────

function renderHardwarePalette() {
  const container = el.hardwarePalette;
  if (!container) return;
  container.innerHTML = "";

  for (const category of HW_PALETTE_CATEGORIES) {
    const section = document.createElement("div");
    section.className = "hw-palette-category";

    const header = document.createElement("button");
    header.type = "button";
    header.className = "hw-palette-category-header";
    header.innerHTML = `<span>${escapeHtml(category.label)}</span><span class="muted" style="font-size:10px">${category.items.length}</span>`;
    header.setAttribute("aria-expanded", "true");
    header.addEventListener("click", () => {
      const expanded = header.getAttribute("aria-expanded") === "true";
      header.setAttribute("aria-expanded", String(!expanded));
      list.hidden = expanded;
    });
    section.appendChild(header);

    const list = document.createElement("div");
    list.className = "hw-palette-items";
    for (const item of category.items) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "hw-palette-item";
      btn.draggable = true;
      btn.dataset.hwType = item.type;
      const iconStroke = hwIconStrokeForModule({
        paletteType: item.type,
        type: item.type,
        nodeType: hwModuleNodeType(item),
        driver: item.driver,
      });
      btn.style.setProperty("--hw-icon-accent", iconStroke);
      const meta = hwPaletteMeta(item);
      btn.innerHTML = `<span class="hw-palette-icon">${hwIconSvgMarkup(item.icon, {
        stroke: iconStroke,
        size: 20,
        strokeWidth: 1.4,
        title: item.label,
        chip: true,
      })}</span>
      <span class="hw-palette-copy">
        <span class="hw-palette-title">${escapeHtml(item.label)}</span>
        ${meta ? `<span class="hw-palette-meta">${escapeHtml(meta)}</span>` : ""}
      </span>`;
      btn.addEventListener("dblclick", () => hwAddModule(item));
      btn.addEventListener("dragstart", (e) => {
        e.dataTransfer.setData("text/plain", JSON.stringify(item));
        e.dataTransfer.effectAllowed = "copy";
      });
      list.appendChild(btn);
    }
    section.appendChild(list);
    container.appendChild(section);
  }
}

// ── Module Management ──────────────────────────────────

function hwFindPaletteDef(type) {
  for (const cat of HW_PALETTE_CATEGORIES) {
    for (const item of cat.items) {
      if (item.type === type) return item;
    }
  }
  return null;
}

function hwAllocateAddresses(direction, channels) {
  const addresses = [];
  if (direction === "input") {
    for (let i = 0; i < channels; i++) {
      const byte = hwState.nextInputByte + Math.floor(i / 8);
      const bit = i % 8;
      addresses.push(`%IX${byte}.${bit}`);
    }
    hwState.nextInputByte += Math.ceil(channels / 8);
  } else if (direction === "output") {
    for (let i = 0; i < channels; i++) {
      const byte = hwState.nextOutputByte + Math.floor(i / 8);
      const bit = i % 8;
      addresses.push(`%QX${byte}.${bit}`);
    }
    hwState.nextOutputByte += Math.ceil(channels / 8);
  }
  return addresses;
}

function hwIsCommunicationModule(mod) {
  if (!mod) return false;
  if (mod.nodeType === "comm") return true;
  const driver = String(mod.driver || "").toLowerCase();
  return HW_COMM_DRIVERS.has(driver);
}

function hwModuleLabelWithChannels(mod) {
  const addresses = Array.isArray(mod?.addresses) ? mod.addresses : [];
  if (addresses.length === 0) return mod?.label || "";
  return `${mod.label}\n${hwCompactAddressRange(addresses[0], addresses[addresses.length - 1])}`;
}

function hwNodeHeightForLabel(_label) {
  return 88;
}

function hwCompactAddressRange(start, end) {
  const first = String(start || "");
  const last = String(end || "");
  if (!first) return last;
  if (!last || first === last) return first;
  let prefixLen = 0;
  while (
    prefixLen < first.length &&
    prefixLen < last.length &&
    first[prefixLen] === last[prefixLen]
  ) {
    prefixLen += 1;
  }
  if (prefixLen >= 3) {
    return `${first}..${last.slice(prefixLen)}`;
  }
  return `${first}..${last}`;
}

function hwFormatPrimitive(value) {
  if (value === true) return "enabled";
  if (value === false) return "disabled";
  if (value == null) return "--";
  if (Array.isArray(value)) return value.join(", ");
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function hwDriverDisplayName(name) {
  const id = String(name || "").toLowerCase();
  if (id === "modbus-tcp") return "Modbus TCP";
  if (id === "mqtt") return "MQTT";
  if (id === "gpio") return "GPIO";
  if (id === "ethercat") return "EtherCAT";
  if (id === "simulated") return "Simulated I/O";
  if (id === "loopback") return "Loopback";
  if (id === "opcua") return "OPC UA";
  if (id === "cloud-wan") return "Cloud WAN";
  if (id === "cloud-links") return "Cloud Links";
  return id ? id.replace(/[-_]/g, " ").replace(/\b\w/g, (ch) => ch.toUpperCase()) : "Driver";
}

function hwSettingsCategoryForKey(key) {
  const value = String(key || "").trim();
  if (!value) return "all";
  if (value === "web.auth" || value === "web.tls") {
    return "security";
  }
  if (
    value.startsWith("resource.")
    || value.startsWith("log.")
    || value === "control.endpoint"
    || value === "control.mode"
    || value === "control.debug_enabled"
  ) {
    return "general";
  }
  if (
    value.startsWith("watchdog.")
    || value.startsWith("fault.")
    || value === "resource.tasks_json"
    || value === "resource.cycle_interval_ms"
  ) {
    return "execution";
  }
  if (value.startsWith("retain.")) return "retention";
  if (
    value.startsWith("web.")
    || value.startsWith("discovery.")
    || value.startsWith("mesh.")
    || value.startsWith("runtime_cloud.")
    || value.startsWith("opcua.")
    || value.startsWith("io.")
  ) {
    return "communication";
  }
  if (value.startsWith("tls.") || value.startsWith("deploy.") || value === "control.auth_token") {
    return "security";
  }
  if (value.startsWith("observability.")) return "observability";
  if (value.startsWith("simulation.")) return "simulation";
  return "all";
}

function hwSettingsLabelForKey(key) {
  const value = String(key || "").trim();
  if (!value) return "Configure";
  if (Object.prototype.hasOwnProperty.call(HW_SETTINGS_LABEL_OVERRIDES, value)) {
    return HW_SETTINGS_LABEL_OVERRIDES[value];
  }
  const tail = value.split(".").pop() || value;
  return tail
    .replace(/_json$/i, " JSON")
    .replace(/_/g, " ")
    .replace(/\b\w/g, (ch) => ch.toUpperCase());
}

function hwSettingsActionsForDriver(driverName) {
  const name = String(driverName || "").toLowerCase().trim();
  if (!name) return [];
  const configured = HW_DRIVER_SETTINGS_KEYS[name];
  if (!Array.isArray(configured) || configured.length === 0) return [];
  const actions = [];
  const seen = new Set();
  for (const keyValue of configured) {
    const key = String(keyValue || "").trim();
    if (!key || seen.has(key)) continue;
    seen.add(key);
    actions.push({
      key,
      label: hwSettingsLabelForKey(key),
      category: hwSettingsCategoryForKey(key),
    });
  }
  return actions;
}

function hwDriverDetailPairs(name, params) {
  const p = (params && typeof params === "object") ? params : {};
  const id = String(name || "").toLowerCase();
  if (id === "modbus-tcp") {
    return [
      ["Address", p.address || "--"],
      ["Unit", p.unit_id ?? 1],
      ["Timeout", `${p.timeout_ms ?? 500} ms`],
    ];
  }
  if (id === "mqtt") {
    return [
      ["Broker", p.broker || "--"],
      ["Topic In", p.topic_in || "--"],
      ["Topic Out", p.topic_out || "--"],
    ];
  }
  if (id === "ethercat") {
    return [
      ["Adapter", p.adapter || "--"],
      ["Timeout", `${p.timeout_ms ?? 250} ms`],
      ["On Error", p.on_error || "fault"],
    ];
  }
  if (id === "gpio") {
    const inputCount = Array.isArray(p.inputs) ? p.inputs.length : (Array.isArray(p.input) ? p.input.length : 0);
    const outputCount = Array.isArray(p.outputs) ? p.outputs.length : (Array.isArray(p.output) ? p.output.length : 0);
    return [
      ["Backend", p.backend || "libgpiod"],
      ["GPIO chip", p.chip || "/dev/gpiochip0"],
      ["Inputs", inputCount],
      ["Outputs", outputCount],
    ];
  }
  const pairs = [];
  for (const [key, value] of Object.entries(p)) {
    if (typeof value === "object") continue;
    pairs.push([key, hwFormatPrimitive(value)]);
    if (pairs.length === 3) break;
  }
  if (pairs.length === 0) {
    pairs.push(["Status", "Configured"]);
  }
  return pairs;
}

function hwParseTomlValue(raw) {
  const text = String(raw || "").trim();
  if (!text) return "";
  if (
    (text.startsWith("\"") && text.endsWith("\""))
    || (text.startsWith("'") && text.endsWith("'"))
  ) {
    return hwTomlUnquote(text);
  }
  if (text === "true") return true;
  if (text === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(text)) {
    return Number(text);
  }
  if (text.startsWith("{") && text.endsWith("}")) {
    const inline = hwTomlParseInlineTable(text);
    if (inline && typeof inline === "object") return inline;
  }
  if (text.startsWith("[") && text.endsWith("]")) {
    const inner = text.slice(1, -1).trim();
    if (!inner) return [];
    return hwTomlSplitTopLevel(inner)
      .map((part) => hwParseTomlValue(part))
      .filter((part) => part !== "");
  }
  return text;
}

function hwParseTomlSections(text) {
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
    const eqIndex = line.indexOf("=");
    if (eqIndex <= 0 || !currentSection) continue;
    const key = line.slice(0, eqIndex).trim();
    const valueRaw = line.slice(eqIndex + 1).trim();
    sections[currentSection][key] = hwParseTomlValue(valueRaw);
  }
  return sections;
}

function hwTomlStripComment(line) {
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

function hwTomlNeedsContinuation(raw) {
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

function hwTomlReadRawAssignment(text, section, key) {
  const lines = String(text || "").replace(/\r\n/g, "\n").split("\n");
  let sectionStart = -1;
  let sectionEnd = lines.length;

  for (let i = 0; i < lines.length; i += 1) {
    const match = lines[i].match(/^\s*\[([^\]]+)\]\s*$/);
    if (!match) continue;
    if (String(match[1] || "").trim() === section) {
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

  const keyRegex = new RegExp(`^\\s*${String(key || "").replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*=\\s*(.*)$`);
  for (let i = sectionStart + 1; i < sectionEnd; i += 1) {
    const line = lines[i];
    const match = line.match(keyRegex);
    if (!match) continue;

    let rawValue = hwTomlStripComment(match[1]);
    let cursor = i;
    while (hwTomlNeedsContinuation(rawValue) && cursor + 1 < sectionEnd) {
      cursor += 1;
      const continuation = hwTomlStripComment(lines[cursor]);
      if (!continuation) continue;
      rawValue = `${rawValue}\n${continuation}`;
    }
    return rawValue.trim();
  }
  return null;
}

function hwTomlSplitTopLevel(text) {
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

function hwTomlUnquote(text) {
  const value = String(text || "").trim();
  if (value.startsWith("\"") && value.endsWith("\"")) {
    return value.slice(1, -1).replace(/\\\"/g, "\"").replace(/\\\\/g, "\\");
  }
  if (value.startsWith("'") && value.endsWith("'")) {
    return value.slice(1, -1);
  }
  return value;
}

function hwTomlParseInlineTable(raw) {
  const text = String(raw || "").trim();
  if (!text.startsWith("{") || !text.endsWith("}")) return null;
  const inner = text.slice(1, -1).trim();
  if (!inner) return {};
  const out = {};
  for (const entry of hwTomlSplitTopLevel(inner)) {
    const eqIndex = entry.indexOf("=");
    if (eqIndex <= 0) continue;
    const key = hwTomlUnquote(entry.slice(0, eqIndex)).trim();
    const value = hwParseTomlValue(entry.slice(eqIndex + 1));
    out[key] = value;
  }
  return out;
}

function hwNormalizeRuntimeLinkTransport(value) {
  const normalized = String(value || "").trim().toLowerCase();
  if (!normalized) return "";
  if (!HW_RUNTIME_LINK_TRANSPORT_IDS.has(normalized)) return "";
  return normalized;
}

function hwRuntimeLinkTransportOptionsHtml(activeTransport) {
  const selected = hwNormalizeRuntimeLinkTransport(activeTransport) || "realtime";
  return HW_RUNTIME_LINK_TRANSPORTS.map((entry) => (
    `<option value="${escapeAttr(entry.id)}"${entry.id === selected ? " selected" : ""}>${escapeHtml(entry.label)}</option>`
  )).join("");
}

function hwParseCloudLinkTransportSection(runtimeTomlText) {
  const rawValue = hwTomlReadRawAssignment(runtimeTomlText, "runtime.cloud.links", "transports");
  if (!rawValue) return { present: false, rules: [] };

  const text = String(rawValue).trim();
  if (!text.startsWith("[") || !text.endsWith("]")) {
    return { present: true, rules: [] };
  }
  const inner = text.slice(1, -1).trim();
  if (!inner) return { present: true, rules: [] };

  const rules = [];
  for (const entry of hwTomlSplitTopLevel(inner)) {
    const table = hwTomlParseInlineTable(entry);
    if (!table || typeof table !== "object" || Array.isArray(table)) continue;
    const source = String(table.source || "").trim();
    const target = String(table.target || "").trim();
    const transport = hwNormalizeRuntimeLinkTransport(table.transport || "realtime") || "realtime";
    if (!source || !target) continue;
    rules.push({ source, target, transport });
  }
  return { present: true, rules };
}

function hwParseCloudWanAllowWriteSection(runtimeTomlText) {
  const rawValue = hwTomlReadRawAssignment(runtimeTomlText, "runtime.cloud.wan", "allow_write");
  if (!rawValue) return { present: false, rules: [] };

  const text = String(rawValue).trim();
  if (!text.startsWith("[") || !text.endsWith("]")) {
    return { present: true, rules: [] };
  }
  const inner = text.slice(1, -1).trim();
  if (!inner) return { present: true, rules: [] };

  const rules = [];
  for (const entry of hwTomlSplitTopLevel(inner)) {
    const table = hwTomlParseInlineTable(entry);
    if (!table || typeof table !== "object" || Array.isArray(table)) continue;
    const action = String(table.action || "").trim();
    const target = String(table.target || "").trim();
    if (!action || !target) continue;
    rules.push({ action, target });
  }
  return { present: true, rules };
}

function hwRuntimeCommEntriesFromToml(runtimeTomlText) {
  const sections = hwParseTomlSections(runtimeTomlText);
  const entries = [];
  for (const spec of HW_RUNTIME_COMM_SECTIONS) {
    const values = sections[spec.section];
    if (!values || typeof values !== "object") continue;
    const details = [];
    for (const key of spec.keys) {
      if (!Object.prototype.hasOwnProperty.call(values, key)) continue;
      details.push([key.replace(/_/g, " "), hwFormatPrimitive(values[key])]);
    }
    if (details.length === 0) {
      details.push(["status", "enabled"]);
    }
    entries.push({
      id: spec.id,
      label: spec.label,
      details,
    });
  }
  const cloudLinks = hwParseCloudLinkTransportSection(runtimeTomlText);
  if (cloudLinks.present) {
    const transports = Array.from(new Set(cloudLinks.rules.map((rule) => rule.transport)));
    const routes = cloudLinks.rules.map((rule) => `${rule.source}->${rule.target}`);
    const routePreview = routes.slice(0, 3).join(" | ");
    const details = [
      ["links", cloudLinks.rules.length],
      ["transports", transports.length > 0 ? transports.join(", ") : "none"],
      [
        "routes",
        routePreview || "none",
      ],
    ];
    if (routes.length > 3) {
      details.push(["more", `+${routes.length - 3}`]);
    }
    entries.push({
      id: "cloud-links",
      label: "Cloud Link Transports",
      details,
    });
  }
  const cloudWan = hwParseCloudWanAllowWriteSection(runtimeTomlText);
  if (cloudWan.present) {
    const actions = Array.from(new Set(cloudWan.rules.map((rule) => rule.action)));
    const targets = cloudWan.rules.map((rule) => rule.target);
    const targetPreview = targets.slice(0, 3).join(" | ");
    const details = [
      ["rules", cloudWan.rules.length],
      ["actions", actions.length > 0 ? actions.join(", ") : "none"],
      ["targets", targetPreview || "none"],
    ];
    if (targets.length > 3) {
      details.push(["more", `+${targets.length - 3}`]);
    }
    entries.push({
      id: "cloud-wan",
      label: "Cloud WAN Access",
      details,
    });
  }
  return entries;
}

function hwRuntimeCommModulesFromEntries(entries) {
  const modules = [];
  for (const entry of entries) {
    modules.push({
      type: `runtime-${entry.id}`,
      label: entry.label,
      driver: entry.id,
      nodeType: "comm",
      channels: 0,
      direction: null,
      preserveLabel: true,
      params: Object.fromEntries(entry.details),
    });
  }
  return modules;
}

function hwBuildAddressRows() {
  const rows = [];
  const firstByAddress = new Map();
  for (const mod of hwState.modules) {
    for (let i = 0; i < mod.addresses.length; i++) {
      const addr = mod.addresses[i];
      const isDigital = addr.startsWith("%IX") || addr.startsWith("%QX");
      const row = {
        address: addr,
        type: isDigital ? "BOOL" : "REAL",
        module: mod.label,
        moduleId: mod.id,
        channel: `Ch${i}`,
        usedInCode: hwAddressUsedInCode(addr),
        value: hwState.ioValues[addr] ?? "--",
        conflict: false,
        forced: hwState.forcedAddresses.has(addr),
      };
      const earlier = firstByAddress.get(addr);
      if (earlier) {
        row.conflict = true;
        earlier.conflict = true;
      } else {
        firstByAddress.set(addr, row);
      }
      rows.push(row);
    }
  }
  rows.sort((a, b) => a.address.localeCompare(b.address));
  return rows;
}

function hwRenderSummary() {
  const container = el.hwSummary;
  if (!container) return;

  const rows = hwBuildAddressRows();
  const modules = hwState.modules.filter((mod) => mod.paletteType !== "cpu");
  const commModules = modules.filter((mod) => hwIsCommunicationModule(mod));
  const usedCount = rows.filter((row) => row.usedInCode).length;
  const conflictCount = rows.filter((row) => row.conflict).length;
  const driverNames = new Set();
  for (const mod of modules) {
    if (mod.driver) driverNames.add(hwDriverDisplayName(mod.driver));
  }
  for (const entry of hwState.runtimeCommEntries) {
    driverNames.add(entry.label);
  }
  const safeStateCount = Array.isArray(hwState.lastIoConfig?.safe_state)
    ? hwState.lastIoConfig.safe_state.length
    : 0;
  const cards = [
    {
      label: "Modules",
      value: String(modules.length),
      note: `${commModules.length} communication`,
    },
    {
      label: "I/O Points",
      value: String(rows.length),
      note: `${usedCount} mapped in code`,
    },
    {
      label: "Active Drivers",
      value: String(driverNames.size),
      note: driverNames.size > 0 ? Array.from(driverNames).join(" • ") : "none",
    },
    {
      label: "Address Health",
      value: conflictCount > 0 ? `${conflictCount} conflicts` : "clean",
      note: `${safeStateCount} safe-state entries`,
    },
  ];

  container.innerHTML = cards.map((card) => `
    <article class="hw-summary-card">
      <span class="hw-summary-label">${escapeHtml(card.label)}</span>
      <strong class="hw-summary-value">${escapeHtml(card.value)}</strong>
      <span class="hw-summary-note">${escapeHtml(card.note)}</span>
    </article>
  `).join("");
}

function hwRenderDriverCards() {
  const container = el.hwDriverCards;
  if (!container) return;

  const cards = [];
  let ioDrivers = Array.isArray(hwState.lastIoConfig?.drivers) && hwState.lastIoConfig.drivers.length > 0
    ? hwState.lastIoConfig.drivers
    : (hwState.lastIoConfig?.driver ? [{ name: hwState.lastIoConfig.driver, params: hwState.lastIoConfig.params || {} }] : []);
  if (ioDrivers.length === 0) {
    const seen = new Set();
    ioDrivers = hwState.modules
      .filter((mod) => Boolean(mod.driver))
      .filter((mod) => {
        const key = String(mod.driver || "").toLowerCase();
        if (!key || seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .map((mod) => ({ name: mod.driver, params: mod.params || {} }));
  }
  for (const driver of ioDrivers) {
    const settingsKey = hwSettingsKeyForDriver(driver.name);
    const settingsActions = hwSettingsActionsForDriver(driver.name);
    if (settingsActions.length === 0 && settingsKey) {
      settingsActions.push({
        key: settingsKey,
        label: "Configure",
        category: hwSettingsCategoryForKey(settingsKey),
      });
    }
    cards.push({
      title: hwDriverDisplayName(driver.name),
      kind: "I/O",
      details: hwDriverDetailPairs(driver.name, driver.params),
      settingsKey,
      settingsActions,
    });
  }
  for (const runtimeEntry of hwState.runtimeCommEntries) {
    const settingsKey = hwSettingsKeyForDriver(runtimeEntry.id);
    const settingsActions = hwSettingsActionsForDriver(runtimeEntry.id);
    if (settingsActions.length === 0 && settingsKey) {
      settingsActions.push({
        key: settingsKey,
        label: "Configure",
        category: hwSettingsCategoryForKey(settingsKey),
      });
    }
    cards.push({
      title: runtimeEntry.label,
      kind: "Runtime",
      details: runtimeEntry.details,
      settingsKey,
      settingsActions,
    });
  }

  if (cards.length === 0) {
    container.innerHTML = '<div class="hw-driver-empty">No driver details available. Load or create an I/O configuration to populate communication cards.</div>';
    return;
  }

  container.innerHTML = cards.map((card) => `
    <article class="hw-driver-card">
      <div class="hw-driver-card-head">
        <h4>${escapeHtml(card.title)}</h4>
        <span class="hw-driver-kind">${escapeHtml(card.kind)}</span>
      </div>
      <dl class="hw-driver-kv">
        ${card.details.map(([key, value]) => `<div><dt>${escapeHtml(String(key))}</dt><dd>${escapeHtml(hwFormatPrimitive(value))}</dd></div>`).join("")}
      </dl>
      ${
        Array.isArray(card.settingsActions) && card.settingsActions.length > 0
          ? `<div class="hw-driver-actions">${card.settingsActions.map((action) => (
            `<button type="button" class="btn ghost" data-hw-driver-settings="${escapeAttr(action.key)}" data-hw-driver-settings-category="${escapeAttr(action.category || "all")}">${escapeHtml(action.label || "Configure")}</button>`
          )).join("")}</div>`
          : ""
      }
    </article>
  `).join("");

  container.querySelectorAll("[data-hw-driver-settings]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const key = String(btn.dataset.hwDriverSettings || "").trim();
      if (!key) return;
      const category = String(btn.dataset.hwDriverSettingsCategory || "").trim()
        || hwSettingsCategoryForKey(key);
      hwOpenSettingsForKey(key, category, {
        runtimeId: hwState.activeRuntimeId,
      });
    });
  });
}

function hwModuleNodeType(def) {
  if (def.nodeType) return def.nodeType;
  if (def.type === "cpu") return "cpu";
  if (def.direction === "input") return "input";
  if (def.direction === "output") return "output";
  if (HW_COMM_DRIVERS.has(String(def.driver || "").toLowerCase()) || def.type === "opcua") return "comm";
  return "input";
}

function hwDefaultParams(driver) {
  switch (driver) {
    case "modbus-tcp":
      return { address: "127.0.0.1:502", unit_id: 1, input_start: 0, output_start: 0, timeout_ms: 500, on_error: "fault" };
    case "mqtt":
      return {
        broker: "127.0.0.1:1883",
        client_id: "",
        topic_in: "trust/io/in",
        topic_out: "trust/io/out",
        username: "",
        password: "",
        tls: false,
        allow_insecure_remote: false,
        reconnect_ms: 500,
        keep_alive_s: 5,
      };
    case "gpio":
      return { backend: "libgpiod", chip: "/dev/gpiochip0", sysfs_base: "/sys/class/gpio", inputs: [], outputs: [] };
    case "ethercat":
      return {
        adapter: "mock",
        timeout_ms: 250,
        cycle_warn_ms: 5,
        on_error: "fault",
        modules: [],
        mock_inputs: [],
        mock_latency_ms: 0,
        mock_fail_read: false,
        mock_fail_write: false,
      };
    default:
      return {};
  }
}

function hwAddModule(paletteDef, position) {
  if (!hwState.cy) return;
  const def = paletteDef || {};
  const id = `hw-${hwState.nextModuleId++}`;
  const typeSuffix = hwState.modules.filter((m) => m.paletteType === def.type).length + 1;
  const label = def.type === "cpu"
    ? "CPU"
    : (def.preserveLabel ? (def.label || def.type) : `${def.label || def.type}-${typeSuffix}`);
  const channels = def.channels || 0;
  const direction = def.direction || null;
  const addresses = Array.isArray(def.addresses)
    ? def.addresses.slice()
    : (direction ? hwAllocateAddresses(direction, channels) : []);
  const driver = def.driver || null;
  const params = driver
    ? { ...hwDefaultParams(driver), ...(def.params && typeof def.params === "object" ? def.params : {}) }
    : (def.params && typeof def.params === "object" ? { ...def.params } : {});

  const mod = {
    id,
    label,
    paletteType: def.type,
    nodeType: hwModuleNodeType(def),
    icon: String(def.icon || hwIconForModule(def)).trim(),
    driver,
    channels,
    direction,
    addresses,
    params,
  };
  hwState.modules.push(mod);

  const pos = position || {
    x: 220 + (hwState.modules.length - 1) * 270,
    y: 220,
  };
  const nodeLabel = hwModuleLabelWithChannels(mod);
  const iconKey = hwIconForModule(mod);
  const iconStroke = hwIconStrokeForModule(mod);
  const dark = String(document.body?.dataset?.theme || "").trim().toLowerCase() === "dark";
  const cardText = hwNodeCardTextForData({
    label: nodeLabel,
    nodeType: mod.nodeType,
    cardTitle: mod.label,
  });

  hwState.cy.add({
    group: "nodes",
    data: {
      id,
      label: nodeLabel,
      cardTitle: cardText.title,
      cardSubtitle: cardText.subtitle,
      cardBadge: "",
      height: hwNodeHeightForLabel(nodeLabel),
      width: 232,
      nodeType: mod.nodeType,
      paletteType: mod.paletteType,
      type: mod.paletteType,
      driver: mod.driver || "",
      iconImage: hwCanvasIconDataUri(iconKey, iconStroke),
      cardImage: hwNodeCardDataUri(iconKey, iconStroke, {
        variant: hwNodeCardVariantForData({ nodeType: mod.nodeType }),
        dark,
        active: false,
        title: cardText.title,
        subtitle: "",
        badge: "",
      }),
      moduleRef: id,
    },
    position: pos,
  });

  // Auto-connect to CPU if not the CPU itself
  const cpuMod = hwState.modules.find((m) => m.paletteType === "cpu");
  if (cpuMod && mod.paletteType !== "cpu") {
    hwState.cy.add({
      group: "edges",
      data: {
        id: `${cpuMod.id}-${id}`,
        source: cpuMod.id,
        target: id,
      },
    });
  }

  hwState.cy.layout({ name: "preset" }).run();
  hwRenderAddressTable();
  hwRenderSummary();
  hwRenderDriverCards();
  hwUpdateEmptyState();
  hwQueuePersistIoConfig();
  return mod;
}

function hwRemoveModule(moduleId) {
  const idx = hwState.modules.findIndex((m) => m.id === moduleId);
  if (idx < 0) return;
  hwState.modules.splice(idx, 1);
  if (hwState.cy) {
    const node = hwState.cy.getElementById(moduleId);
    if (node.length) {
      hwState.cy.remove(node.connectedEdges());
      hwState.cy.remove(node);
    }
  }
  if (hwState.selectedModuleId === moduleId) {
    hwState.selectedModuleId = null;
    hwRenderPropertyPanel(null);
  }
  hwRenderAddressTable();
  hwRenderSummary();
  hwRenderDriverCards();
  hwUpdateEmptyState();
  hwQueuePersistIoConfig();
}

// ── Cytoscape Canvas ───────────────────────────────────

function hwInitCanvas() {
  const container = el.hwCanvas;
  if (!container || hwState.cy) return;
  if (typeof cytoscape === "undefined") {
    container.textContent = "Cytoscape.js not loaded.";
    return;
  }
  hwState.cy = cytoscape({
    container,
    style: hwBuildCytoscapeStyles(),
    layout: { name: "preset" },
    userZoomingEnabled: true,
    userPanningEnabled: true,
    boxSelectionEnabled: false,
    minZoom: 0.26,
    maxZoom: 4.2,
  });

  hwState.cy.on("tap", "node", (evt) => {
    hwHideHardwareContextMenus();
    const nodeId = evt.target.id();
    const runtimeMeta = hwState.fabricNodeMeta.get(nodeId);
    if (runtimeMeta) {
      hwState.selectedModuleId = null;
      hwState.selectedFabricNodeId = nodeId;
      hwState.selectedFabricEdgeId = "";
      const linkModeActive = !!hwState.linkCreateSourceRuntimeId;
      if (linkModeActive && runtimeMeta.type === "runtime") {
        const picked = String(runtimeMeta.runtimeId || "").trim();
        if (!picked) return;
        if (!hwState.linkCreateSourceRuntimeId) {
          hwState.linkCreateSourceRuntimeId = picked;
          hwSetLinkFlowHint(true, `Source selected: ${picked}. Select target runtime.`);
          if (typeof showIdeToast === "function") {
            showIdeToast(`Source selected: ${picked}. Select target runtime.`, "warn");
          }
          return;
        }
        const source = hwState.linkCreateSourceRuntimeId;
        hwState.linkCreateSourceRuntimeId = "";
        hwMarkAddLinkButtonActive(false);
        void hwCreateRuntimeCloudLink(source, picked);
        return;
      }
      hwRenderFabricNodePanel(runtimeMeta);
      return;
    }

    hwState.selectedModuleId = nodeId;
    hwState.selectedFabricNodeId = "";
    hwState.selectedFabricEdgeId = "";
    const mod = hwState.modules.find((m) => m.id === nodeId);
    hwRenderPropertyPanel(mod || null);
  });

  hwState.cy.on("dbltap", "node", (evt) => {
    const nodeId = evt.target.id();
    if (hwState.fabricNodeMeta.has(nodeId)) {
      hwShowPropertyPanel();
      return;
    }
    hwState.selectedModuleId = nodeId;
    const mod = hwState.modules.find((m) => m.id === nodeId);
    hwRenderPropertyPanel(mod || null);
    hwShowPropertyPanel();
  });

  hwState.cy.on("tap", "edge", (evt) => {
    hwHideHardwareContextMenus();
    const edgeId = evt.target.id();
    const meta = hwState.fabricEdgeMeta.get(edgeId);
    if (!meta) return;
    hwState.selectedModuleId = null;
    hwState.selectedFabricNodeId = "";
    hwState.selectedFabricEdgeId = edgeId;
    hwRenderFabricEdgePanel(meta);
  });

  hwState.cy.on("cxttap", "node", (evt) => {
    if (evt?.originalEvent && typeof evt.originalEvent.preventDefault === "function") {
      evt.originalEvent.preventDefault();
    }
    const nodeId = evt.target.id();
    const runtimeMeta = hwState.fabricNodeMeta.get(nodeId);
    if (runtimeMeta) {
      const pos = hwContextMenuEventPosition(evt);
      hwOpenNodeContextMenu(runtimeMeta, pos.x, pos.y);
      return;
    }
    const mod = hwState.modules.find((m) => m.id === nodeId);
    const address = hwPickContextAddress(mod);
    if (!address) return;
    void hwOpenAddressContextMenu(address);
  });

  hwState.cy.on("cxttap", "edge", (evt) => {
    if (evt?.originalEvent && typeof evt.originalEvent.preventDefault === "function") {
      evt.originalEvent.preventDefault();
    }
    const edgeId = evt.target.id();
    const meta = hwState.fabricEdgeMeta.get(edgeId);
    if (!meta) return;
    const pos = hwContextMenuEventPosition(evt);
    hwOpenEdgeContextMenu(meta, pos.x, pos.y);
  });

  hwState.cy.on("tap", (evt) => {
    if (evt.target === hwState.cy) {
      hwHideHardwareContextMenus();
      hwState.selectedModuleId = null;
      hwState.selectedFabricNodeId = "";
      hwState.selectedFabricEdgeId = "";
      hwState.linkCreateSourceRuntimeId = "";
      hwMarkAddLinkButtonActive(false);
      hwRenderPropertyPanel(null);
    }
  });

  // Drop from palette
  container.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  });
  container.addEventListener("drop", (e) => {
    e.preventDefault();
    try {
      const data = JSON.parse(e.dataTransfer.getData("text/plain"));
      const rect = container.getBoundingClientRect();
      const pos = {
        x: (e.clientX - rect.left),
        y: (e.clientY - rect.top),
      };
      const rendered = hwState.cy.renderer().projectIntoViewport(pos.x, pos.y);
      hwAddModule(data, { x: rendered[0], y: rendered[1] });
    } catch {
      // Ignore invalid drops
    }
  });
}

// ── Empty State / Preset Buttons ───────────────────────

function hwUpdateEmptyState() {
  const emptyEl = el.hwEmptyState;
  const workspaceEl = el.hwWorkspace;
  const canvasEl = el.hwCanvas;
  const tableEl = el.hwAddressTable;
  if (!emptyEl || !canvasEl) return;

  const hasModules = hwState.modules.length > 0;
  emptyEl.style.display = hasModules ? "none" : "";
  if (workspaceEl) {
    workspaceEl.style.opacity = hasModules ? "1" : "0.55";
  }
  if (el.hwCanvasToolbar) {
    el.hwCanvasToolbar.hidden = !hasModules || hwState.viewMode !== "canvas";
  }
  if (!hasModules) {
    hwSetLegendVisible(false);
  }
  if (hwState.viewMode === "canvas") {
    canvasEl.style.display = hasModules ? "" : "none";
    if (tableEl) tableEl.style.display = "none";
    if (hasModules && hwState.cy) {
      hwScheduleCanvasRelayout();
    }
  } else {
    canvasEl.style.display = "none";
    if (tableEl) tableEl.style.display = hasModules ? "" : "none";
  }
}

function hwRenderPresets() {
  const container = el.hwPresets;
  if (!container) return;
  container.innerHTML = "";
  for (const preset of HW_PRESETS) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "btn ghost";
    btn.textContent = preset.label;
    btn.addEventListener("click", () => hwApplyPreset(preset));
    container.appendChild(btn);
  }
}

function hwApplyPreset(preset) {
  hwResetModules();
  hwState.lastIoConfig = null;
  hwState.runtimeCommEntries = [];

  for (const spec of preset.modules) {
    const def = hwFindPaletteDef(spec.type);
    if (def) {
      const merged = { ...def };
      if (spec.driver) merged.driver = spec.driver;
      hwAddModule(merged);
    }
  }

  if (hwState.cy) {
    const layout = hwState.cy.layout({
      name: "grid",
      rows: 1,
      padding: 56,
    });
    layout.run();
    hwScheduleCanvasRelayout({ padding: 32 });
  }
  hwState.hydratedProject = null;
  hwRenderSummary();
  hwRenderDriverCards();
  hwRenderPropertyPanel(null);
  hwQueuePersistIoConfig();
}

function hwResetModules() {
  hwState.modules = [];
  hwState.fabricNodeMeta = new Map();
  hwState.fabricEdgeMeta = new Map();
  hwState.selectedFabricNodeId = "";
  hwState.selectedFabricEdgeId = "";
  hwState.linkCreateSourceRuntimeId = "";
  hwMarkAddLinkButtonActive(false);
  hwState.nextInputByte = 0;
  hwState.nextOutputByte = 0;
  hwState.nextModuleId = 1;
  hwState.selectedModuleId = null;
  hwState.lastIoConfig = null;
  hwState.runtimeCommEntries = [];
  if (hwState.cy) {
    hwState.cy.elements().remove();
  }
  hwRenderPropertyPanel(null);
  hwRenderAddressTable();
  hwRenderSummary();
  hwRenderDriverCards();
  hwUpdateEmptyState();
}

// ── View Toggle ────────────────────────────────────────

function hwSetViewMode(mode) {
  hwState.viewMode = mode;
  if (mode !== "canvas") {
    hwHideHardwareContextMenus();
  }
  if (el.hwViewCanvas) el.hwViewCanvas.classList.toggle("active", mode === "canvas");
  if (el.hwViewTable) el.hwViewTable.classList.toggle("active", mode === "table");
  if (el.hwCanvasToolbar) {
    el.hwCanvasToolbar.hidden = mode !== "canvas";
  }
  if (mode !== "canvas") {
    hwSetLegendVisible(false);
  }
  if (mode !== "canvas" && hwState.linkCreateSourceRuntimeId) {
    hwState.linkCreateSourceRuntimeId = "";
    hwMarkAddLinkButtonActive(false);
  }
  hwUpdateEmptyState();
  if (mode === "canvas" && hwState.cy) {
    hwScheduleCanvasRelayout({ padding: 32 });
  }
  if (mode === "table") {
    hwRenderAddressTable();
  }
}

function hwSetLegendVisible(visible) {
  hwState.legendVisible = !!visible;
  if (el.hwLegend) {
    el.hwLegend.hidden = !hwState.legendVisible;
  }
  if (el.hwLegendToggleBtn) {
    el.hwLegendToggleBtn.setAttribute("aria-expanded", String(hwState.legendVisible));
  }
}

function hwContextMenuEventPosition(evt) {
  const event = evt?.originalEvent;
  if (event && Number.isFinite(event.clientX) && Number.isFinite(event.clientY)) {
    return { x: Number(event.clientX), y: Number(event.clientY) };
  }
  const rendered = evt?.renderedPosition || evt?.position || { x: 0, y: 0 };
  const rect = el.hwCanvas?.getBoundingClientRect();
  return {
    x: Number((rect?.left || 0) + (rendered.x || 0)),
    y: Number((rect?.top || 0) + (rendered.y || 0)),
  };
}

function hwPositionContextMenu(menu, x, y) {
  if (!menu) return;
  const safeX = Math.max(8, Math.floor(Number(x) || 0));
  const safeY = Math.max(8, Math.floor(Number(y) || 0));
  menu.style.left = `${safeX}px`;
  menu.style.top = `${safeY}px`;
}

function hwHideNodeContextMenu() {
  if (el.hwNodeContextMenu) {
    el.hwNodeContextMenu.classList.add("ide-hidden");
  }
  hwState.contextRuntimeMeta = null;
}

function hwHideEdgeContextMenu() {
  if (el.hwEdgeContextMenu) {
    el.hwEdgeContextMenu.classList.add("ide-hidden");
  }
  hwState.contextEdgeMeta = null;
}

function hwHideHardwareContextMenus() {
  hwHideNodeContextMenu();
  hwHideEdgeContextMenu();
}

function hwOpenNodeContextMenu(meta, x, y) {
  if (!el.hwNodeContextMenu || !meta) return;
  hwHideEdgeContextMenu();
  const runtimeIds = Array.isArray(meta.runtimeIds)
    ? meta.runtimeIds.map((value) => String(value || "").trim()).filter(Boolean)
    : [];
  const type = String(meta.type || "").trim().toLowerCase();
  const runtimeId = type === "runtime"
    ? String(meta.runtimeId || "").trim()
    : (runtimeIds[0] || String(hwState.activeRuntimeId || "").trim());
  hwState.contextRuntimeMeta = {
    type,
    proto: String(meta.proto || "").trim().toLowerCase(),
    label: String(meta.label || "").trim(),
    runtimeId,
    runtimeIds,
  };
  const isRuntimeNode = type === "runtime";
  if (el.hwCtxCreateLinkBtn) {
    el.hwCtxCreateLinkBtn.disabled = !isRuntimeNode;
  }
  if (el.hwCtxRuntimeSettingsBtn) {
    el.hwCtxRuntimeSettingsBtn.disabled = !hwState.contextRuntimeMeta.runtimeId;
    el.hwCtxRuntimeSettingsBtn.textContent = isRuntimeNode ? "Open Runtime Settings" : "Open Endpoint Settings";
    el.hwCtxRuntimeSettingsBtn.classList.remove("ide-hidden");
  }
  if (el.hwCtxRuntimeCommSettingsBtn) {
    if (isRuntimeNode) {
      el.hwCtxRuntimeCommSettingsBtn.disabled = !hwState.contextRuntimeMeta.runtimeId;
      el.hwCtxRuntimeCommSettingsBtn.textContent = "Open Communication Settings";
      el.hwCtxRuntimeCommSettingsBtn.classList.remove("ide-hidden");
    } else {
      el.hwCtxRuntimeCommSettingsBtn.disabled = true;
      el.hwCtxRuntimeCommSettingsBtn.classList.add("ide-hidden");
    }
  }
  hwPositionContextMenu(el.hwNodeContextMenu, x, y);
  el.hwNodeContextMenu.classList.remove("ide-hidden");
}

function hwOpenEdgeContextMenu(meta, x, y) {
  if (!el.hwEdgeContextMenu || !meta) return;
  hwHideNodeContextMenu();
  hwState.contextEdgeMeta = { ...meta };
  const isRuntimeLink = String(meta.type || "") === "runtime_cloud";
  if (el.hwCtxCreateLinkFromEdgeBtn) {
    el.hwCtxCreateLinkFromEdgeBtn.disabled = !isRuntimeLink;
  }
  if (el.hwCtxDeleteLinkBtn) {
    const canDelete = meta.type === "runtime_cloud" || meta.type === "mesh";
    el.hwCtxDeleteLinkBtn.disabled = !canDelete;
  }
  hwPositionContextMenu(el.hwEdgeContextMenu, x, y);
  el.hwEdgeContextMenu.classList.remove("ide-hidden");
}

function hwSetInspectorCollapsed(collapsed) {
  hwState.inspectorCollapsed = !!collapsed;
  if (el.hwWorkspace) {
    el.hwWorkspace.classList.toggle("inspector-collapsed", hwState.inspectorCollapsed);
  }
  if (el.hwPropertyPanel) {
    el.hwPropertyPanel.hidden = hwState.inspectorCollapsed;
  }
  if (el.hwToggleInspectorBtn) {
    el.hwToggleInspectorBtn.dataset.active = hwState.inspectorCollapsed ? "false" : "true";
    el.hwToggleInspectorBtn.setAttribute("aria-pressed", String(!hwState.inspectorCollapsed));
    el.hwToggleInspectorBtn.textContent = hwState.inspectorCollapsed ? "Inspector" : "Inspector On";
  }
  if (!hwState.inspectorCollapsed) {
    const selected = hwState.modules.find((entry) => entry.id === hwState.selectedModuleId) || null;
    hwRenderPropertyPanel(selected);
  }
  hwScheduleCanvasRelayout({ padding: 32 });
}

function hwSetDriversCollapsed(collapsed) {
  hwState.driversCollapsed = !!collapsed;
  if (el.hwDriversPanel) {
    el.hwDriversPanel.classList.toggle("is-collapsed", hwState.driversCollapsed);
  }
  if (el.hwDriversPanelToggleBtn) {
    el.hwDriversPanelToggleBtn.setAttribute("aria-expanded", String(!hwState.driversCollapsed));
    el.hwDriversPanelToggleBtn.textContent = hwState.driversCollapsed ? "Expand" : "Collapse";
  }
  if (el.hwToggleDriversBtn) {
    el.hwToggleDriversBtn.dataset.active = hwState.driversCollapsed ? "false" : "true";
    el.hwToggleDriversBtn.setAttribute("aria-pressed", String(!hwState.driversCollapsed));
    el.hwToggleDriversBtn.textContent = hwState.driversCollapsed ? "Drivers" : "Drivers On";
  }
  hwScheduleCanvasRelayout({ padding: 32 });
}

function hwSurfaceCardElement() {
  return document.querySelector("#ideTabPanel_hardware .hw-surface-card");
}

function hwSyncFullscreenButtonState() {
  const fullscreenActive = !!document.fullscreenElement || hwState.isCanvasFullscreen;
  if (el.hwFullscreenBtn) {
    el.hwFullscreenBtn.textContent = fullscreenActive ? "Exit Fullscreen" : "Fullscreen";
  }
}

async function hwToggleCanvasFullscreen() {
  const card = hwSurfaceCardElement();
  if (!card) return;
  const nativeSupported = typeof card.requestFullscreen === "function";
  if (nativeSupported) {
    if (document.fullscreenElement) {
      try {
        await document.exitFullscreen();
      } catch {
        // keep local fallback below
      }
    } else {
      try {
        await card.requestFullscreen();
        hwState.isCanvasFullscreen = true;
        hwSyncFullscreenButtonState();
        hwScheduleCanvasRelayout({ padding: 30 });
        return;
      } catch {
        // fallback below
      }
    }
  }
  hwState.isCanvasFullscreen = !hwState.isCanvasFullscreen;
  card.classList.toggle("is-local-fullscreen", hwState.isCanvasFullscreen);
  hwSyncFullscreenButtonState();
  hwScheduleCanvasRelayout({ padding: 30 });
}

function hwEstimateDigitalChannelsForDirection(config, direction) {
  const safe = Array.isArray(config?.safe_state) ? config.safe_state : [];
  let maxBit = -1;
  const prefix = direction === "output" ? "%QX" : "%IX";
  for (const entry of safe) {
    const address = String(entry?.address || "");
    const match = address.match(new RegExp(`^${prefix}(\\d+)\\.(\\d+)$`));
    if (!match) continue;
    const byte = Number(match[1]) || 0;
    const bit = Number(match[2]) || 0;
    const index = byte * 8 + bit;
    if (index > maxBit) maxBit = index;
  }
  return maxBit >= 0 ? maxBit + 1 : 0;
}

function hwPickDigitalType(direction, channels) {
  if (direction === "input") {
    if (channels > 8) return "di-16";
    return "di-8";
  }
  if (channels > 8) return "do-16";
  return "do-8";
}

function hwReadArrayLength(value) {
  return Array.isArray(value) ? value.length : 0;
}

function hwDriverModulesFromConfig(driverConfig, fullConfig) {
  const name = String(driverConfig?.name || "").trim().toLowerCase();
  const params = (driverConfig && typeof driverConfig.params === "object" && driverConfig.params)
    ? driverConfig.params
    : {};
  const modules = [];

  if (name === "ethercat") {
    modules.push({
      type: "ethercat",
      label: "EtherCAT Coupler",
      driver: "ethercat",
      direction: "input",
      channels: 16,
      params,
      preserveLabel: true,
    });
    const busModules = Array.isArray(params.modules) ? params.modules : [];
    for (const mod of busModules) {
      const model = String(mod?.model || "").trim();
      if (!model || /^EK/i.test(model)) continue;
      let direction = "input";
      if (/^EL2/i.test(model)) direction = "output";
      const channels = Number(mod?.channels) > 0 ? Number(mod.channels) : (/008/.test(model) ? 8 : 4);
      const type = hwPickDigitalType(direction, channels);
      const palette = hwFindPaletteDef(type) || {};
      modules.push({
        type,
        label: model,
        driver: "ethercat",
        direction: palette.direction || direction,
        channels,
        params: { model, slot: mod?.slot },
        preserveLabel: true,
      });
    }
    return modules;
  }

  if (name === "modbus-tcp") {
    modules.push({
      type: "modbus-tcp",
      label: "Modbus TCP",
      driver: "modbus-tcp",
      direction: "input",
      channels: 16,
      params,
      preserveLabel: true,
    });
    return modules;
  }

  if (name === "mqtt") {
    modules.push({
      type: "mqtt-bridge",
      label: "MQTT Bridge",
      driver: "mqtt",
      direction: "input",
      channels: 8,
      params,
      preserveLabel: true,
    });
    return modules;
  }

  if (name === "gpio") {
    const inputCount = Math.max(
      hwReadArrayLength(params.inputs),
      hwReadArrayLength(params.input),
      hwEstimateDigitalChannelsForDirection(fullConfig, "input"),
    );
    const outputCount = Math.max(
      hwReadArrayLength(params.outputs),
      hwReadArrayLength(params.output),
      hwEstimateDigitalChannelsForDirection(fullConfig, "output"),
    );
    if (inputCount > 0) {
      const type = hwPickDigitalType("input", inputCount);
      const palette = hwFindPaletteDef(type) || {};
      modules.push({
        type,
        label: "GPIO Inputs",
        driver: "gpio",
        direction: palette.direction || "input",
        channels: inputCount,
        params,
        preserveLabel: true,
      });
    }
    if (outputCount > 0) {
      const type = hwPickDigitalType("output", outputCount);
      const palette = hwFindPaletteDef(type) || {};
      modules.push({
        type,
        label: "GPIO Outputs",
        driver: "gpio",
        direction: palette.direction || "output",
        channels: outputCount,
        params,
        preserveLabel: true,
      });
    }
    if (modules.length === 0) {
      modules.push({
        type: "do-8",
        label: "GPIO I/O",
        driver: "gpio",
        direction: "output",
        channels: 8,
        params,
        preserveLabel: true,
      });
    }
    return modules;
  }

  if (name === "simulated" || name === "loopback") {
    const inputCount = hwEstimateDigitalChannelsForDirection(fullConfig, "input");
    const outputCount = hwEstimateDigitalChannelsForDirection(fullConfig, "output");
    if (inputCount > 0) {
      const type = hwPickDigitalType("input", inputCount);
      const palette = hwFindPaletteDef(type) || {};
      modules.push({
        type,
        label: "Sim Inputs",
        driver: "simulated",
        direction: palette.direction || "input",
        channels: inputCount,
        params,
        preserveLabel: true,
      });
    }
    if (outputCount > 0) {
      const type = hwPickDigitalType("output", outputCount);
      const palette = hwFindPaletteDef(type) || {};
      modules.push({
        type,
        label: "Sim Outputs",
        driver: "simulated",
        direction: palette.direction || "output",
        channels: outputCount,
        params,
        preserveLabel: true,
      });
    }
    if (modules.length === 0) {
      modules.push({
        type: "di-8",
        label: "Simulated I/O",
        driver: "simulated",
        direction: "input",
        channels: 8,
        params,
        preserveLabel: true,
      });
    }
    return modules;
  }

  modules.push({
    type: `${name || "driver"}-bridge`,
    label: (name || "Driver").toUpperCase(),
    driver: name || null,
    direction: null,
    nodeType: "comm",
    channels: 0,
    params,
    preserveLabel: true,
  });
  return modules;
}

function hwResolveIoDrivers(config) {
  if (Array.isArray(config?.drivers) && config.drivers.length > 0) {
    return config.drivers;
  }
  const legacyDriver = String(config?.driver || "").trim();
  if (!legacyDriver) {
    return [];
  }
  return [{
    name: legacyDriver,
    params: (config?.params && typeof config.params === "object") ? config.params : {},
  }];
}

function hwApplyIoConfigToCanvas(config, projectPath, runtimeTomlText) {
  if (!hwState.cy) return;
  const drivers = hwResolveIoDrivers(config);
  const runtimeEntries = hwRuntimeCommEntriesFromToml(runtimeTomlText);

  hwResetModules();
  hwState.lastIoConfig = config || null;
  hwState.runtimeCommEntries = runtimeEntries;
  hwState.lastPersistFingerprint = JSON.stringify({
    drivers: drivers.map((driver) => ({
      name: String(driver.name || "").trim().toLowerCase(),
      params: (driver.params && typeof driver.params === "object") ? driver.params : {},
    })),
    safe_state: Array.isArray(config?.safe_state) ? config.safe_state : [],
    use_system_io: !!config?.use_system_io,
  });

  const cpu = hwFindPaletteDef("cpu");
  if (cpu) hwAddModule(cpu);

  const moduleKeys = new Set();
  for (const driver of drivers) {
    const derived = hwDriverModulesFromConfig(driver, config);
    for (const moduleDef of derived) {
      const key = `${String(moduleDef.driver || "none").toLowerCase()}:${String(moduleDef.label || moduleDef.type || "")}`;
      if (moduleKeys.has(key)) continue;
      moduleKeys.add(key);
      hwAddModule(moduleDef);
    }
  }

  if (hwState.cy && hwState.modules.length > 0) {
    const layout = hwState.cy.layout({
      name: "breadthfirst",
      directed: true,
      spacingFactor: 1.55,
      padding: 44,
    });
    if (typeof layout.one === "function") {
      layout.one("layoutstop", () => {
        hwScheduleCanvasRelayout({ padding: 36 });
      });
    }
    layout.run();
    hwScheduleCanvasRelayout({ padding: 36 });
  }

  hwRenderAddressTable();
  hwRenderSummary();
  hwRenderDriverCards();
  hwRenderPropertyPanel(null);
  hwUpdateEmptyState();
  hwState.hydratedProject = projectPath || "";
}

function hwFabricSanitizeId(text) {
  return String(text || "")
    .trim()
    .replace(/[^a-zA-Z0-9:_-]+/g, "_")
    .replace(/^_+/, "")
    .replace(/_+$/, "");
}

function hwRuntimeNodeId(runtimeId) {
  return `runtime:${hwFabricSanitizeId(runtimeId)}`;
}

function hwEndpointNodeId(proto, key) {
  return `endpoint:${hwFabricSanitizeId(proto)}:${hwFabricSanitizeId(key)}`;
}

function hwModelByRuntimeId(runtimeId) {
  const key = String(runtimeId || "").trim();
  if (!key) return null;
  return hwState.workspaceRuntimes.find((entry) => entry.runtimeId === key) || null;
}

function hwEnsureEndpointNode(proto, key, label, nodes, endpointMetaMap) {
  const nodeId = hwEndpointNodeId(proto, key || label || proto);
  if (endpointMetaMap.has(nodeId)) {
    return nodeId;
  }
  const formattedLabel = hwFormatFabricLabel(proto, label || key || proto || "Endpoint");
  const cardText = hwCardLabelParts(formattedLabel);
  const iconKey = hwProtocolIcon(proto);
  const iconStroke = hwProtocolIconStroke(proto);
  const dark = String(document.body?.dataset?.theme || "").trim().toLowerCase() === "dark";
  endpointMetaMap.set(nodeId, true);
  nodes.push({
    group: "nodes",
      data: {
        id: nodeId,
        fabric: "true",
        kind: "endpoint",
        proto: String(proto || "comm"),
        label: formattedLabel,
        cardTitle: cardText.title,
        cardSubtitle: cardText.subtitle,
        cardBadge: "",
        height: 84,
        width: 220,
        iconImage: hwCanvasIconDataUri(iconKey, iconStroke),
        cardImage: hwNodeCardDataUri(iconKey, iconStroke, {
          variant: "endpoint",
          dark,
          active: false,
          title: cardText.title,
          subtitle: "",
          badge: "",
        }),
      },
    });
  return nodeId;
}

function hwEnsureEndpointNodeMeta(nodeMeta, endpointId, proto, label, runtimeId) {
  if (!nodeMeta || !endpointId) return;
  const existing = nodeMeta.get(endpointId);
  const runtimeSet = new Set(Array.isArray(existing?.runtimeIds) ? existing.runtimeIds : []);
  const normalizedRuntimeId = String(runtimeId || "").trim();
  if (normalizedRuntimeId) {
    runtimeSet.add(normalizedRuntimeId);
  }
  nodeMeta.set(endpointId, {
    type: "endpoint",
    proto: String(proto || "").trim(),
    label: String(label || "").trim(),
    runtimeIds: Array.from(runtimeSet),
  });
}

function hwBuildCommunicationFabric() {
  const runtimeModels = Array.isArray(hwState.workspaceRuntimes) ? hwState.workspaceRuntimes : [];
  const runtimeIds = new Set(runtimeModels.map((entry) => String(entry.runtimeId || "").trim()).filter(Boolean));
  const nodes = [];
  const edges = [];
  const endpointMetaMap = new Map();
  const nodeMeta = new Map();
  const edgeMeta = new Map();
  const seenEdges = new Set();

  for (const model of runtimeModels) {
    const runtimeId = String(model.runtimeId || "").trim();
    if (!runtimeId) continue;
    const nodeId = hwRuntimeNodeId(runtimeId);
    const runtimeSection = model.runtimeSections?.runtime || {};
    const resourceSection = model.runtimeSections?.resource || {};
    const webListen = String(model.webListen || runtimeSection.listen || "").trim();
    const runtimeLabel = webListen ? `${runtimeId}\n${webListen}` : runtimeId;
    const runtimeCard = hwCardLabelParts(runtimeLabel);
    const runtimeIconStroke = hwProtocolIconStroke("runtime");
    const dark = String(document.body?.dataset?.theme || "").trim().toLowerCase() === "dark";
    nodes.push({
      group: "nodes",
      data: {
        id: nodeId,
        fabric: "true",
        kind: "runtime",
        proto: "runtime",
        label: runtimeLabel,
        runtimeId,
        webListen,
        cardTitle: runtimeCard.title || runtimeId,
        cardSubtitle: runtimeCard.subtitle || "",
        cardBadge: "",
        activeRuntime: runtimeId === hwState.activeRuntimeId ? "true" : "false",
        height: 94,
        width: 248,
        iconImage: hwCanvasIconDataUri("runtime", runtimeIconStroke),
        cardImage: hwNodeCardDataUri("runtime", runtimeIconStroke, {
          variant: "runtime",
          dark,
          active: runtimeId === hwState.activeRuntimeId,
          title: runtimeCard.title || runtimeId,
          subtitle: "",
          badge: "",
        }),
      },
    });
    nodeMeta.set(nodeId, {
      type: "runtime",
      runtimeId,
      runtimeRoot: model.runtimeRoot || "",
      webListen,
      hostGroup: model.hostGroup || "",
      cycleMs: resourceSection.cycle_interval_ms,
    });
  }

  const activeRuntimeNodeId = hwRuntimeNodeId(hwState.activeRuntimeId);
  const activeCpu = hwState.modules.find((entry) => entry.paletteType === "cpu");
  if (activeCpu && runtimeIds.has(hwState.activeRuntimeId)) {
    const edgeId = `fabric:internal:${hwFabricSanitizeId(hwState.activeRuntimeId)}:${hwFabricSanitizeId(activeCpu.id)}`;
    edges.push({
      group: "edges",
      data: {
        id: edgeId,
        source: activeRuntimeNodeId,
        target: activeCpu.id,
        fabric: "true",
        kind: "edge",
        proto: "internal",
        label: "I/O backplane",
      },
    });
    edgeMeta.set(edgeId, {
      type: "internal",
      runtimeId: hwState.activeRuntimeId,
    });
  }

  for (const model of runtimeModels) {
    const runtimeId = String(model.runtimeId || "").trim();
    if (!runtimeId) continue;
    const runtimeNodeId = hwRuntimeNodeId(runtimeId);
    const ioConfig = (model.ioConfig && typeof model.ioConfig === "object") ? model.ioConfig : { drivers: [] };
    const drivers = Array.isArray(ioConfig.drivers) ? ioConfig.drivers : [];
    const runtimeSections = model.runtimeSections || {};

    for (let idx = 0; idx < drivers.length; idx += 1) {
      const driver = drivers[idx] || {};
      const name = String(driver.name || "").trim().toLowerCase();
      const params = (driver.params && typeof driver.params === "object" && !Array.isArray(driver.params))
        ? driver.params
        : {};

      if (name === "mqtt") {
        const broker = String(params.broker || "").trim() || `${runtimeId}:mqtt`;
        const endpointId = hwEnsureEndpointNode("mqtt", broker, `MQTT ${broker}`, nodes, endpointMetaMap);
        hwEnsureEndpointNodeMeta(nodeMeta, endpointId, "mqtt", `MQTT ${broker}`, runtimeId);
        const edgeId = `fabric:mqtt:${hwFabricSanitizeId(runtimeId)}:${hwFabricSanitizeId(broker)}:${idx}`;
        edges.push({
          group: "edges",
          data: {
            id: edgeId,
            source: runtimeNodeId,
            target: endpointId,
            fabric: "true",
            kind: "edge",
            proto: "mqtt",
            label: params.topic_out ? `topic ${params.topic_out}` : "pub/sub",
          },
        });
        edgeMeta.set(edgeId, {
          type: "mqtt",
          runtimeId,
          driverIndex: idx,
          broker,
          topicIn: String(params.topic_in || ""),
          topicOut: String(params.topic_out || ""),
          tls: !!params.tls,
        });
      } else if (name === "modbus-tcp") {
        const address = String(params.address || "").trim() || `${runtimeId}:502`;
        const endpointId = hwEnsureEndpointNode("modbus", address, `PLC ${address}`, nodes, endpointMetaMap);
        hwEnsureEndpointNodeMeta(nodeMeta, endpointId, "modbus", `PLC ${address}`, runtimeId);
        const edgeId = `fabric:modbus:${hwFabricSanitizeId(runtimeId)}:${hwFabricSanitizeId(address)}:${idx}`;
        edges.push({
          group: "edges",
          data: {
            id: edgeId,
            source: runtimeNodeId,
            target: endpointId,
            fabric: "true",
            kind: "edge",
            proto: "modbus",
            label: `unit ${params.unit_id ?? 1}`,
          },
        });
        edgeMeta.set(edgeId, {
          type: "modbus",
          runtimeId,
          driverIndex: idx,
          address,
          unitId: Number(params.unit_id ?? 1) || 1,
          timeoutMs: Number(params.timeout_ms ?? 500) || 500,
        });
      } else if (name === "ethercat") {
        const adapter = String(params.adapter || "").trim() || "mock";
        const endpointId = hwEnsureEndpointNode("ethercat", adapter, `EtherCAT ${adapter}`, nodes, endpointMetaMap);
        hwEnsureEndpointNodeMeta(nodeMeta, endpointId, "ethercat", `EtherCAT ${adapter}`, runtimeId);
        const edgeId = `fabric:ethercat:${hwFabricSanitizeId(runtimeId)}:${hwFabricSanitizeId(adapter)}:${idx}`;
        edges.push({
          group: "edges",
          data: {
            id: edgeId,
            source: runtimeNodeId,
            target: endpointId,
            fabric: "true",
            kind: "edge",
            proto: "ethercat",
            label: `${Array.isArray(params.modules) ? params.modules.length : 0} modules`,
          },
        });
        edgeMeta.set(edgeId, {
          type: "ethercat",
          runtimeId,
          driverIndex: idx,
          adapter,
          timeoutMs: Number(params.timeout_ms ?? 250) || 250,
        });
      }
    }

    const opcua = runtimeSections["runtime.opcua"] || {};
    if (opcua && typeof opcua === "object" && Object.keys(opcua).length > 0) {
      const listen = String(opcua.listen || "0.0.0.0:4840").trim();
      const endpointId = hwEnsureEndpointNode("opcua", `${runtimeId}:${listen}`, `OPC UA ${listen}`, nodes, endpointMetaMap);
      hwEnsureEndpointNodeMeta(nodeMeta, endpointId, "opcua", `OPC UA ${listen}`, runtimeId);
      const edgeId = `fabric:opcua:${hwFabricSanitizeId(runtimeId)}:${hwFabricSanitizeId(listen)}`;
      edges.push({
        group: "edges",
        data: {
          id: edgeId,
          source: runtimeNodeId,
          target: endpointId,
          fabric: "true",
          kind: "edge",
          proto: "opcua",
          label: String(opcua.security_mode || "none"),
        },
      });
      edgeMeta.set(edgeId, {
        type: "opcua",
        runtimeId,
        listen,
        securityMode: String(opcua.security_mode || "none"),
      });
    }

    const discovery = runtimeSections["runtime.discovery"] || {};
    if (discovery && typeof discovery === "object" && Object.keys(discovery).length > 0) {
      const service = String(discovery.service_name || runtimeId).trim();
      const endpointId = hwEnsureEndpointNode("discovery", `${service}:${runtimeId}`, `Discovery ${service}`, nodes, endpointMetaMap);
      hwEnsureEndpointNodeMeta(nodeMeta, endpointId, "discovery", `Discovery ${service}`, runtimeId);
      const edgeId = `fabric:discovery:${hwFabricSanitizeId(runtimeId)}:${hwFabricSanitizeId(service)}`;
      edges.push({
        group: "edges",
        data: {
          id: edgeId,
          source: runtimeNodeId,
          target: endpointId,
          fabric: "true",
          kind: "edge",
          proto: "discovery",
          label: discovery.advertise === false ? "passive" : "advertise",
        },
      });
      edgeMeta.set(edgeId, {
        type: "discovery",
        runtimeId,
        serviceName: service,
      });
    }

    const web = runtimeSections["runtime.web"] || {};
    if (web && typeof web === "object" && Object.keys(web).length > 0) {
      const listen = String(web.listen || model.webListen || "0.0.0.0:8080").trim();
      const endpointId = hwEnsureEndpointNode("web", `${runtimeId}:${listen}`, `Web ${listen}`, nodes, endpointMetaMap);
      hwEnsureEndpointNodeMeta(nodeMeta, endpointId, "web", `Web ${listen}`, runtimeId);
      const edgeId = `fabric:web:${hwFabricSanitizeId(runtimeId)}:${hwFabricSanitizeId(listen)}`;
      edges.push({
        group: "edges",
        data: {
          id: edgeId,
          source: runtimeNodeId,
          target: endpointId,
          fabric: "true",
          kind: "edge",
          proto: "web",
          label: String(web.auth || "local"),
        },
      });
      edgeMeta.set(edgeId, {
        type: "web",
        runtimeId,
        listen,
        auth: String(web.auth || "local"),
        tls: !!web.tls,
      });
    }

    const mesh = runtimeSections["runtime.mesh"] || {};
    if (mesh && typeof mesh === "object") {
      const connect = Array.isArray(mesh.connect) ? mesh.connect : [];
      for (let idx = 0; idx < connect.length; idx += 1) {
        const peer = String(connect[idx] || "").trim();
        if (!peer) continue;
        const endpointId = hwEnsureEndpointNode("mesh", `${runtimeId}:${peer}`, `Mesh ${peer}`, nodes, endpointMetaMap);
        hwEnsureEndpointNodeMeta(nodeMeta, endpointId, "mesh", `Mesh ${peer}`, runtimeId);
        const edgeId = `fabric:mesh:${hwFabricSanitizeId(runtimeId)}:${hwFabricSanitizeId(peer)}:${idx}`;
        edges.push({
          group: "edges",
          data: {
            id: edgeId,
            source: runtimeNodeId,
            target: endpointId,
            fabric: "true",
            kind: "edge",
            proto: "mesh",
            label: mesh.tls ? "tls" : "plain",
          },
        });
        edgeMeta.set(edgeId, {
          type: "mesh",
          runtimeId,
          connectIndex: idx,
          peer,
          tls: !!mesh.tls,
        });
      }
    }

    const links = Array.isArray(model.runtimeCloudLinks) ? model.runtimeCloudLinks : [];
    for (let idx = 0; idx < links.length; idx += 1) {
      const rule = links[idx] || {};
      const sourceId = String(rule.source || "").trim();
      const targetId = String(rule.target || "").trim();
      const transport = String(rule.transport || "").trim().toLowerCase() || "realtime";
      if (!sourceId || !targetId) continue;
      const sourceNodeId = runtimeIds.has(sourceId)
        ? hwRuntimeNodeId(sourceId)
        : hwEnsureEndpointNode("runtime-external", sourceId, sourceId, nodes, endpointMetaMap);
      if (!runtimeIds.has(sourceId)) {
        hwEnsureEndpointNodeMeta(nodeMeta, sourceNodeId, "runtime-external", sourceId, model.runtimeId);
      }
      const targetNodeId = runtimeIds.has(targetId)
        ? hwRuntimeNodeId(targetId)
        : hwEnsureEndpointNode("runtime-external", targetId, targetId, nodes, endpointMetaMap);
      if (!runtimeIds.has(targetId)) {
        hwEnsureEndpointNodeMeta(nodeMeta, targetNodeId, "runtime-external", targetId, model.runtimeId);
      }
      const edgeId = `fabric:cloud:${hwFabricSanitizeId(model.runtimeId)}:${idx}:${hwFabricSanitizeId(sourceId)}:${hwFabricSanitizeId(targetId)}`;
      const dedupeKey = `${sourceNodeId}->${targetNodeId}:${transport}:${model.runtimeId}`;
      if (seenEdges.has(dedupeKey)) continue;
      seenEdges.add(dedupeKey);
      edges.push({
        group: "edges",
        data: {
          id: edgeId,
          source: sourceNodeId,
          target: targetNodeId,
          fabric: "true",
          kind: "edge",
          proto: "runtime_cloud",
          transport,
          label: transport,
        },
      });
      edgeMeta.set(edgeId, {
        type: "runtime_cloud",
        ownerRuntimeId: model.runtimeId,
        ruleIndex: idx,
        source: sourceId,
        target: targetId,
        transport,
      });
    }
  }

  return { nodes, edges, nodeMeta, edgeMeta };
}

function hwRuntimeIdsLinkedToEndpoint(node, runtimeNodeIds) {
  const ids = new Set();
  if (!node || !runtimeNodeIds || runtimeNodeIds.size === 0) return [];
  node.connectedEdges("[fabric='true']").forEach((edge) => {
    const source = String(edge.data("source") || "").trim();
    const target = String(edge.data("target") || "").trim();
    if (runtimeNodeIds.has(source)) ids.add(source);
    if (runtimeNodeIds.has(target)) ids.add(target);
  });
  return Array.from(ids);
}

function hwApplyFabricPresetLayout() {
  if (!hwState.cy) return;
  const cy = hwState.cy;
  const runtimeNodes = cy.nodes("[fabric='true'][kind='runtime']");
  const endpointNodes = cy.nodes("[fabric='true'][kind='endpoint']");
  const moduleNodes = cy.nodes().filter(
    (node) => String(node.data("fabric") || "") !== "true",
  );
  if (runtimeNodes.length === 0) return;

  const runtimeSpacing = runtimeNodes.length <= 1
    ? 0
    : (
        runtimeNodes.length === 2
          ? 520
          : Math.max(380, Math.min(640, 980 / Math.max(1, runtimeNodes.length - 1)))
      );
  const runtimeY = -40;
  const runtimePositions = new Map();
  const runtimeNodeIds = new Set();

  runtimeNodes.forEach((node, index) => {
    const x = (index - (runtimeNodes.length - 1) / 2) * runtimeSpacing;
    const y = runtimeY;
    node.position({ x, y });
    runtimePositions.set(node.id(), { x, y });
    runtimeNodeIds.add(node.id());
  });

  const endpointSingleRuntime = new Map();
  const endpointSharedRuntime = [];
  const endpointOrphan = [];

  endpointNodes.forEach((node) => {
    const linkedRuntimeIds = hwRuntimeIdsLinkedToEndpoint(node, runtimeNodeIds);
    if (linkedRuntimeIds.length === 1) {
      const runtimeId = linkedRuntimeIds[0];
      if (!endpointSingleRuntime.has(runtimeId)) {
        endpointSingleRuntime.set(runtimeId, []);
      }
      endpointSingleRuntime.get(runtimeId).push(node);
      return;
    }
    if (linkedRuntimeIds.length > 1) {
      endpointSharedRuntime.push({ node, linkedRuntimeIds });
      return;
    }
    endpointOrphan.push(node);
  });

  const runtimeFanRadiusX = 360;
  const runtimeFanRadiusY = 220;
  for (const [runtimeId, nodes] of endpointSingleRuntime.entries()) {
    const anchor = runtimePositions.get(runtimeId) || { x: 0, y: runtimeY };
    const ordered = nodes.slice().sort((left, right) => {
      const leftLabel = String(left.data("label") || "");
      const rightLabel = String(right.data("label") || "");
      return leftLabel.localeCompare(rightLabel);
    });
    const total = ordered.length;
    const start = (-146 * Math.PI) / 180;
    const end = (36 * Math.PI) / 180;
    ordered.forEach((node, index) => {
      const t = total <= 1 ? 0.5 : index / Math.max(1, total - 1);
      const angle = start + ((end - start) * t);
      const x = anchor.x + Math.cos(angle) * runtimeFanRadiusX;
      const y = anchor.y + Math.sin(angle) * runtimeFanRadiusY;
      node.position({ x, y });
    });
  }

  endpointSharedRuntime.forEach((entry, index) => {
    const points = entry.linkedRuntimeIds
      .map((runtimeId) => runtimePositions.get(runtimeId))
      .filter(Boolean);
    const center = points.reduce(
      (acc, point) => {
        acc.x += point.x;
        acc.y += point.y;
        return acc;
      },
      { x: 0, y: 0 },
    );
    const count = Math.max(1, points.length);
    const col = index % 4;
    const row = Math.floor(index / 4);
    const x = (center.x / count) + ((col - 1.5) * 172);
    const y = (center.y / count) - 292 - (row * 124);
    entry.node.position({ x, y });
  });

  endpointOrphan.forEach((node, index) => {
    const col = index % 5;
    const row = Math.floor(index / 5);
    const x = (col - 2) * 190;
    const y = 312 + (row * 112);
    node.position({ x, y });
  });

  if (moduleNodes.length > 0) {
    const activeRuntimeNodeId = hwRuntimeNodeId(hwState.activeRuntimeId);
    const activeAnchor = runtimePositions.get(activeRuntimeNodeId)
      || runtimePositions.values().next().value
      || { x: 0, y: runtimeY };
    const cols = Math.min(3, Math.max(1, Math.ceil(Math.sqrt(moduleNodes.length))));
    const colGap = 220;
    const rowGap = 112;
    const baseX = activeAnchor.x - (((cols - 1) * colGap) / 2);
    const baseY = activeAnchor.y + 220;
    moduleNodes.forEach((node, index) => {
      const col = index % cols;
      const row = Math.floor(index / cols);
      node.position({
        x: baseX + (col * colGap),
        y: baseY + (row * rowGap),
      });
    });
  }
}

function hwApplyCommunicationFabric() {
  if (!hwState.cy) return;
  const existingFabric = hwState.cy.elements('[fabric = "true"]');
  if (existingFabric.length) {
    hwState.cy.remove(existingFabric);
  }
  const { nodes, edges, nodeMeta, edgeMeta } = hwBuildCommunicationFabric();
  hwState.fabricNodeMeta = nodeMeta;
  hwState.fabricEdgeMeta = edgeMeta;
  if (nodes.length > 0) {
    hwState.cy.add(nodes);
  }
  if (edges.length > 0) {
    hwState.cy.add(edges);
  }
  const hasFabric = nodes.length > 0;
  hwSetFabricFocusMode(hasFabric);
  if (hwState.cy.elements().length > 0) {
    const totalNodes = hwState.cy.nodes().length;
    if (totalNodes > 52) {
      const layout = hwState.cy.layout({
        name: "cose",
        animate: false,
        fit: true,
        randomize: false,
        padding: 68,
        idealEdgeLength: 248,
        nodeRepulsion: 720000,
        edgeElasticity: 82,
        gravity: 0.36,
        nodeOverlap: 18,
        componentSpacing: 240,
        numIter: 1800,
        initialTemp: 220,
        coolingFactor: 0.95,
        minTemp: 1,
      });
      if (typeof layout.one === "function") {
        layout.one("layoutstop", () => {
          hwScheduleCanvasRelayout({ padding: 30 });
        });
      }
      layout.run();
    } else {
      hwApplyFabricPresetLayout();
      const layout = hwState.cy.layout({
        name: "cose",
        animate: false,
        fit: true,
        randomize: false,
        padding: 52,
        idealEdgeLength: 206,
        nodeRepulsion: 680000,
        edgeElasticity: 68,
        gravity: 0.34,
        nodeOverlap: 16,
        componentSpacing: 200,
        numIter: 1200,
        initialTemp: 180,
        coolingFactor: 0.96,
        minTemp: 1,
      });
      if (typeof layout.one === "function") {
        layout.one("layoutstop", () => {
          hwScheduleCanvasRelayout({ padding: 30 });
        });
      }
      layout.run();
    }
    hwScheduleCanvasRelayout({ padding: 24 });
  }
  hwRenderLegend();
}

function hwSetFabricFocusMode(enabled) {
  if (!hwState.cy) return;
  const active = !!enabled;
  const moduleNodes = hwState.cy.nodes().filter(
    (node) => String(node.data("fabric") || "") !== "true",
  );
  const moduleEdges = hwState.cy.edges().filter(
    (edge) => String(edge.data("fabric") || "") !== "true",
  );
  const display = active ? "none" : "element";
  moduleNodes.style("display", display);
  moduleEdges.style("display", display);
}

function hwRenderLegend() {
  if (!el.hwLegend) return;
  const items = [
    { label: "Realtime", color: "#0ea5e9", style: "solid" },
    { label: "Zenoh", color: "#0ea5e9", style: "dashed" },
    { label: "MQTT", color: "#f59e0b", style: "dashed" },
    { label: "Modbus TCP", color: "#2563eb", style: "solid" },
    { label: "OPC UA", color: "#7c3aed", style: "solid" },
    { label: "Mesh", color: "#10b981", style: "dashed" },
    { label: "Discovery", color: "#06b6d4", style: "dotted" },
    { label: "Web API", color: "#14b8a6", style: "solid" },
    { label: "EtherCAT", color: "#ca8a04", style: "solid" },
  ];
  el.hwLegend.innerHTML = items.map((item) => (
    `<div class="hw-legend-item">
      <span class="hw-legend-line" style="border-top-color:${escapeAttr(item.color)};border-top-style:${escapeAttr(item.style)}"></span>
      <span>${escapeHtml(item.label)}</span>
    </div>`
  )).join("");
}

function hwRenderFabricNodePanel(meta) {
  const panel = el.hwPropertyPanel;
  if (!panel || !meta) return;
  if (meta.type === "runtime") {
    panel.innerHTML = `<div class="hw-prop-header">
      <h4>${escapeHtml(meta.runtimeId)}</h4>
      <span class="muted" style="font-size:11px">Runtime</span>
    </div>
    <div class="hw-prop-grid">
      <div class="hw-prop-stat"><span>Runtime ID</span><strong>${escapeHtml(meta.runtimeId)}</strong></div>
      <div class="hw-prop-stat"><span>Web Listen</span><strong>${escapeHtml(meta.webListen || "--")}</strong></div>
      <div class="hw-prop-stat"><span>Host Group</span><strong>${escapeHtml(meta.hostGroup || "--")}</strong></div>
      <div class="hw-prop-stat"><span>Cycle (ms)</span><strong>${escapeHtml(String(meta.cycleMs ?? "--"))}</strong></div>
    </div>
    <div class="hw-prop-actions">
      <button type="button" class="btn ghost" id="hwSetActiveRuntimeBtn">Set Active Runtime</button>
      <button type="button" class="btn ghost" id="hwOpenRuntimeSettingsBtn">Open Runtime Settings</button>
      <button type="button" class="btn ghost" id="hwOpenRuntimeCommSettingsBtn">Open Communication Settings</button>
    </div>`;
    const setActive = document.getElementById("hwSetActiveRuntimeBtn");
    if (setActive) {
      setActive.addEventListener("click", () => {
        hwSetActiveRuntimeId(meta.runtimeId);
        hwRenderRuntimeSelector();
        hwApplyWorkspaceRuntimeModels(hwState.hydratedProject || "");
      });
    }
    const openRuntimeSettingsBtn = document.getElementById("hwOpenRuntimeSettingsBtn");
    if (openRuntimeSettingsBtn) {
      openRuntimeSettingsBtn.addEventListener("click", () => {
        hwOpenSettingsForKey("resource.name", "general", { runtimeId: meta.runtimeId });
      });
    }
    const openRuntimeCommSettingsBtn = document.getElementById("hwOpenRuntimeCommSettingsBtn");
    if (openRuntimeCommSettingsBtn) {
      openRuntimeCommSettingsBtn.addEventListener("click", () => {
        hwOpenSettingsForKey("runtime_cloud.links.transports_json", "communication", { runtimeId: meta.runtimeId });
      });
    }
    return;
  }
  const proto = String(meta.proto || "").trim().toLowerCase();
  const settingsDriver = hwSettingsDriverForEndpointProto(proto);
  const settingsKey = hwSettingsKeyForDriver(settingsDriver);
  const runtimeIds = Array.isArray(meta.runtimeIds) ? meta.runtimeIds.map((value) => String(value || "").trim()).filter(Boolean) : [];
  const runtimeScope = runtimeIds.length > 0 ? runtimeIds[0] : hwState.activeRuntimeId;
  panel.innerHTML = `<div class="hw-prop-header">
    <h4>${escapeHtml(meta.label || "Communication Endpoint")}</h4>
    <span class="muted" style="font-size:11px">Endpoint</span>
  </div>
  <div class="hw-prop-grid">
    <div class="hw-prop-stat"><span>Protocol</span><strong>${escapeHtml(proto || "--")}</strong></div>
    <div class="hw-prop-stat"><span>Runtime Scope</span><strong>${escapeHtml(runtimeScope || "--")}</strong></div>
  </div>
  <p class="muted" style="font-size:12px;margin:0 0 10px">Select a link to edit transport details, or jump straight to settings.</p>
  ${
    settingsKey
      ? `<div class="hw-prop-actions">
          <button type="button" class="btn ghost" id="hwOpenEndpointSettingsBtn">Open ${escapeHtml(hwDriverDisplayName(settingsDriver))} Settings</button>
        </div>`
      : ""
  }`;
  const endpointSettingsBtn = document.getElementById("hwOpenEndpointSettingsBtn");
  if (endpointSettingsBtn && settingsKey) {
    endpointSettingsBtn.addEventListener("click", () => {
      hwOpenSettingsForFabricEndpoint(meta);
    });
  }
}

function hwRenderFabricEdgePanel(meta) {
  const panel = el.hwPropertyPanel;
  if (!panel || !meta) return;
  const typeLabel = hwDriverDisplayName(meta.type || "communication");
  let fields = "";

  if (meta.type === "runtime_cloud") {
    fields = `<div class="field">
      <label style="font-size:11px;color:var(--muted-strong)">Transport</label>
      <select data-hw-link-field="transport">
        ${hwRuntimeLinkTransportOptionsHtml(meta.transport)}
      </select>
    </div>`;
  } else if (meta.type === "mqtt") {
    fields = `<div class="field"><label style="font-size:11px;color:var(--muted-strong)">Broker</label><input data-hw-link-field="broker" value="${escapeAttr(meta.broker || "")}"/></div>
      <div class="field"><label style="font-size:11px;color:var(--muted-strong)">Topic In</label><input data-hw-link-field="topicIn" value="${escapeAttr(meta.topicIn || "")}"/></div>
      <div class="field"><label style="font-size:11px;color:var(--muted-strong)">Topic Out</label><input data-hw-link-field="topicOut" value="${escapeAttr(meta.topicOut || "")}"/></div>
      <div class="field"><label style="display:flex;gap:8px;align-items:center;font-size:11px;color:var(--muted-strong)"><input type="checkbox" data-hw-link-field="tls"${meta.tls ? " checked" : ""}/> TLS</label></div>`;
  } else if (meta.type === "modbus") {
    fields = `<div class="field"><label style="font-size:11px;color:var(--muted-strong)">PLC Address</label><input data-hw-link-field="address" value="${escapeAttr(meta.address || "")}"/></div>
      <div class="field"><label style="font-size:11px;color:var(--muted-strong)">Unit ID</label><input type="number" data-hw-link-field="unitId" value="${escapeAttr(String(meta.unitId ?? 1))}"/></div>
      <div class="field"><label style="font-size:11px;color:var(--muted-strong)">Timeout (ms)</label><input type="number" data-hw-link-field="timeoutMs" value="${escapeAttr(String(meta.timeoutMs ?? 500))}"/></div>`;
  } else if (meta.type === "ethercat") {
    fields = `<div class="field"><label style="font-size:11px;color:var(--muted-strong)">Adapter</label><input data-hw-link-field="adapter" value="${escapeAttr(meta.adapter || "")}"/></div>
      <div class="field"><label style="font-size:11px;color:var(--muted-strong)">Timeout (ms)</label><input type="number" data-hw-link-field="timeoutMs" value="${escapeAttr(String(meta.timeoutMs ?? 250))}"/></div>`;
  } else if (meta.type === "opcua") {
    fields = `<div class="field"><label style="font-size:11px;color:var(--muted-strong)">Listen</label><input data-hw-link-field="listen" value="${escapeAttr(meta.listen || "")}"/></div>
      <div class="field"><label style="font-size:11px;color:var(--muted-strong)">Security Mode</label>
      <select data-hw-link-field="securityMode">
        <option value="none"${meta.securityMode === "none" ? " selected" : ""}>none</option>
        <option value="sign"${meta.securityMode === "sign" ? " selected" : ""}>sign</option>
        <option value="sign_and_encrypt"${meta.securityMode === "sign_and_encrypt" ? " selected" : ""}>sign_and_encrypt</option>
      </select></div>`;
  } else if (meta.type === "mesh") {
    fields = `<div class="field"><label style="font-size:11px;color:var(--muted-strong)">Peer URL</label><input data-hw-link-field="peer" value="${escapeAttr(meta.peer || "")}"/></div>
      <div class="field"><label style="display:flex;gap:8px;align-items:center;font-size:11px;color:var(--muted-strong)"><input type="checkbox" data-hw-link-field="tls"${meta.tls ? " checked" : ""}/> TLS</label></div>`;
  } else if (meta.type === "discovery") {
    fields = `<div class="field"><label style="font-size:11px;color:var(--muted-strong)">Service Name</label><input data-hw-link-field="serviceName" value="${escapeAttr(meta.serviceName || "")}"/></div>`;
  } else if (meta.type === "web") {
    fields = `<div class="field"><label style="font-size:11px;color:var(--muted-strong)">Listen</label><input data-hw-link-field="listen" value="${escapeAttr(meta.listen || "")}"/></div>
      <div class="field"><label style="font-size:11px;color:var(--muted-strong)">Auth</label>
      <select data-hw-link-field="auth">
        <option value="local"${meta.auth === "local" ? " selected" : ""}>local</option>
        <option value="token"${meta.auth === "token" ? " selected" : ""}>token</option>
      </select></div>
      <div class="field"><label style="display:flex;gap:8px;align-items:center;font-size:11px;color:var(--muted-strong)"><input type="checkbox" data-hw-link-field="tls"${meta.tls ? " checked" : ""}/> TLS</label></div>`;
  } else {
    fields = '<p class="muted" style="font-size:12px;margin:0">This communication link type is currently read-only.</p>';
  }

  const canDelete = meta.type === "runtime_cloud" || meta.type === "mesh";
  const routeLabel = (meta.source && meta.target) ? `${meta.source} -> ${meta.target}` : "";
  panel.innerHTML = `<div class="hw-prop-header">
    <h4>Communication Link</h4>
    <span class="hw-link-type-pill">${escapeHtml(typeLabel)}</span>
  </div>
  <div class="hw-link-editor">
    <div class="hw-link-meta">
      <div class="row"><span class="muted">Runtime</span><span>${escapeHtml(meta.runtimeId || meta.ownerRuntimeId || "--")}</span></div>
      <div class="row"><span class="muted">Type</span><span>${escapeHtml(meta.type || "--")}</span></div>
      ${routeLabel ? `<div class="row"><span class="muted">Route</span><span>${escapeHtml(routeLabel)}</span></div>` : ""}
    </div>
    ${fields}
    <div class="hw-link-actions">
      <button type="button" class="btn secondary" id="hwLinkSaveBtn">Save</button>
      ${canDelete ? '<button type="button" class="btn ghost" id="hwLinkDeleteBtn" style="color:var(--danger)">Delete Link</button>' : ""}
    </div>
  </div>`;

  const saveBtn = document.getElementById("hwLinkSaveBtn");
  if (saveBtn) {
    saveBtn.addEventListener("click", () => {
      void hwSaveFabricEdge(meta);
    });
  }
  const deleteBtn = document.getElementById("hwLinkDeleteBtn");
  if (deleteBtn) {
    deleteBtn.addEventListener("click", () => {
      void hwDeleteFabricEdge(meta);
    });
  }
}

async function hwFetchWorkspaceIoConfig() {
  try {
    return await apiJson("/api/ide/io/config", {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 3000,
    });
  } catch {
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

async function hwFetchWorkspaceRuntimeToml() {
  try {
    const snapshot = await apiJson(`/api/ide/file?path=${encodeURIComponent("runtime.toml")}`, {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 3000,
    });
    return typeof snapshot?.content === "string" ? snapshot.content : "";
  } catch {
    return "";
  }
}

async function hwFetchWorkspaceRuntimeEntries() {
  if (!(typeof state === "object" && state && state.standaloneMode)) {
    return [];
  }
  try {
    const result = await apiJson("/api/config-ui/runtime/lifecycle", {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 4000,
    });
    const items = Array.isArray(result?.items) ? result.items : [];
    return items.map((item) => ({
      runtimeId: String(item?.runtime_id || "").trim(),
      runtimeRoot: String(item?.runtime_root || "").trim(),
      webListen: String(item?.web_listen || "").trim(),
      hostGroup: String(item?.host_group || "").trim(),
    })).filter((item) => item.runtimeId);
  } catch {
    return [];
  }
}

async function hwFetchRuntimeTomlSnapshotForRuntime(runtimeId) {
  const runtime = String(runtimeId || "").trim();
  if (runtime && typeof state === "object" && state && state.standaloneMode) {
    try {
      const result = await apiJson(`/api/config-ui/runtime/config?runtime_id=${encodeURIComponent(runtime)}`, {
        method: "GET",
        headers: apiHeaders(),
        timeoutMs: 4000,
      });
      return {
        text: typeof result?.text === "string" ? result.text : "",
        revision: typeof result?.revision === "string" ? result.revision : null,
      };
    } catch {
      // Fall through to local single-runtime snapshot.
    }
  }
  try {
    const snapshot = await apiJson(`/api/ide/file?path=${encodeURIComponent("runtime.toml")}`, {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 3000,
    });
    return {
      text: typeof snapshot?.content === "string" ? snapshot.content : "",
      revision: null,
    };
  } catch {
    return { text: "", revision: null };
  }
}

async function hwFetchIoTomlSnapshotForRuntime(runtimeId) {
  const runtime = String(runtimeId || "").trim();
  if (runtime && typeof state === "object" && state && state.standaloneMode) {
    try {
      const result = await apiJson(`/api/config-ui/io/config?runtime_id=${encodeURIComponent(runtime)}`, {
        method: "GET",
        headers: apiHeaders(),
        timeoutMs: 4000,
      });
      return {
        text: typeof result?.text === "string" ? result.text : "",
        revision: typeof result?.revision === "string" ? result.revision : null,
      };
    } catch {
      // Fall through.
    }
  }
  try {
    const fallback = await hwFetchWorkspaceIoConfig();
    if (fallback && typeof fallback === "object" && fallback.ok !== false) {
      const drivers = Array.isArray(fallback.drivers) ? fallback.drivers : [];
      const safeState = Array.isArray(fallback.safe_state) ? fallback.safe_state : [];
      const ioConfig = {
        drivers: drivers.map((entry) => ({
          name: String(entry?.name || "").trim(),
          params: (entry && typeof entry.params === "object" && entry.params) ? entry.params : {},
        })).filter((entry) => entry.name.length > 0),
        safe_state: safeState,
        use_system_io: !!fallback.use_system_io,
      };
      return {
        text: hwRenderIoTomlText(ioConfig),
        revision: null,
      };
    }
  } catch {
    // handled by default below
  }
  return { text: "", revision: null };
}

function hwFormatTomlString(value) {
  return `"${String(value ?? "")
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\n/g, "\\n")}"`;
}

function hwFormatTomlValue(value) {
  if (value == null) return hwFormatTomlString("");
  if (typeof value === "string") return hwFormatTomlString(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return "0";
    return Number.isInteger(value) ? String(value) : String(value);
  }
  if (typeof value === "boolean") return value ? "true" : "false";
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    return `[${value.map((entry) => hwFormatTomlValue(entry)).join(", ")}]`;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value)
      .filter(([key]) => String(key || "").trim().length > 0);
    if (entries.length === 0) return "{}";
    return `{ ${entries
      .map(([key, entryValue]) => `${String(key).trim()} = ${hwFormatTomlValue(entryValue)}`)
      .join(", ")} }`;
  }
  return hwFormatTomlString(String(value));
}

function hwParseIoSafeStateBlocks(text) {
  const lines = String(text || "").replace(/\r\n/g, "\n").split("\n");
  const entries = [];
  let current = null;
  for (const lineRaw of lines) {
    const line = hwTomlStripComment(lineRaw);
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
    current[key] = hwParseTomlValue(valueRaw);
  }
  if (current && current.address) {
    entries.push({
      address: String(current.address),
      value: current.value == null ? "FALSE" : String(current.value),
    });
  }
  return entries;
}

function hwParseIoDriversFromText(text) {
  const driversRaw = hwTomlReadRawAssignment(text, "io", "drivers");
  if (driversRaw && driversRaw.startsWith("[") && driversRaw.endsWith("]")) {
    const inner = driversRaw.slice(1, -1).trim();
    if (!inner) return [];
    return hwTomlSplitTopLevel(inner)
      .map((entry) => hwTomlParseInlineTable(entry))
      .filter((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
      .map((entry) => ({
        name: String(entry.name || "").trim(),
        params: (entry.params && typeof entry.params === "object" && !Array.isArray(entry.params))
          ? entry.params
          : {},
      }))
      .filter((entry) => entry.name.length > 0);
  }

  const sections = hwParseTomlSections(text);
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

function hwParseIoTomlText(text) {
  const sections = hwParseTomlSections(text);
  const io = sections.io || {};
  const drivers = hwParseIoDriversFromText(text);
  const safeState = hwParseIoSafeStateBlocks(text);
  return {
    drivers,
    safe_state: safeState,
    use_system_io: !!io.use_system_io,
  };
}

function hwRenderIoTomlText(ioConfig) {
  const config = (ioConfig && typeof ioConfig === "object") ? ioConfig : {};
  const drivers = Array.isArray(config.drivers) ? config.drivers : [];
  const safeState = Array.isArray(config.safe_state) ? config.safe_state : [];

  const lines = ["[io]"];
  if (config.use_system_io) {
    lines.push("use_system_io = true");
  }
  if (drivers.length === 1 && !Array.isArray(config.drivers_json_legacy)) {
    const only = drivers[0];
    lines.push(`driver = ${hwFormatTomlString(only.name)}`);
    const params = (only.params && typeof only.params === "object") ? only.params : {};
    lines.push("");
    lines.push("[io.params]");
    for (const [key, value] of Object.entries(params)) {
      lines.push(`${key} = ${hwFormatTomlValue(value)}`);
    }
  } else {
    lines.push("drivers = [");
    for (const driver of drivers) {
      const name = String(driver?.name || "").trim();
      if (!name) continue;
      const params = (driver && typeof driver.params === "object" && !Array.isArray(driver.params))
        ? driver.params
        : {};
      lines.push(`  { name = ${hwFormatTomlString(name)}, params = ${hwFormatTomlValue(params)} },`);
    }
    lines.push("]");
  }

  for (const entry of safeState) {
    const address = String(entry?.address || "").trim();
    if (!address) continue;
    lines.push("");
    lines.push("[[io.safe_state]]");
    lines.push(`address = ${hwFormatTomlString(address)}`);
    lines.push(`value = ${hwFormatTomlString(String(entry?.value ?? "FALSE"))}`);
  }

  return `${lines.join("\n")}\n`;
}

function hwRuntimeEntryFromToml(text) {
  const sections = hwParseTomlSections(text);
  const resource = sections.resource || {};
  const runtimeId = String(resource.name || "").trim();
  return runtimeId || "runtime-1";
}

function hwBroadcastActiveRuntimeSelection(runtimeId, source) {
  const value = String(runtimeId || "").trim();
  if (!value) return;
  document.dispatchEvent(new CustomEvent(HW_RUNTIME_SELECTION_EVENT, {
    detail: {
      runtimeId: value,
      source: source || "hardware",
    },
  }));
}

function hwSetActiveRuntimeId(runtimeId, options) {
  const opts = options || {};
  const value = String(runtimeId || "").trim();
  if (!value) return false;
  const previous = String(hwState.activeRuntimeId || "").trim();
  hwState.activeRuntimeId = value;
  try {
    localStorage.setItem("trust.ide.hw.activeRuntimeId", value);
  } catch {
    // ignore localStorage failures
  }
  if (previous !== value && opts.broadcast !== false) {
    hwBroadcastActiveRuntimeSelection(value, opts.source || "hardware");
  }
  return previous !== value;
}

function hwResolveInitialActiveRuntimeId(runtimeModels) {
  const models = Array.isArray(runtimeModels) ? runtimeModels : [];
  if (models.length === 0) return "";
  const stored = (() => {
    try {
      return String(localStorage.getItem("trust.ide.hw.activeRuntimeId") || "").trim();
    } catch {
      return "";
    }
  })();
  if (stored && models.some((item) => item.runtimeId === stored)) {
    return stored;
  }
  if (hwState.activeRuntimeId && models.some((item) => item.runtimeId === hwState.activeRuntimeId)) {
    return hwState.activeRuntimeId;
  }
  return models[0].runtimeId;
}

function hwRenderRuntimeSelector() {
  const select = el.hwRuntimeSelect;
  if (!select) return;
  const models = Array.isArray(hwState.workspaceRuntimes) ? hwState.workspaceRuntimes : [];
  if (models.length === 0) {
    select.innerHTML = '<option value="">No runtime</option>';
    select.disabled = true;
    return;
  }
  let html = "";
  for (const model of models) {
    const runtimeId = String(model.runtimeId || "").trim();
    if (!runtimeId) continue;
    html += `<option value="${escapeAttr(runtimeId)}">${escapeHtml(runtimeId)}</option>`;
  }
  select.innerHTML = html;
  select.disabled = false;
  select.value = hwState.activeRuntimeId || models[0].runtimeId;
}

function hwRenderTransportPills() {
  const container = el.hwTransportPills;
  if (!container) return;
  const models = Array.isArray(hwState.workspaceRuntimes) ? hwState.workspaceRuntimes : [];
  const counts = new Map();
  for (const model of models) {
    const rules = Array.isArray(model.runtimeCloudLinks) ? model.runtimeCloudLinks : [];
    for (const rule of rules) {
      const key = hwNormalizeRuntimeLinkTransport(rule.transport) || "realtime";
      counts.set(key, (counts.get(key) || 0) + 1);
    }
  }
  if (counts.size === 0) {
    container.innerHTML = '<span class="hw-transport-pill">No runtime links configured yet.</span>';
    return;
  }
  container.innerHTML = HW_RUNTIME_LINK_TRANSPORTS
    .filter((entry) => (counts.get(entry.id) || 0) > 0)
    .map((entry) => {
      const total = counts.get(entry.id) || 0;
      const color = hwProtocolIconStroke(entry.id);
      return `<span class="hw-transport-pill">
        <span class="hw-transport-pill-dot" style="background:${escapeAttr(color)}"></span>
        <span>${escapeHtml(entry.label)}</span>
        <span class="hw-transport-pill-count">${escapeHtml(String(total))}</span>
      </span>`;
    }).join("");
}

async function hwBuildWorkspaceRuntimeModels(projectPath) {
  const items = await hwFetchWorkspaceRuntimeEntries();
  const models = [];
  const standaloneMode = typeof state === "object" && state && state.standaloneMode;

  if (items.length === 0) {
    if (standaloneMode) {
      return models;
    }
    const [runtimeSnapshot, ioSnapshot] = await Promise.all([
      hwFetchRuntimeTomlSnapshotForRuntime(""),
      hwFetchIoTomlSnapshotForRuntime(""),
    ]);
    const runtimeId = hwRuntimeEntryFromToml(runtimeSnapshot.text);
    models.push({
      runtimeId,
      runtimeRoot: projectPath || "",
      webListen: "",
      hostGroup: "",
      runtimeTomlText: runtimeSnapshot.text || "",
      runtimeRevision: runtimeSnapshot.revision || null,
      runtimeSections: hwParseTomlSections(runtimeSnapshot.text || ""),
      ioTomlText: ioSnapshot.text || "",
      ioRevision: ioSnapshot.revision || null,
      ioConfig: hwParseIoTomlText(ioSnapshot.text || ""),
      runtimeCloudLinks: hwParseCloudLinkTransportSection(runtimeSnapshot.text || "").rules,
    });
    return models;
  }

  for (const item of items) {
    const runtimeId = String(item.runtimeId || "").trim();
    if (!runtimeId) continue;
    const [runtimeSnapshot, ioSnapshot] = await Promise.all([
      hwFetchRuntimeTomlSnapshotForRuntime(runtimeId),
      hwFetchIoTomlSnapshotForRuntime(runtimeId),
    ]);
    const runtimeTomlText = String(runtimeSnapshot?.text || "");
    const ioTomlText = String(ioSnapshot?.text || "");
    models.push({
      runtimeId,
      runtimeRoot: String(item.runtimeRoot || ""),
      webListen: String(item.webListen || ""),
      hostGroup: String(item.hostGroup || ""),
      runtimeTomlText,
      runtimeRevision: runtimeSnapshot?.revision || null,
      runtimeSections: hwParseTomlSections(runtimeTomlText),
      ioTomlText,
      ioRevision: ioSnapshot?.revision || null,
      ioConfig: hwParseIoTomlText(ioTomlText),
      runtimeCloudLinks: hwParseCloudLinkTransportSection(runtimeTomlText).rules,
    });
  }
  return models;
}

function hwApplyWorkspaceRuntimeModels(projectPath) {
  const models = Array.isArray(hwState.workspaceRuntimes) ? hwState.workspaceRuntimes : [];
  if (models.length === 0) {
    if (hwState.modules.length === 0) hwUpdateEmptyState();
    hwRenderTransportPills();
    return;
  }
  const active = models.find((item) => item.runtimeId === hwState.activeRuntimeId) || models[0];
  hwState.activeRuntimeId = active.runtimeId;
  hwRenderRuntimeSelector();
  hwApplyIoConfigToCanvas(active.ioConfig, projectPath, active.runtimeTomlText);
  hwApplyCommunicationFabric();
  hwRenderTransportPills();
}

async function hwHydrateFromProjectConfig(force) {
  if (hwState.hydrating) return;
  const projectPath = typeof state !== "undefined" ? (state.activeProject || "") : "";
  if (!force && hwState.hydratedProject === projectPath && hwState.workspaceRuntimes.length > 0) {
    return;
  }
  hwState.hydrating = true;
  try {
    const models = await hwBuildWorkspaceRuntimeModels(projectPath);
    hwState.workspaceRuntimes = models;
    hwSetActiveRuntimeId(hwResolveInitialActiveRuntimeId(models));
    hwApplyWorkspaceRuntimeModels(projectPath);
    hwState.hydratedProject = projectPath || "";
  } finally {
    hwState.hydrating = false;
  }
}

function hwEscapeRegex(text) {
  return String(text || "").replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function hwTomlUpsert(text, section, key, formattedValue) {
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

  const keyRegex = new RegExp(`^\\s*${hwEscapeRegex(key)}\\s*=\\s*(.*)$`);
  for (let i = sectionStart + 1; i < sectionEnd; i += 1) {
    const match = lines[i].match(keyRegex);
    if (!match) continue;
    let valueEnd = i;
    let rawValue = hwTomlStripComment(match[1]);
    while (hwTomlNeedsContinuation(rawValue) && valueEnd + 1 < sectionEnd) {
      valueEnd += 1;
      const continuation = hwTomlStripComment(lines[valueEnd]);
      if (!continuation) continue;
      rawValue = `${rawValue}\n${continuation}`;
    }
    lines.splice(i, valueEnd - i + 1, assignment);
    return `${lines.join(newline)}${newline}`;
  }

  lines.splice(sectionEnd, 0, assignment);
  return `${lines.join(newline)}${newline}`;
}

function hwFormatCloudLinkRulesToml(rules) {
  const entries = Array.isArray(rules) ? rules : [];
  if (entries.length === 0) return "[]";
  return `[${entries.map((entry) => (
    `{ source = ${hwFormatTomlString(String(entry.source || "").trim())}, target = ${hwFormatTomlString(String(entry.target || "").trim())}, transport = ${hwFormatTomlString(String(entry.transport || "realtime").trim())} }`
  )).join(", ")}]`;
}

async function hwWriteRuntimeTomlText(runtimeId, text, expectedRevision) {
  const runtime = String(runtimeId || "").trim();
  if (runtime && typeof state === "object" && state && state.standaloneMode) {
    const payload = { runtime_id: runtime, text: String(text || "") };
    if (expectedRevision) payload.expected_revision = expectedRevision;
    const result = await apiJson("/api/config-ui/runtime/config", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify(payload),
      timeoutMs: 6000,
    });
    return { revision: result?.revision ? String(result.revision) : null };
  }

  await apiJson("/api/ide/file", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify({
      path: "runtime.toml",
      content: String(text || ""),
    }),
    timeoutMs: 6000,
  });
  return { revision: null };
}

async function hwWriteIoTomlText(runtimeId, text, expectedRevision) {
  const runtime = String(runtimeId || "").trim();
  if (runtime && typeof state === "object" && state && state.standaloneMode) {
    const payload = { runtime_id: runtime, text: String(text || "") };
    if (expectedRevision) payload.expected_revision = expectedRevision;
    const result = await apiJson("/api/config-ui/io/config", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify(payload),
      timeoutMs: 6000,
    });
    return { revision: result?.revision ? String(result.revision) : null };
  }

  await apiJson("/api/ide/file", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify({
      path: "io.toml",
      content: String(text || ""),
    }),
    timeoutMs: 6000,
  });
  return { revision: null };
}

function hwInputValue(selector) {
  const element = document.querySelector(selector);
  if (!element) return "";
  if (element.type === "checkbox") return !!element.checked;
  return String(element.value || "").trim();
}

async function hwSaveFabricEdge(meta) {
  const kind = String(meta?.type || "").trim();
  if (!kind) return;
  const runtimeId = String(meta.runtimeId || meta.ownerRuntimeId || "").trim();
  const model = hwModelByRuntimeId(runtimeId);
  if (!model) {
    if (typeof showIdeToast === "function") {
      showIdeToast("Runtime model not found for this link.", "error");
    }
    return;
  }

  try {
    if (kind === "runtime_cloud") {
      const transport = hwNormalizeRuntimeLinkTransport(hwInputValue('[data-hw-link-field="transport"]')) || "";
      if (!transport) {
        throw new Error(`Transport must be one of: ${HW_RUNTIME_LINK_TRANSPORTS.map((entry) => entry.id).join(", ")}`);
      }
      const rules = Array.isArray(model.runtimeCloudLinks) ? [...model.runtimeCloudLinks] : [];
      if (meta.ruleIndex < 0 || meta.ruleIndex >= rules.length) {
        throw new Error("Link index out of date, reload and retry");
      }
      rules[meta.ruleIndex] = {
        ...rules[meta.ruleIndex],
        transport,
      };
      const nextText = hwTomlUpsert(
        model.runtimeTomlText,
        "runtime.cloud.links",
        "transports",
        hwFormatCloudLinkRulesToml(rules),
      );
      const saveResult = await hwWriteRuntimeTomlText(runtimeId, nextText, model.runtimeRevision);
      model.runtimeTomlText = nextText;
      model.runtimeRevision = saveResult.revision;
      model.runtimeCloudLinks = rules;
    } else if (kind === "mqtt" || kind === "modbus" || kind === "ethercat") {
      const drivers = Array.isArray(model.ioConfig?.drivers) ? model.ioConfig.drivers : [];
      const driver = drivers[meta.driverIndex];
      if (!driver || typeof driver !== "object") {
        throw new Error("Driver index out of date, reload and retry");
      }
      const params = (driver.params && typeof driver.params === "object" && !Array.isArray(driver.params))
        ? { ...driver.params }
        : {};
      if (kind === "mqtt") {
        params.broker = String(hwInputValue('[data-hw-link-field="broker"]') || params.broker || "").trim();
        params.topic_in = String(hwInputValue('[data-hw-link-field="topicIn"]') || params.topic_in || "").trim();
        params.topic_out = String(hwInputValue('[data-hw-link-field="topicOut"]') || params.topic_out || "").trim();
        params.tls = !!hwInputValue('[data-hw-link-field="tls"]');
      } else if (kind === "modbus") {
        params.address = String(hwInputValue('[data-hw-link-field="address"]') || params.address || "").trim();
        params.unit_id = Number(hwInputValue('[data-hw-link-field="unitId"]') || params.unit_id || 1) || 1;
        params.timeout_ms = Number(hwInputValue('[data-hw-link-field="timeoutMs"]') || params.timeout_ms || 500) || 500;
      } else if (kind === "ethercat") {
        params.adapter = String(hwInputValue('[data-hw-link-field="adapter"]') || params.adapter || "").trim();
        params.timeout_ms = Number(hwInputValue('[data-hw-link-field="timeoutMs"]') || params.timeout_ms || 250) || 250;
      }
      model.ioConfig.drivers[meta.driverIndex] = {
        ...driver,
        params,
      };
      const nextIoToml = hwRenderIoTomlText(model.ioConfig);
      const saveResult = await hwWriteIoTomlText(runtimeId, nextIoToml, model.ioRevision);
      model.ioTomlText = nextIoToml;
      model.ioRevision = saveResult.revision;
    } else if (kind === "opcua") {
      const listen = String(hwInputValue('[data-hw-link-field="listen"]') || meta.listen || "").trim();
      const securityMode = String(hwInputValue('[data-hw-link-field="securityMode"]') || meta.securityMode || "none").trim();
      let nextText = model.runtimeTomlText;
      nextText = hwTomlUpsert(nextText, "runtime.opcua", "listen", hwFormatTomlString(listen));
      nextText = hwTomlUpsert(nextText, "runtime.opcua", "security_mode", hwFormatTomlString(securityMode));
      const saveResult = await hwWriteRuntimeTomlText(runtimeId, nextText, model.runtimeRevision);
      model.runtimeTomlText = nextText;
      model.runtimeRevision = saveResult.revision;
    } else if (kind === "mesh") {
      const peer = String(hwInputValue('[data-hw-link-field="peer"]') || meta.peer || "").trim();
      if (!peer) throw new Error("Peer URL is required");
      const tls = !!hwInputValue('[data-hw-link-field="tls"]');
      const meshSection = model.runtimeSections["runtime.mesh"] || {};
      const connect = Array.isArray(meshSection.connect) ? [...meshSection.connect] : [];
      if (meta.connectIndex < 0 || meta.connectIndex >= connect.length) {
        throw new Error("Mesh link index out of date, reload and retry");
      }
      connect[meta.connectIndex] = peer;
      let nextText = model.runtimeTomlText;
      nextText = hwTomlUpsert(nextText, "runtime.mesh", "connect", hwFormatTomlValue(connect));
      nextText = hwTomlUpsert(nextText, "runtime.mesh", "tls", hwFormatTomlValue(tls));
      const saveResult = await hwWriteRuntimeTomlText(runtimeId, nextText, model.runtimeRevision);
      model.runtimeTomlText = nextText;
      model.runtimeRevision = saveResult.revision;
    } else if (kind === "discovery") {
      const serviceName = String(hwInputValue('[data-hw-link-field="serviceName"]') || meta.serviceName || "").trim();
      let nextText = model.runtimeTomlText;
      nextText = hwTomlUpsert(nextText, "runtime.discovery", "service_name", hwFormatTomlString(serviceName));
      const saveResult = await hwWriteRuntimeTomlText(runtimeId, nextText, model.runtimeRevision);
      model.runtimeTomlText = nextText;
      model.runtimeRevision = saveResult.revision;
    } else if (kind === "web") {
      const listen = String(hwInputValue('[data-hw-link-field="listen"]') || meta.listen || "").trim();
      const auth = String(hwInputValue('[data-hw-link-field="auth"]') || meta.auth || "local").trim();
      const tls = !!hwInputValue('[data-hw-link-field="tls"]');
      let nextText = model.runtimeTomlText;
      nextText = hwTomlUpsert(nextText, "runtime.web", "listen", hwFormatTomlString(listen));
      nextText = hwTomlUpsert(nextText, "runtime.web", "auth", hwFormatTomlString(auth));
      nextText = hwTomlUpsert(nextText, "runtime.web", "tls", hwFormatTomlValue(tls));
      const saveResult = await hwWriteRuntimeTomlText(runtimeId, nextText, model.runtimeRevision);
      model.runtimeTomlText = nextText;
      model.runtimeRevision = saveResult.revision;
    }

    if (typeof showIdeToast === "function") {
      showIdeToast("Communication link updated.", "success");
    }
    await hwHydrateFromProjectConfig(true);
  } catch (err) {
    if (typeof showIdeToast === "function") {
      showIdeToast(`Save failed: ${err.message || err}`, "error");
    }
  }
}

async function hwDeleteFabricEdge(meta) {
  const kind = String(meta?.type || "").trim();
  const runtimeId = String(meta.runtimeId || meta.ownerRuntimeId || "").trim();
  const model = hwModelByRuntimeId(runtimeId);
  if (!model) return;
  try {
    if (kind === "runtime_cloud") {
      const rules = Array.isArray(model.runtimeCloudLinks) ? [...model.runtimeCloudLinks] : [];
      if (meta.ruleIndex < 0 || meta.ruleIndex >= rules.length) {
        throw new Error("Link index out of date, reload and retry");
      }
      rules.splice(meta.ruleIndex, 1);
      const nextText = hwTomlUpsert(
        model.runtimeTomlText,
        "runtime.cloud.links",
        "transports",
        hwFormatCloudLinkRulesToml(rules),
      );
      const saveResult = await hwWriteRuntimeTomlText(runtimeId, nextText, model.runtimeRevision);
      model.runtimeTomlText = nextText;
      model.runtimeRevision = saveResult.revision;
      model.runtimeCloudLinks = rules;
    } else if (kind === "mesh") {
      const meshSection = model.runtimeSections["runtime.mesh"] || {};
      const connect = Array.isArray(meshSection.connect) ? [...meshSection.connect] : [];
      if (meta.connectIndex < 0 || meta.connectIndex >= connect.length) {
        throw new Error("Mesh link index out of date, reload and retry");
      }
      connect.splice(meta.connectIndex, 1);
      const nextText = hwTomlUpsert(
        model.runtimeTomlText,
        "runtime.mesh",
        "connect",
        hwFormatTomlValue(connect),
      );
      const saveResult = await hwWriteRuntimeTomlText(runtimeId, nextText, model.runtimeRevision);
      model.runtimeTomlText = nextText;
      model.runtimeRevision = saveResult.revision;
    } else {
      return;
    }
    if (typeof showIdeToast === "function") {
      showIdeToast("Communication link removed.", "success");
    }
    await hwHydrateFromProjectConfig(true);
  } catch (err) {
    if (typeof showIdeToast === "function") {
      showIdeToast(`Delete failed: ${err.message || err}`, "error");
    }
  }
}

function hwMarkAddLinkButtonActive(active) {
  hwSetLinkFlowHint(active, "Select source runtime, then target runtime.");
}

function hwSetLinkFlowHint(active, message) {
  const hint = document.getElementById("hwLinkFlowHint");
  const text = document.getElementById("hwLinkFlowText");
  if (!hint || !text) return;
  hint.hidden = !active;
  if (active) {
    text.textContent = String(message || "Select source runtime, then target runtime.");
  } else {
    text.textContent = "";
  }
}

async function hwPromptRuntimeLinkTransport(defaultTransport = "realtime") {
  const fallback = hwNormalizeRuntimeLinkTransport(defaultTransport) || "realtime";
  const modal = document.getElementById("hwTransportModal");
  const optionsContainer = document.getElementById("hwTransportOptions");
  const cancelBtn = document.getElementById("hwTransportModalCancel");
  if (modal && optionsContainer && cancelBtn) {
    optionsContainer.innerHTML = HW_RUNTIME_LINK_TRANSPORTS.map((entry) => {
      const id = String(entry.id || "").trim().toLowerCase();
      const label = entry.label || id;
      const note = HW_RUNTIME_LINK_TRANSPORT_NOTES[id] || "Runtime communication link transport.";
      const isDefault = id === fallback;
      const icon = hwProtocolIcon(id);
      const iconStroke = hwProtocolIconStroke(id);
      return `<button type="button" class="hw-transport-option${isDefault ? " is-default" : ""}" data-hw-transport-option="${escapeAttr(id)}">
        <span class="hw-transport-option-title">
          <span class="hw-transport-option-title-main">
            <span class="hw-transport-option-icon">${hwIconSvgMarkup(icon, {
              stroke: iconStroke,
              size: 17,
              strokeWidth: 1.35,
              title: label,
              chip: true,
              chipFill: "rgba(255,255,255,0.05)",
            })}</span>
            <span>${escapeHtml(label)}</span>
          </span>
          <span class="hw-transport-option-id">${escapeHtml(id)}</span>
        </span>
        <span class="hw-transport-option-note">${escapeHtml(note)}</span>
      </button>`;
    }).join("");

    const optionButtons = Array.from(optionsContainer.querySelectorAll("[data-hw-transport-option]"));
    const selectedButton = optionsContainer.querySelector(`[data-hw-transport-option="${fallback}"]`);
    const preferredButton = selectedButton || optionButtons[0];
    const modalNote = document.getElementById("hwTransportModalNote");
    if (modalNote) {
      modalNote.textContent = "Select the communication type for this runtime-to-runtime link.";
    }

    const value = await new Promise((resolve) => {
      let settled = false;

      const cleanup = () => {
        modal.classList.remove("open");
        modal.setAttribute("aria-hidden", "true");
        cancelBtn.removeEventListener("click", onCancel);
        modal.removeEventListener("click", onBackdrop);
        document.removeEventListener("keydown", onKeyDown);
        for (const btn of optionButtons) {
          btn.removeEventListener("click", onOptionClick);
        }
      };

      const finish = (result) => {
        if (settled) return;
        settled = true;
        cleanup();
        resolve(result);
      };

      const onCancel = () => finish(null);
      const onBackdrop = (event) => {
        if (event.target === modal) {
          finish(null);
        }
      };
      const onKeyDown = (event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          finish(null);
        }
      };
      const onOptionClick = (event) => {
        const choice = String(event.currentTarget?.dataset?.hwTransportOption || "").trim();
        finish(choice || null);
      };

      cancelBtn.addEventListener("click", onCancel);
      modal.addEventListener("click", onBackdrop);
      document.addEventListener("keydown", onKeyDown);
      for (const btn of optionButtons) {
        btn.addEventListener("click", onOptionClick);
      }

      modal.classList.add("open");
      modal.setAttribute("aria-hidden", "false");
      if (preferredButton) {
        preferredButton.focus();
      } else {
        cancelBtn.focus();
      }
    });

    if (value == null) {
      return null;
    }
    const transport = hwNormalizeRuntimeLinkTransport(value);
    if (!transport) {
      const optionsText = HW_RUNTIME_LINK_TRANSPORTS.map((entry) => entry.id).join(", ");
      throw new Error(`Unsupported transport. Allowed: ${optionsText}`);
    }
    return transport;
  }

  const optionsText = HW_RUNTIME_LINK_TRANSPORTS.map((entry) => entry.id).join(", ");
  let value = null;
  if (typeof idePrompt === "function") {
    value = await idePrompt(`Link transport (${optionsText})`, fallback);
  } else if (typeof window !== "undefined" && typeof window.prompt === "function") {
    value = window.prompt(`Link transport (${optionsText})`, fallback);
  } else {
    value = fallback;
  }
  if (value == null) {
    return null;
  }
  const transport = hwNormalizeRuntimeLinkTransport(value);
  if (!transport) {
    throw new Error(`Unsupported transport. Allowed: ${optionsText}`);
  }
  return transport;
}

async function hwCreateRuntimeCloudLink(sourceRuntimeId, targetRuntimeId, preferredTransport) {
  const source = String(sourceRuntimeId || "").trim();
  const target = String(targetRuntimeId || "").trim();
  if (!source || !target) return;
  if (source === target) {
    if (typeof showIdeToast === "function") {
      showIdeToast("Select a different target runtime.", "warn");
    }
    return;
  }
  const owner = hwModelByRuntimeId(source);
  if (!owner) return;
  try {
    let transport = hwNormalizeRuntimeLinkTransport(preferredTransport);
    if (!transport) {
      transport = await hwPromptRuntimeLinkTransport(hwState.lastRuntimeLinkTransport || "realtime");
    }
    if (!transport) {
      if (typeof showIdeToast === "function") {
        showIdeToast("Link creation cancelled.", "warn");
      }
      return;
    }
    const rules = Array.isArray(owner.runtimeCloudLinks) ? [...owner.runtimeCloudLinks] : [];
    const exists = rules.some(
      (rule) => String(rule.source || "") === source
        && String(rule.target || "") === target
        && String(rule.transport || "").trim().toLowerCase() === transport,
    );
    if (exists) {
      if (typeof showIdeToast === "function") {
        showIdeToast("Link already exists for this transport.", "warn");
      }
      return;
    }
    rules.push({ source, target, transport });
    const nextText = hwTomlUpsert(
      owner.runtimeTomlText,
      "runtime.cloud.links",
      "transports",
      hwFormatCloudLinkRulesToml(rules),
    );
    const saveResult = await hwWriteRuntimeTomlText(owner.runtimeId, nextText, owner.runtimeRevision);
    owner.runtimeTomlText = nextText;
    owner.runtimeRevision = saveResult.revision;
    owner.runtimeCloudLinks = rules;
    hwState.lastRuntimeLinkTransport = transport;
    hwRenderTransportPills();
    if (typeof showIdeToast === "function") {
      showIdeToast(`Runtime link created (${transport}).`, "success");
    }
    await hwHydrateFromProjectConfig(true);
  } catch (err) {
    if (typeof showIdeToast === "function") {
      showIdeToast(`Link creation failed: ${err.message || err}`, "error");
    }
  }
}

function hwNormalizeSafeStateValue(value) {
  if (value === true) return "TRUE";
  if (value === false) return "FALSE";
  const text = String(value || "").trim().toLowerCase();
  if (text === "true") return "TRUE";
  if (text === "false") return "FALSE";
  const typed = String(value || "").trim().match(/^[A-Za-z_][A-Za-z0-9_]*\(([-+]?\d+)\)$/);
  if (typed) return typed[1];
  return String(value || "FALSE");
}

function hwBuildPersistableModules() {
  return hwState.modules.filter((mod) => {
    if (!mod || !mod.driver) return false;
    if (mod.paletteType === "cpu") return false;
    if (String(mod.paletteType || "").startsWith("runtime-")) return false;
    return true;
  });
}

function hwBuildIoConfigPayload() {
  const modules = hwBuildPersistableModules();
  const grouped = new Map();

  for (const mod of modules) {
    const name = String(mod.driver || "").trim().toLowerCase();
    if (!name) continue;
    if (!grouped.has(name)) {
      grouped.set(name, {
        name,
        params: (mod.params && typeof mod.params === "object") ? { ...mod.params } : {},
      });
    }
  }

  let drivers = Array.from(grouped.values());
  if (drivers.length === 0) {
    if (Array.isArray(hwState.lastIoConfig?.drivers) && hwState.lastIoConfig.drivers.length > 0) {
      drivers = hwState.lastIoConfig.drivers.map((driver) => ({
        name: String(driver.name || "").trim().toLowerCase() || "simulated",
        params: (driver.params && typeof driver.params === "object") ? { ...driver.params } : {},
      }));
    } else {
      drivers = [{
        name: String(hwState.lastIoConfig?.driver || "simulated").trim().toLowerCase(),
        params: (hwState.lastIoConfig?.params && typeof hwState.lastIoConfig.params === "object")
          ? { ...hwState.lastIoConfig.params }
          : {},
      }];
    }
  }

  const safeStateByAddress = new Map();
  const priorSafeState = Array.isArray(hwState.lastIoConfig?.safe_state)
    ? hwState.lastIoConfig.safe_state
    : [];
  for (const entry of priorSafeState) {
    const address = String(entry?.address || "").trim();
    if (!address) continue;
    safeStateByAddress.set(address, hwNormalizeSafeStateValue(entry?.value));
  }
  for (const mod of modules) {
    if (mod.direction !== "output") continue;
    for (const address of (mod.addresses || [])) {
      const key = String(address || "").trim();
      if (!key) continue;
      if (!safeStateByAddress.has(key)) {
        safeStateByAddress.set(key, "FALSE");
      }
    }
  }

  const safe_state = Array.from(safeStateByAddress.entries())
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([address, value]) => ({ address, value }));
  const useSystemIo = !!hwState.lastIoConfig?.use_system_io;

  return {
    drivers,
    safe_state,
    use_system_io: useSystemIo,
  };
}

async function hwSaveWorkspaceIoConfig(payload) {
  try {
    return await apiJson("/api/ide/io/config", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify(payload),
      timeoutMs: 4000,
    });
  } catch {
    return await apiJson("/api/io/config", {
      method: "POST",
      body: JSON.stringify(payload),
      timeoutMs: 4000,
    });
  }
}

function hwQueuePersistIoConfig() {
  if (hwState.hydrating) return;
  if (hwState.persistInFlight) {
    hwState.persistQueued = true;
    return;
  }
  if (hwState.persistTimer) {
    clearTimeout(hwState.persistTimer);
  }
  hwState.persistTimer = setTimeout(() => {
    hwState.persistTimer = null;
    void hwPersistIoConfigNow();
  }, 220);
}

async function hwPersistIoConfigNow() {
  if (hwState.hydrating) return;
  if (hwState.persistInFlight) {
    hwState.persistQueued = true;
    return;
  }

  const payload = hwBuildIoConfigPayload();
  const fingerprint = JSON.stringify(payload);
  if (fingerprint === hwState.lastPersistFingerprint) {
    return;
  }

  hwState.persistInFlight = true;
  hwState.persistQueued = false;
  try {
    await hwSaveWorkspaceIoConfig(payload);
    hwState.lastPersistFingerprint = fingerprint;
    hwState.lastIoConfig = {
      ...(hwState.lastIoConfig || {}),
      driver: payload.drivers[0]?.name || "simulated",
      params: payload.drivers[0]?.params || {},
      drivers: payload.drivers,
      safe_state: payload.safe_state,
      use_system_io: !!payload.use_system_io,
    };
    document.dispatchEvent(new CustomEvent("ide-io-config-updated", {
      detail: {
        source: "hardware",
      },
    }));
  } catch (err) {
    if (typeof showIdeToast === "function") {
      showIdeToast(`Failed to save io.toml: ${err.message || err}`, "error");
    }
  } finally {
    hwState.persistInFlight = false;
    if (hwState.persistQueued) {
      hwState.persistQueued = false;
      hwQueuePersistIoConfig();
    }
  }
}

// ── Property Panel ─────────────────────────────────────

function hwShowPropertyPanel() {
  if (hwState.inspectorCollapsed) return;
  if (el.hwPropertyPanel) el.hwPropertyPanel.hidden = false;
}

function hwRenderPropertyPanel(mod) {
  const panel = el.hwPropertyPanel;
  if (!panel) return;
  panel.hidden = hwState.inspectorCollapsed;
  if (!mod) {
    const selectable = hwState.modules.filter((entry) => entry.paletteType !== "cpu");
    let html = `<div class="hw-prop-header">
      <h4>Hardware Inspector</h4>
      <span class="muted" style="font-size:11px">Select module</span>
    </div>`;
    html += `<p class="muted" style="margin:0 0 10px;font-size:12px">Select a node on the canvas to edit properties and communication parameters.</p>`;
    if (selectable.length === 0) {
      html += `<p class="muted" style="font-size:12px">No hardware modules loaded yet.</p>`;
    } else {
      html += `<div class="hw-module-list">`;
      for (const entry of selectable) {
        html += `<button type="button" class="hw-module-select" data-hw-select="${escapeAttr(entry.id)}">
          <div>${escapeHtml(entry.label)}</div>
          <div class="muted">${escapeHtml(entry.driver || "generic")} • ${entry.addresses.length} points</div>
        </button>`;
      }
      html += `</div>`;
    }
    html += `<div class="hw-inspector-note">Tip: double-click a module node to pin its property editor.</div>`;
    panel.innerHTML = html;
    panel.querySelectorAll("[data-hw-select]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const moduleId = btn.dataset.hwSelect;
        const picked = hwState.modules.find((entry) => entry.id === moduleId);
        if (!picked) return;
        hwState.selectedModuleId = picked.id;
        if (hwState.cy) {
          hwState.cy.elements("node").unselect();
          const node = hwState.cy.getElementById(picked.id);
          if (node.length) node.select();
        }
        hwRenderPropertyPanel(picked);
      });
    });
    return;
  }

  let html = `<div class="hw-prop-header">
    <h4>${escapeHtml(mod.label)}</h4>
    <span class="muted" style="font-size:11px">${escapeHtml(mod.paletteType)}</span>
  </div>`;

  html += `<div class="hw-prop-grid">
    <div class="hw-prop-stat"><span>Driver</span><strong>${escapeHtml(mod.driver || "generic")}</strong></div>
    <div class="hw-prop-stat"><span>Node Type</span><strong>${escapeHtml(mod.nodeType || "--")}</strong></div>
    <div class="hw-prop-stat"><span>Channels</span><strong>${escapeHtml(String(mod.channels || 0))}</strong></div>
    <div class="hw-prop-stat"><span>Addresses</span><strong>${escapeHtml(String(mod.addresses.length))}</strong></div>
  </div>`;

  // Module name
  html += `<div class="field">
    <label style="font-size:11px;color:var(--muted-strong)">Name</label>
    <input type="text" value="${escapeAttr(mod.label)}" data-hw-prop="label" style="font-size:12px"/>
  </div>`;

  // Addresses summary
  if (mod.addresses.length > 0) {
    html += `<div class="field">
      <label style="font-size:11px;color:var(--muted-strong)">Addresses</label>
      <div class="muted" style="font-size:11px;font-family:var(--ide-font-mono)">${escapeHtml(hwCompactAddressRange(mod.addresses[0], mod.addresses[mod.addresses.length - 1]))}</div>
    </div>`;
  }

  // Driver params
  if (mod.driver) {
    html += `<div class="field">
      <label style="font-size:11px;color:var(--muted-strong)">Driver</label>
      <div style="font-size:12px;font-weight:500">${escapeHtml(mod.driver)}</div>
    </div>`;
    html += hwRenderDriverParams(mod);
  }

  // Actions
  html += `<div class="hw-prop-actions">`;
  const settingsKey = hwSettingsKeyForDriver(mod.driver);
  if (settingsKey) {
    html += `<button type="button" class="btn ghost" data-hw-action="open-settings">Open ${escapeHtml(hwDriverDisplayName(mod.driver))} Settings</button>`;
  }
  if (mod.driver === "modbus-tcp" || mod.driver === "mqtt") {
    const canTestConnection = !!(typeof onlineState === "object" && onlineState && onlineState.connected);
    html += `<button type="button" class="btn ghost" data-hw-action="test-connection" title="${canTestConnection ? "Test connection" : "Go online first to test"}"${canTestConnection ? "" : " disabled"}>Test Connection</button>`;
  }
  if (mod.paletteType !== "cpu") {
    html += `<button type="button" class="btn ghost" data-hw-action="remove" style="color:var(--danger)">Remove Module</button>`;
  }
  html += `</div>`;

  panel.innerHTML = html;

  // Bind inputs
  panel.querySelectorAll("[data-hw-prop]").forEach((input) => {
    input.addEventListener("change", (e) => {
      hwUpdateModuleProperty(mod.id, e.target.dataset.hwProp, e.target.value);
    });
  });
  panel.querySelectorAll("[data-hw-param]").forEach((input) => {
    input.addEventListener("change", (e) => {
      const target = e.target;
      const nextValue = target.type === "checkbox" ? !!target.checked : target.value;
      hwUpdateModuleParam(mod.id, target.dataset.hwParam, nextValue);
    });
  });
  panel.querySelectorAll("[data-hw-action]").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      const action = e.currentTarget.dataset.hwAction;
      if (action === "remove") hwRemoveModule(mod.id);
      if (action === "open-settings") hwOpenSettingsForModule(mod);
      if (action === "test-connection") hwTestConnection(mod);
    });
  });
}

function hwRenderDriverParams(mod) {
  const p = mod.params || {};
  let html = "";
  switch (mod.driver) {
    case "modbus-tcp":
      html += hwField("address", "Server Address", p.address || "127.0.0.1:502");
      html += hwField("unit_id", "Unit ID", p.unit_id ?? 1, "number");
      html += hwField("input_start", "Input Start", p.input_start ?? 0, "number");
      html += hwField("output_start", "Output Start", p.output_start ?? 0, "number");
      html += hwField("timeout_ms", "Timeout (ms)", p.timeout_ms ?? 500, "number");
      html += hwSelect("on_error", "Error Policy", p.on_error || "fault", ["fault", "warn", "ignore"]);
      break;
    case "mqtt":
      html += hwField("broker", "Broker", p.broker || "127.0.0.1:1883");
      html += hwField("client_id", "Client ID", p.client_id || "");
      html += hwField("topic_in", "Topic In", p.topic_in || "trust/io/in");
      html += hwField("topic_out", "Topic Out", p.topic_out || "trust/io/out");
      html += hwField("username", "Username", p.username || "");
      html += hwField("password", "Password", p.password || "", "password");
      html += hwToggle("tls", "TLS", !!p.tls);
      html += hwToggle("allow_insecure_remote", "Allow Insecure Remote", !!p.allow_insecure_remote);
      html += hwField("reconnect_ms", "Reconnect (ms)", p.reconnect_ms ?? 500, "number");
      html += hwField("keep_alive_s", "Keep-alive (s)", p.keep_alive_s ?? 5, "number");
      break;
    case "gpio":
      html += hwSelect("backend", "Backend", p.backend || "libgpiod", ["libgpiod", "sysfs"]);
      html += hwField("chip", "GPIO chip", p.chip || "/dev/gpiochip0");
      html += hwField("sysfs_base", "Sysfs Base", p.sysfs_base || "/sys/class/gpio");
      html += hwJsonField("inputs_json", "Inputs (JSON)", p.inputs ?? []);
      html += hwJsonField("outputs_json", "Outputs (JSON)", p.outputs ?? []);
      break;
    case "ethercat":
      html += hwField("adapter", "Adapter", p.adapter || "mock");
      html += hwField("timeout_ms", "Timeout (ms)", p.timeout_ms ?? 250, "number");
      html += hwField("cycle_warn_ms", "Cycle Warn (ms)", p.cycle_warn_ms ?? 5, "number");
      html += hwSelect("on_error", "Error Policy", p.on_error || "fault", ["fault", "warn", "ignore"]);
      html += hwJsonField("modules_json", "Modules (JSON)", p.modules ?? []);
      html += hwJsonField("mock_inputs_json", "Mock Inputs (JSON)", p.mock_inputs ?? []);
      html += hwField("mock_latency_ms", "Mock Latency (ms)", p.mock_latency_ms ?? 0, "number");
      html += hwToggle("mock_fail_read", "Mock Fail Read", !!p.mock_fail_read);
      html += hwToggle("mock_fail_write", "Mock Fail Write", !!p.mock_fail_write);
      break;
  }
  return html;
}

function hwField(param, label, value, type) {
  const inputType = type || "text";
  return `<div class="field">
    <label style="font-size:11px;color:var(--muted-strong)">${escapeHtml(label)}</label>
    <input type="${inputType}" value="${escapeAttr(String(value))}" data-hw-param="${escapeAttr(param)}" style="font-size:12px"/>
  </div>`;
}

function hwSelect(param, label, value, options) {
  const opts = options.map((o) => `<option value="${escapeAttr(o)}"${o === value ? " selected" : ""}>${escapeHtml(o)}</option>`).join("");
  return `<div class="field">
    <label style="font-size:11px;color:var(--muted-strong)">${escapeHtml(label)}</label>
    <select data-hw-param="${escapeAttr(param)}" style="font-size:12px">${opts}</select>
  </div>`;
}

function hwToggle(param, label, value) {
  return `<div class="field">
    <label style="display:flex;align-items:center;gap:8px;font-size:11px;color:var(--muted-strong)">
      <input type="checkbox" data-hw-param="${escapeAttr(param)}"${value ? " checked" : ""}/>
      <span>${escapeHtml(label)}</span>
    </label>
  </div>`;
}

function hwJsonField(param, label, value) {
  let text = "[]";
  if (typeof value === "string") {
    text = value;
  } else {
    try {
      text = JSON.stringify(value ?? [], null, 2);
    } catch {
      text = "[]";
    }
  }
  return `<div class="field">
    <label style="font-size:11px;color:var(--muted-strong)">${escapeHtml(label)}</label>
    <textarea data-hw-param="${escapeAttr(param)}" class="settings-json-input" spellcheck="false">${escapeHtml(text)}</textarea>
  </div>`;
}

function hwUpdateModuleProperty(moduleId, prop, value) {
  const mod = hwState.modules.find((m) => m.id === moduleId);
  if (!mod) return;
  if (prop === "label") {
    mod.label = value;
    if (hwState.cy) {
      const node = hwState.cy.getElementById(moduleId);
      if (node.length) {
        const nextLabel = hwModuleLabelWithChannels(mod);
        node.data("label", nextLabel);
        node.data("cardTitle", hwCardTrimText(mod.label, 26));
        node.data("cardSubtitle", "");
        node.data("cardBadge", "");
        node.data("height", hwNodeHeightForLabel(nextLabel));
        node.data("cardImage", hwNodeCardImageForData(node.data(), hwResolveCytoscapeTheme()));
      }
    }
  }
  hwRenderAddressTable();
  hwRenderSummary();
  hwRenderDriverCards();
  if (hwState.selectedModuleId === moduleId) {
    hwRenderPropertyPanel(mod);
  }
}

function hwUpdateModuleParam(moduleId, param, value) {
  const mod = hwState.modules.find((m) => m.id === moduleId);
  if (!mod || !mod.params) return;
  const numFields = ["unit_id", "input_start", "output_start", "timeout_ms", "reconnect_ms", "keep_alive_s", "cycle_warn_ms", "mock_latency_ms"];
  const toggleFields = ["tls", "allow_insecure_remote", "mock_fail_read", "mock_fail_write"];
  const jsonFields = ["inputs_json", "outputs_json", "modules_json", "mock_inputs_json"];
  if (numFields.includes(param)) {
    mod.params[param] = Number(value) || 0;
  } else if (toggleFields.includes(param)) {
    mod.params[param] = !!value;
  } else if (jsonFields.includes(param)) {
    const targetKey = param.replace(/_json$/, "");
    const fallback = "[]";
    const text = String(value ?? "").trim() || fallback;
    try {
      const parsed = JSON.parse(text);
      if (!Array.isArray(parsed)) {
        throw new Error("Expected JSON array");
      }
      mod.params[targetKey] = parsed;
    } catch (err) {
      if (typeof showIdeToast === "function") {
        showIdeToast(`Invalid JSON for ${param}: ${err.message || err}`, "error");
      }
      return;
    }
  } else {
    mod.params[param] = value;
  }
  hwRenderDriverCards();
  hwQueuePersistIoConfig();
  if (hwState.selectedModuleId === moduleId) {
    hwRenderPropertyPanel(mod);
  }
}

function hwOpenSettingsForModule(mod) {
  if (!mod) return;
  const actions = hwSettingsActionsForDriver(mod.driver);
  const first = actions[0] || null;
  const key = first?.key || hwSettingsKeyForDriver(mod.driver);
  if (!key) return;
  const category = first?.category || hwSettingsCategoryForKey(key);
  hwOpenSettingsForKey(key, category, {
    runtimeId: hwState.activeRuntimeId,
  });
}

function hwOpenSettingsForKey(key, category, options) {
  const opts = options || {};
  const nextKey = String(key || "").trim();
  const nextCategory = String(category || "").trim() || hwSettingsCategoryForKey(nextKey) || "all";
  if (!nextKey && !nextCategory) return;
  if (typeof switchIdeTab === "function") {
    switchIdeTab("settings");
  }
  document.dispatchEvent(new CustomEvent("ide-settings-focus-request", {
    detail: {
      category: nextCategory,
      key: nextKey,
      runtimeId: String(opts.runtimeId || hwState.activeRuntimeId || "").trim(),
      source: String(opts.source || "hardware").trim() || "hardware",
      context: (opts.context && typeof opts.context === "object") ? opts.context : null,
    },
  }));
}

function hwRuntimeIdForFabricEdge(meta) {
  return String(meta?.ownerRuntimeId || meta?.runtimeId || "").trim();
}

function hwSettingsDriverForFabricEdge(meta) {
  const kind = String(meta?.type || "").trim().toLowerCase();
  if (!kind) return "";
  if (kind === "runtime_cloud") return "cloud-links";
  if (kind === "mqtt") return "mqtt";
  if (kind === "modbus") return "modbus-tcp";
  if (kind === "ethercat") return "ethercat";
  if (kind === "opcua") return "opcua";
  if (kind === "mesh") return "mesh";
  if (kind === "discovery") return "discovery";
  if (kind === "web") return "web";
  return "";
}

function hwSettingsDriverForEndpointProto(proto) {
  const value = String(proto || "").trim().toLowerCase();
  if (!value) return "";
  if (value === "modbus") return "modbus-tcp";
  if (value === "runtime-external") return "cloud-links";
  return value;
}

function hwOpenSettingsForFabricEndpoint(meta) {
  const runtimeIds = Array.isArray(meta?.runtimeIds)
    ? meta.runtimeIds.map((value) => String(value || "").trim()).filter(Boolean)
    : [];
  const runtimeId = runtimeIds[0] || hwState.activeRuntimeId;
  const driver = hwSettingsDriverForEndpointProto(meta?.proto);
  const key = hwSettingsKeyForDriver(driver);
  if (!key) return;
  const category = hwSettingsCategoryForKey(key) || "communication";
  hwOpenSettingsForKey(key, category, {
    runtimeId,
    context: {
      kind: "fabric-endpoint",
      proto: String(meta?.proto || "").trim(),
      label: String(meta?.label || "").trim(),
    },
  });
}

function hwOpenSettingsForFabricEdge(meta, reason) {
  const runtimeId = hwRuntimeIdForFabricEdge(meta);
  const driver = hwSettingsDriverForFabricEdge(meta);
  const key = hwSettingsKeyForDriver(driver) || "runtime_cloud.links.transports_json";
  const category = hwSettingsCategoryForKey(key) || "communication";
  hwOpenSettingsForKey(key, category, {
    runtimeId,
    context: {
      kind: "fabric-edge",
      action: String(reason || "open").trim() || "open",
      edgeType: String(meta?.type || "").trim(),
      source: String(meta?.source || "").trim(),
      target: String(meta?.target || "").trim(),
      transport: String(meta?.transport || "").trim(),
    },
  });
}

function hwSettingsKeyForDriver(driverName) {
  const name = String(driverName || "").toLowerCase();
  if (!name) return "";
  if (name === "simulated") return "io.simulated.inputs";
  if (name === "loopback") return "io.simulated.inputs";
  if (name === "mqtt") return "io.mqtt.broker";
  if (name === "modbus-tcp") return "io.modbus.address";
  if (name === "gpio") return "io.gpio.backend";
  if (name === "ethercat") return "io.ethercat.adapter";
  if (name === "opcua") return "opcua.enabled";
  if (name === "discovery") return "discovery.enabled";
  if (name === "web") return "web.enabled";
  if (name === "mesh") return "mesh.enabled";
  if (name === "cloud") return "runtime_cloud.profile";
  if (name === "cloud-wan") return "runtime_cloud.wan.allow_write_json";
  if (name === "cloud-links") return "runtime_cloud.links.transports_json";
  if (name === "control") return "control.debug_enabled";
  if (name === "tls") return "tls.mode";
  if (name === "deploy") return "deploy.require_signed";
  if (name === "watchdog") return "watchdog.enabled";
  if (name === "fault") return "fault.policy";
  if (name === "retain") return "retain.mode";
  if (name === "observability") return "observability.enabled";
  return "";
}

function hwTestConnection(mod) {
  if (!mod || !mod.driver) return;
  if (!onlineState || !onlineState.connected) {
    if (typeof showIdeToast === "function") {
      showIdeToast("Go online first to test connections.", "warn");
    }
    return;
  }
  const driver = String(mod.driver || "").toLowerCase();
  const timeoutMs = 1200;

  if (driver === "modbus-tcp") {
    const address = String(mod.params?.address || "").trim();
    if (!address) {
      if (typeof showIdeToast === "function") {
        showIdeToast("No endpoint configured for this module.", "warn");
      }
      return;
    }
    apiJson("/api/io/modbus-test", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify({
        address,
        timeout_ms: timeoutMs,
      }),
      timeoutMs: 3000,
    }).then((result) => {
      if (result && result.ok) {
        if (typeof showIdeToast === "function") {
          showIdeToast(`Connection to ${address} succeeded.`, "success");
        }
        return;
      }
      const error = result?.error || "connection failed";
      if (typeof showIdeToast === "function") {
        showIdeToast(`Connection failed: ${error}`, "error");
      }
    }).catch((err) => {
      if (typeof showIdeToast === "function") {
        showIdeToast(`Connection failed: ${err.message || err}`, "error");
      }
    });
    return;
  }

  if (driver === "mqtt") {
    const broker = String(mod.params?.broker || "").trim();
    if (!broker) {
      if (typeof showIdeToast === "function") {
        showIdeToast("No MQTT broker configured for this module.", "warn");
      }
      return;
    }
    apiJson("/api/io/mqtt-test", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify({
        broker,
        timeout_ms: timeoutMs,
      }),
      timeoutMs: 3000,
    }).then((result) => {
      if (result && result.ok) {
        if (typeof showIdeToast === "function") {
          showIdeToast(`MQTT broker ${broker} is reachable.`, "success");
        }
        return;
      }
      const error = result?.error || "connection failed";
      if (typeof showIdeToast === "function") {
        showIdeToast(`MQTT test failed: ${error}`, "error");
      }
    }).catch((err) => {
      if (typeof showIdeToast === "function") {
        showIdeToast(`MQTT test failed: ${err.message || err}`, "error");
      }
    });
    return;
  }

  if (typeof showIdeToast === "function") {
    showIdeToast("Connection test is currently available for Modbus TCP and MQTT.", "warn");
  }
}

// ── Address Map Table (US-4.4) ─────────────────────────

function hwRenderAddressTable() {
  const table = el.hwAddressTable;
  if (!table) return;

  const rows = hwBuildAddressRows();
  if (rows.length === 0) {
    table.innerHTML = '<div class="hw-driver-empty">No I/O addresses yet. Add a module or apply a preset to generate the address map.</div>';
    return;
  }

  let html = `<table class="data-table hw-address-table">
    <thead><tr>
      <th>Address</th><th>Type</th><th>Module</th><th>Channel</th><th>Used in Code</th><th>Value</th>
    </tr></thead><tbody>`;

  for (const row of rows) {
    const cls = row.conflict ? "hw-row-conflict" : (row.usedInCode ? "" : "hw-row-unused");
    const forceTag = row.forced ? ' <span class="hw-force-badge">F</span>' : "";
    html += `<tr class="${cls}" data-hw-address-row="${escapeAttr(row.address)}">
      <td class="mono">${escapeHtml(row.address)}</td>
      <td>${escapeHtml(row.type)}</td>
      <td>${escapeHtml(row.module)}</td>
      <td>${escapeHtml(row.channel)}</td>
      <td>${row.usedInCode ? '<a class="hw-code-link" data-hw-address="' + escapeAttr(row.address) + '">Yes</a>' : '<span class="muted">No</span>'}</td>
      <td class="mono">${escapeHtml(String(row.value))}${forceTag}</td>
    </tr>`;
  }
  html += "</tbody></table>";
  table.innerHTML = html;

  // Bind code links
  table.querySelectorAll("[data-hw-address]").forEach((link) => {
    link.addEventListener("click", () => {
      hwJumpToAddress(link.dataset.hwAddress);
    });
  });
  table.querySelectorAll("[data-hw-address-row]").forEach((row) => {
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      const address = String(row.dataset.hwAddressRow || "").trim();
      if (!address) return;
      void hwOpenAddressContextMenu(address);
    });
  });
}

function hwAddressUsedInCode(address) {
  // Check if any open file references this address literal
  if (!state.openTabs) return false;
  for (const [, tab] of state.openTabs) {
    if (tab.content && tab.content.includes(address)) return true;
  }
  return false;
}

function hwJumpToAddress(address) {
  // Switch to Code tab and search for address
  if (typeof switchIdeTab === "function") switchIdeTab("code");
  if (typeof workspaceSearchFor === "function") {
    workspaceSearchFor(address);
  }
}

function hwAddressLooksBoolean(address) {
  return /^%[IQ]X/i.test(String(address || "").trim());
}

function hwNormalizeForceValue(address, raw) {
  const text = String(raw || "").trim();
  if (!text) return null;
  if (hwAddressLooksBoolean(address)) {
    const normalized = text.toUpperCase();
    if (normalized === "TRUE" || normalized === "1") return "TRUE";
    if (normalized === "FALSE" || normalized === "0") return "FALSE";
    return null;
  }
  const asNumber = Number(text);
  if (Number.isFinite(asNumber)) return String(asNumber);
  return text;
}

async function hwForceAddress(address, value) {
  if (typeof debugForceIoValue === "function") {
    await debugForceIoValue(address, value);
    return;
  }
  await runtimeControlRequest({
    id: 1,
    type: "io.force",
    params: { address, value: String(value) },
  }, { timeoutMs: 3000 });
}

async function hwUnforceAddress(address) {
  if (typeof debugUnforceIoValue === "function") {
    await debugUnforceIoValue(address);
    return;
  }
  await runtimeControlRequest({
    id: 1,
    type: "io.unforce",
    params: { address },
  }, { timeoutMs: 3000 });
}

function hwPickContextAddress(mod) {
  if (!mod || !Array.isArray(mod.addresses) || mod.addresses.length === 0) return null;
  if (mod.addresses.length === 1) return mod.addresses[0];
  const picked = window.prompt(
    `Select channel address for ${mod.label}`,
    mod.addresses[0],
  );
  if (picked == null) return null;
  const normalized = String(picked).trim();
  if (!normalized) return null;
  if (mod.addresses.includes(normalized)) return normalized;
  if (typeof showIdeToast === "function") {
    showIdeToast("Address is not part of this module.", "warn");
  }
  return null;
}

async function hwOpenAddressContextMenu(address) {
  const normalizedAddress = String(address || "").trim();
  if (!normalizedAddress) return;
  if (!onlineState || !onlineState.connected) {
    if (typeof showIdeToast === "function") {
      showIdeToast("Go online first to force/unforce channels.", "warn");
    }
    return;
  }
  const forced = hwState.forcedAddresses.has(normalizedAddress);
  const action = String(window.prompt(
    `Channel ${normalizedAddress}\nAction: force | release | code`,
    forced ? "release" : "force",
  ) || "").trim().toLowerCase();
  if (!action) return;
  if (action === "code") {
    hwJumpToAddress(normalizedAddress);
    return;
  }
  if (action === "release") {
    try {
      await hwUnforceAddress(normalizedAddress);
      await hwPollIoValues();
    } catch (err) {
      if (typeof showIdeToast === "function") {
        showIdeToast(`Release force failed: ${err.message || err}`, "error");
      }
    }
    return;
  }
  if (action === "force") {
    const suggested = hwAddressLooksBoolean(normalizedAddress) ? "TRUE" : "0";
    const nextRaw = window.prompt(`Force value for ${normalizedAddress}`, suggested);
    if (nextRaw == null) return;
    const value = hwNormalizeForceValue(normalizedAddress, nextRaw);
    if (value == null) {
      if (typeof showIdeToast === "function") {
        showIdeToast("Invalid force value for this channel type.", "error");
      }
      return;
    }
    try {
      await hwForceAddress(normalizedAddress, value);
      await hwPollIoValues();
    } catch (err) {
      if (typeof showIdeToast === "function") {
        showIdeToast(`Force failed: ${err.message || err}`, "error");
      }
    }
  }
}

// ── Live I/O Values (US-4.5) ───────────────────────────

function hwStartLivePolling() {
  hwStopLivePolling();
  hwState.livePollingTimer = setInterval(hwPollIoValues, 500);
}

function hwStopLivePolling() {
  if (hwState.livePollingTimer) {
    clearInterval(hwState.livePollingTimer);
    hwState.livePollingTimer = null;
  }
}

async function hwPollIoValues() {
  try {
    const values = await runtimeControlRequest({
      id: 1,
      type: "io.list",
    }, { timeoutMs: 2000 });
    if (!values || typeof values !== "object") return;
    hwState.ioValues = {};
    if (Array.isArray(values.inputs)) {
      for (const entry of values.inputs) {
        hwState.ioValues[entry.address] = entry.value;
      }
    }
    if (Array.isArray(values.outputs)) {
      for (const entry of values.outputs) {
        hwState.ioValues[entry.address] = entry.value;
      }
    }
    // Update forced set
    hwState.forcedAddresses.clear();
    if (Array.isArray(values.forced)) {
      for (const addr of values.forced) {
        hwState.forcedAddresses.add(addr);
      }
    }
    hwRenderAddressTable();
    hwUpdateCanvasLiveValues();
  } catch {
    // Ignore polling errors
  }
}

function hwUpdateCanvasLiveValues() {
  if (!hwState.cy || hwState.viewMode !== "canvas") return;
  for (const mod of hwState.modules) {
    const node = hwState.cy.getElementById(mod.id);
    if (!node.length) continue;
    let summary = mod.label;
    if (mod.addresses.length > 0) {
      const vals = mod.addresses.slice(0, 4).map((addr) => {
        const v = hwState.ioValues[addr];
        const forced = hwState.forcedAddresses.has(addr);
        if (v === undefined || v === null) return `${addr}: --`;
        const tag = forced ? " [F]" : "";
        return `${addr}: ${v}${tag}`;
      });
      summary += "\n" + vals.join("\n");
      if (mod.addresses.length > 4) {
        summary += `\n... +${mod.addresses.length - 4} more`;
      }
    } else if (hwIsCommunicationModule(mod) && mod.driver) {
      summary += `\n${hwDriverDisplayName(mod.driver)}`;
    }
    node.data("label", summary);
    node.data("height", hwNodeHeightForLabel(summary));
  }
}

// ── Escaping Helpers ───────────────────────────────────

function escapeHtml(str) {
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(str) {
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// ── Initialization ─────────────────────────────────────

function hwInit() {
  if (hwState.initialized) return;
  if (typeof el !== "object" || !el || !el.hardwarePalette || !el.hwEmptyState || !el.hwCanvas) {
    return;
  }
  hwState.initialized = true;

  renderHardwarePalette();
  hwRenderPresets();
  hwEnsureThemeObserver();

  // View toggle buttons
  if (el.hwViewCanvas) {
    el.hwViewCanvas.addEventListener("click", () => hwSetViewMode("canvas"));
  }
  if (el.hwViewTable) {
    el.hwViewTable.addEventListener("click", () => hwSetViewMode("table"));
  }
  if (el.hwFitCanvasBtn) {
    el.hwFitCanvasBtn.addEventListener("click", () => {
      if (!hwState.cy) return;
      hwScheduleCanvasRelayout({ padding: 26 });
    });
  }
  if (el.hwCenterCanvasBtn) {
    el.hwCenterCanvasBtn.addEventListener("click", () => {
      if (!hwState.cy) return;
      if (hwState.selectedModuleId) {
        const selectedNode = hwState.cy.getElementById(hwState.selectedModuleId);
        if (selectedNode.length) {
          hwState.cy.center(selectedNode);
          return;
        }
      }
      hwState.cy.center();
    });
  }
  if (el.hwToggleInspectorBtn) {
    el.hwToggleInspectorBtn.addEventListener("click", () => {
      hwSetInspectorCollapsed(!hwState.inspectorCollapsed);
    });
  }
  if (el.hwToggleDriversBtn) {
    el.hwToggleDriversBtn.addEventListener("click", () => {
      hwSetDriversCollapsed(!hwState.driversCollapsed);
    });
  }
  if (el.hwDriversPanelToggleBtn) {
    el.hwDriversPanelToggleBtn.addEventListener("click", () => {
      hwSetDriversCollapsed(!hwState.driversCollapsed);
    });
  }
  if (el.hwLegendToggleBtn) {
    el.hwLegendToggleBtn.addEventListener("click", () => {
      hwSetLegendVisible(!hwState.legendVisible);
    });
  }
  if (el.hwFullscreenBtn) {
    el.hwFullscreenBtn.addEventListener("click", () => {
      void hwToggleCanvasFullscreen();
    });
  }
  if (el.hwReloadConfigBtn) {
    el.hwReloadConfigBtn.addEventListener("click", () => {
      hwState.hydratedProject = null;
      void hwHydrateFromProjectConfig(true);
      if (typeof showIdeToast === "function") {
        showIdeToast("Hardware view reloaded from workspace TOML files.", "success");
      }
    });
  }
  if (el.hwRuntimeSelect) {
    el.hwRuntimeSelect.addEventListener("change", () => {
      const nextRuntime = String(el.hwRuntimeSelect.value || "").trim();
      if (!nextRuntime) return;
      hwSetActiveRuntimeId(nextRuntime);
      hwApplyWorkspaceRuntimeModels(hwState.hydratedProject || "");
    });
  }
  if (el.hwCtxCreateLinkBtn) {
    el.hwCtxCreateLinkBtn.addEventListener("click", () => {
      const runtimeId = String(hwState.contextRuntimeMeta?.runtimeId || "").trim();
      hwHideNodeContextMenu();
      if (!runtimeId) return;
      hwState.linkCreateSourceRuntimeId = runtimeId;
      hwMarkAddLinkButtonActive(true);
      hwSetLinkFlowHint(true, `Source selected: ${runtimeId}. Select target runtime.`);
      if (typeof showIdeToast === "function") {
        showIdeToast(`Source selected: ${runtimeId}. Select target runtime.`, "warn");
      }
    });
  }
  if (el.hwCtxRuntimeSettingsBtn) {
    el.hwCtxRuntimeSettingsBtn.addEventListener("click", () => {
      const meta = hwState.contextRuntimeMeta;
      hwHideNodeContextMenu();
      if (!meta) return;
      const runtimeId = String(meta.runtimeId || "").trim();
      if (!runtimeId) return;
      if (meta.type === "runtime") {
        hwOpenSettingsForKey("resource.name", "general", { runtimeId });
        return;
      }
      hwOpenSettingsForFabricEndpoint(meta);
    });
  }
  if (el.hwCtxRuntimeCommSettingsBtn) {
    el.hwCtxRuntimeCommSettingsBtn.addEventListener("click", () => {
      const meta = hwState.contextRuntimeMeta;
      hwHideNodeContextMenu();
      if (!meta) return;
      if (meta.type !== "runtime") return;
      const runtimeId = String(meta.runtimeId || "").trim();
      if (!runtimeId) return;
      hwOpenSettingsForKey("runtime_cloud.links.transports_json", "communication", { runtimeId });
    });
  }
  if (el.hwCtxCreateLinkFromEdgeBtn) {
    el.hwCtxCreateLinkFromEdgeBtn.addEventListener("click", () => {
      const meta = hwState.contextEdgeMeta;
      hwHideEdgeContextMenu();
      if (!meta || meta.type !== "runtime_cloud") return;
      const source = String(meta.source || "").trim();
      const target = String(meta.target || "").trim();
      if (!source || !target) return;
      void hwCreateRuntimeCloudLink(source, target);
    });
  }
  if (el.hwCtxEditLinkBtn) {
    el.hwCtxEditLinkBtn.addEventListener("click", () => {
      const meta = hwState.contextEdgeMeta;
      hwHideEdgeContextMenu();
      if (!meta) return;
      hwRenderFabricEdgePanel(meta);
      hwSetInspectorCollapsed(false);
    });
  }
  if (el.hwCtxDeleteLinkBtn) {
    el.hwCtxDeleteLinkBtn.addEventListener("click", () => {
      const meta = hwState.contextEdgeMeta;
      hwHideEdgeContextMenu();
      if (!meta) return;
      void hwDeleteFabricEdge(meta);
    });
  }
  if (el.hwCtxOpenLinkSettingsBtn) {
    el.hwCtxOpenLinkSettingsBtn.addEventListener("click", () => {
      const meta = hwState.contextEdgeMeta;
      hwHideEdgeContextMenu();
      if (!meta) return;
      hwOpenSettingsForFabricEdge(meta, "link");
    });
  }
  if (el.hwCtxOpenTransportSettingsBtn) {
    el.hwCtxOpenTransportSettingsBtn.addEventListener("click", () => {
      const meta = hwState.contextEdgeMeta;
      hwHideEdgeContextMenu();
      if (!meta) return;
      hwOpenSettingsForFabricEdge(meta, "transport");
    });
  }
  const linkFlowCancelBtn = document.getElementById("hwLinkFlowCancelBtn");
  if (linkFlowCancelBtn) {
    linkFlowCancelBtn.addEventListener("click", () => {
      hwState.linkCreateSourceRuntimeId = "";
      hwMarkAddLinkButtonActive(false);
    });
  }
  document.addEventListener("click", (event) => {
    const target = event.target;
    if (target instanceof Node) {
      if (el.hwNodeContextMenu && !el.hwNodeContextMenu.classList.contains("ide-hidden") && !el.hwNodeContextMenu.contains(target)) {
        hwHideNodeContextMenu();
      }
      if (el.hwEdgeContextMenu && !el.hwEdgeContextMenu.classList.contains("ide-hidden") && !el.hwEdgeContextMenu.contains(target)) {
        hwHideEdgeContextMenu();
      }
    } else {
      hwHideHardwareContextMenus();
    }
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      hwHideHardwareContextMenus();
    }
  });

  window.addEventListener("resize", () => {
    const active = typeof ideGetActiveTab === "function"
      ? ideGetActiveTab()
      : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
    if (active === "hardware") {
      hwScheduleCanvasRelayout({ padding: 56 });
    }
  });
  document.addEventListener("fullscreenchange", () => {
    hwState.isCanvasFullscreen = !!document.fullscreenElement;
    hwSyncFullscreenButtonState();
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(() => hwScheduleCanvasRelayout({ padding: 30 }));
    } else {
      hwScheduleCanvasRelayout({ padding: 30 });
    }
  });

  hwRenderSummary();
  hwRenderDriverCards();
  hwRenderLegend();
  hwRenderTransportPills();
  hwSetLegendVisible(false);
  hwSetInspectorCollapsed(true);
  hwSetDriversCollapsed(true);
  hwSyncFullscreenButtonState();
  hwSetViewMode(hwState.viewMode);
  hwRenderPropertyPanel(null);
  hwUpdateEmptyState();
}

function hwActivate() {
  hwInit();
  if (!hwState.initialized) {
    return;
  }
  if (!hwState.cy) {
    hwInitCanvas();
  }
  if (hwState.cy) {
    hwScheduleCanvasRelayout({ padding: 32 });
  }
  if (onlineState?.connected) {
    hwStartLivePolling();
  } else {
    hwStopLivePolling();
  }
  // Project I/O configuration is available as soon as the IDE session has
  // selected a project.  Do not make the hardware surface wait for the
  // editor's WASM analysis bootstrap: that can be unavailable while the
  // authenticated configuration endpoint is already usable.
  if (!(typeof state === "object" && state && state.activeProject)) {
    setTimeout(() => {
      const active = typeof ideGetActiveTab === "function"
        ? ideGetActiveTab()
        : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
      if (active === "hardware") {
        hwActivate();
      }
    }, 250);
    return;
  }
  void hwHydrateFromProjectConfig(false)
    .then(() => {
      hwScheduleCanvasRelayout({ padding: 58 });
    })
    .catch(() => {
      // No-op; hydration failures are surfaced through existing status toasts.
    });
}

function hwDeactivate() {
  hwStopLivePolling();
  hwClearScheduledRelayouts();
  hwHideHardwareContextMenus();
}

// ── Tab change listener ────────────────────────────────

document.addEventListener("ide-tab-change", (e) => {
  const tab = e.detail && e.detail.tab;
  if (tab === "hardware") {
    hwActivate();
  } else {
    hwDeactivate();
  }
});

document.addEventListener("ide-project-changed", () => {
  hwState.hydratedProject = null;
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "hardware") {
    hwActivate();
  }
});

document.addEventListener("ide-io-config-updated", (event) => {
  if (event?.detail?.source === "hardware") return;
  hwState.hydratedProject = null;
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "hardware") {
    hwActivate();
  }
});

document.addEventListener("ide-runtime-config-updated", (event) => {
  if (event?.detail?.source === "hardware") return;
  hwState.hydratedProject = null;
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "hardware") {
    hwActivate();
  }
});

document.addEventListener(HW_RUNTIME_SELECTION_EVENT, (event) => {
  const source = String(event?.detail?.source || "").trim().toLowerCase();
  if (source === "hardware") return;
  const runtimeId = String(event?.detail?.runtimeId || "").trim();
  if (!runtimeId) return;

  if (hwState.workspaceRuntimes.length > 0) {
    const exists = hwState.workspaceRuntimes.some((entry) => entry.runtimeId === runtimeId);
    if (!exists) return;
  }

  const changed = hwSetActiveRuntimeId(runtimeId, {
    source: source || "external",
    broadcast: false,
  });
  if (!changed) return;
  if (hwState.workspaceRuntimes.length > 0) {
    hwRenderRuntimeSelector();
    hwApplyWorkspaceRuntimeModels(hwState.hydratedProject || "");
  }
});

document.addEventListener("ide-session-ready", () => {
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active !== "hardware") return;
  hwActivate();
});

document.addEventListener("ide-runtime-connected", () => {
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "hardware") {
    hwStartLivePolling();
    if (hwState.selectedModuleId) {
      const mod = hwState.modules.find((m) => m.id === hwState.selectedModuleId);
      hwRenderPropertyPanel(mod || null);
    }
  }
});

document.addEventListener("ide-runtime-disconnected", () => {
  hwStopLivePolling();
  if (hwState.selectedModuleId) {
    const mod = hwState.modules.find((m) => m.id === hwState.selectedModuleId);
    hwRenderPropertyPanel(mod || null);
  }
});

function hwSyncInitialTabActivation(retryCount) {
  const attempts = Number(retryCount) || 0;
  // ide-tabs.js can fire before ide.js exposes shared DOM bindings.
  if (typeof el !== "object" || !el || !el.hardwarePalette || !el.hwEmptyState || !el.hwCanvas) {
    if (attempts < 80) {
      setTimeout(() => hwSyncInitialTabActivation(attempts + 1), 25);
    }
    return;
  }
  const active = typeof ideGetActiveTab === "function"
    ? ideGetActiveTab()
    : document.querySelector(".ide-tab-btn.active")?.dataset?.tab;
  if (active === "hardware" || window.location.pathname.startsWith("/ide/hardware")) {
    hwActivate();
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    setTimeout(hwSyncInitialTabActivation, 0);
  });
} else {
  setTimeout(hwSyncInitialTabActivation, 0);
}
