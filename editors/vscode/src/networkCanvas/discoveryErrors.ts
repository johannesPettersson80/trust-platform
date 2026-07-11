export const ADS_UDP_IDENTIFY_BLOCKED_ERROR =
  "ads_udp_identify_blocked" as const;

export type DiscoveryErrorCode = typeof ADS_UDP_IDENTIFY_BLOCKED_ERROR;

export function classifyDiscoveryErrorCode(
  protocol: string,
  error: unknown
): DiscoveryErrorCode | undefined {
  if (protocol !== "ads") {
    return undefined;
  }
  const detail = error instanceof Error ? error.message : String(error ?? "");
  const normalized = detail.toLowerCase();
  if (
    normalized.includes("udpidentifyblocked") ||
    normalized.includes("udp_identify_blocked") ||
    (normalized.includes("udp identify") &&
      normalized.includes("no target answered"))
  ) {
    return ADS_UDP_IDENTIFY_BLOCKED_ERROR;
  }
  return undefined;
}

export function isDiscoveryErrorCode(
  value: unknown
): value is DiscoveryErrorCode {
  return value === ADS_UDP_IDENTIFY_BLOCKED_ERROR;
}

export function offersAdsManualIdentityRecovery(
  errorCode: DiscoveryErrorCode | undefined
): boolean {
  return errorCode === ADS_UDP_IDENTIFY_BLOCKED_ERROR;
}

export function discoveryTypedFailureMessage(
  errorCode: DiscoveryErrorCode
): string {
  switch (errorCode) {
    case ADS_UDP_IDENTIFY_BLOCKED_ERROR:
      return "TwinCAT identity did not answer UDP discovery. Enter the target AMS Net ID to continue manually.";
  }
}

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
      return "TwinCAT";
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
