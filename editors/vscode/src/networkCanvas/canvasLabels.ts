/** User-facing labels shared by host-side notifications and discovery progress. */
export function protocolDisplayName(protocol: string): string {
  switch (protocol) {
    case "ads": return "ADS client";
    case "ads_server": return "ADS server";
    case "opcua": return "OPC UA server";
    case "opcua_client": return "OPC UA client";
    case "modbus_tcp": return "Modbus TCP";
    default: return protocol.replace(/_/g, " ");
  }
}

export function countLabel(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

export function discoverLabel(protocol: string, host?: string, cidr?: string): string {
  const label = discoverProtocolName(protocol);
  if (host) return `${label} @ ${host}`;
  if (cidr) return `${label} ${cidr}`;
  return label;
}

function discoverProtocolName(protocol: string): string {
  switch (protocol) {
    case "ads": return "TwinCAT (ADS)";
    case "discovery": return "truST runtimes";
    case "modbus_tcp": return "Modbus";
    case "opcua_client": return "OPC UA server";
    case "mqtt": return "MQTT broker";
    case "ethercat": return "EtherCAT";
    case "gpio": return "GPIO";
    default: return protocol.replace(/_/g, " ");
  }
}
