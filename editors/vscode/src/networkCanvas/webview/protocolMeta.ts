import { t } from "./theme";

// Protocol identity colours the wire + the endpoint role band only — never floods a node body.
// The hues are shared product theme roles derived from VS Code chart tokens, not per-canvas hexes.
const PROTOCOL_COLORS: Record<string, string> = {
  modbus_tcp: t.protocolBlue,
  modbus: t.protocolBlue,
  mqtt: t.protocolOrange,
  opcua: t.protocolCyan,
  opcua_client: t.protocolCyan,
  ads: t.protocolCyan,
  ads_server: t.protocolCyan,
  mesh: t.protocolMuted,
  runtime_cloud: t.protocolPurple,
  federation: t.protocolPurple,
  realtime: t.protocolPurple,
  realtime_t0: t.protocolPurple,
  gpio: t.protocolCyan,
  ethercat: t.protocolPurple,
};

export function protocolColor(protocol: string): string {
  return PROTOCOL_COLORS[protocol] ?? t.protocolMuted;
}

const PROTOCOL_LABELS: Record<string, string> = {
  modbus_tcp: "MB",
  modbus: "MB",
  mqtt: "MQ",
  opcua: "UA",
  opcua_client: "UA",
  ads: "ADS client",
  ads_server: "ADS server",
  mesh: "ME",
  runtime_cloud: "FE",
  federation: "FE",
  realtime: "T0",
  realtime_t0: "T0",
  gpio: "IO",
  ethercat: "EC",
  web: "WB",
  discovery: "DS",
  simulated: "SM",
  loopback: "LB",
};

export function protocolBadgeLabel(protocol: string): string {
  return PROTOCOL_LABELS[protocol] ?? protocol.replace(/_/g, "").slice(0, 2).toUpperCase();
}

// §8: spell the protocol (no cryptic 2-letter tile). Readable display names.
const PROTOCOL_NAMES: Record<string, string> = {
  modbus_tcp: "Modbus TCP",
  modbus: "Modbus",
  ethercat: "EtherCAT",
  opcua: "OPC UA server",
  opcua_client: "OPC UA client",
  ads: "ADS client",
  ads_server: "ADS server",
  mqtt: "MQTT",
  mesh: "Mesh",
  openot: "OpenOT",
  discovery: "Discovery",
  realtime: "Realtime",
  realtime_t0: "Realtime",
  runtime_cloud: "Federation",
  federation: "Federation",
  gpio: "GPIO",
  simulated: "Simulated I/O",
  loopback: "Loopback I/O",
  web: "Web",
};

export function protocolName(protocol: string): string {
  return PROTOCOL_NAMES[protocol] ?? protocol.replace(/_/g, " ");
}
