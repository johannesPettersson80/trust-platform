export function discoveryRuntimeFailureMessage(
  protocol: string,
  error: unknown
): string {
  const detail = error instanceof Error ? error.message.toLowerCase() : "";
  const recovery = "Reconnect or start the selected runtime, then scan again.";
  if (detail.includes("auth")) {
    return `The selected runtime rejected authentication for ${discoveryProtocolName(protocol)} discovery. Check its auth token. ${recovery}`;
  }
  if (detail.includes("timeout") || detail.includes("timed out")) {
    return `${discoveryProtocolName(protocol)} discovery timed out on the selected runtime. ${recovery}`;
  }
  return `The selected runtime could not complete ${discoveryProtocolName(protocol)} discovery. ${recovery}`;
}

export function discoveryProtocolName(protocol: string): string {
  switch (protocol) {
    case "ads":
      return "ADS";
    case "discovery":
      return "truST runtime";
    case "modbus_tcp":
      return "Modbus";
    case "opcua_client":
      return "OPC UA server";
    case "mqtt":
      return "MQTT broker";
    case "ethercat":
      return "EtherCAT";
    case "gpio":
      return "GPIO";
    default:
      return protocol.replace(/_/g, " ");
  }
}
