export const ADS_UDP_IDENTIFY_BLOCKED_ERROR =
  "ads_udp_identify_blocked" as const;
export const ADS_LOCAL_ROUTER_UNAVAILABLE_ERROR =
  "ads_local_router_unavailable" as const;
export const ADS_DISCOVERY_BLOCKED_ERROR = "ads_discovery_blocked" as const;
export const ADS_DISCOVERY_UNAVAILABLE_ERROR =
  "ads_discovery_unavailable" as const;

export type DiscoveryErrorCode =
  | typeof ADS_UDP_IDENTIFY_BLOCKED_ERROR
  | typeof ADS_LOCAL_ROUTER_UNAVAILABLE_ERROR
  | typeof ADS_DISCOVERY_BLOCKED_ERROR
  | typeof ADS_DISCOVERY_UNAVAILABLE_ERROR;

export function classifyDiscoveryErrorCode(
  protocol: string,
  error: unknown,
): DiscoveryErrorCode | undefined {
  if (protocol !== "ads") {
    return undefined;
  }
  const detail = error instanceof Error ? error.message : String(error ?? "");
  const normalized = detail.toLowerCase();
  if (
    normalized.includes("localrouterunavailable") ||
    normalized.includes("local_router_unavailable") ||
    normalized.includes("local ads router/runtime check failed")
  ) {
    return ADS_LOCAL_ROUTER_UNAVAILABLE_ERROR;
  }
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
  value: unknown,
): value is DiscoveryErrorCode {
  return (
    value === ADS_UDP_IDENTIFY_BLOCKED_ERROR ||
    value === ADS_LOCAL_ROUTER_UNAVAILABLE_ERROR ||
    value === ADS_DISCOVERY_BLOCKED_ERROR ||
    value === ADS_DISCOVERY_UNAVAILABLE_ERROR
  );
}

export function offersAdsManualIdentityRecovery(
  errorCode: DiscoveryErrorCode | undefined,
): boolean {
  return (
    errorCode === ADS_UDP_IDENTIFY_BLOCKED_ERROR ||
    errorCode === ADS_DISCOVERY_BLOCKED_ERROR
  );
}

export function classifyAdsWarningFailure(
  warnings: readonly string[],
): DiscoveryErrorCode | undefined {
  if (warnings.length === 0) {
    return undefined;
  }
  return warnings.some((warning) =>
    /ads-wire|runtime build|not compiled/i.test(warning),
  )
    ? ADS_DISCOVERY_UNAVAILABLE_ERROR
    : ADS_DISCOVERY_BLOCKED_ERROR;
}

export function discoveryTypedFailureMessage(
  errorCode: DiscoveryErrorCode,
): string {
  switch (errorCode) {
    case ADS_LOCAL_ROUTER_UNAVAILABLE_ERROR:
      return "No local ADS runtime answered. Start the ADS router and intended PLC runtime on this computer, then try again. If both are running, repair the ADS installation.";
    case ADS_UDP_IDENTIFY_BLOCKED_ERROR:
      return "No ADS device answered. Make sure it is running and that your firewall allows truST on this network. Try again, or use Advanced if you know its address.";
    case ADS_DISCOVERY_BLOCKED_ERROR:
      return "ADS discovery could not finish. Make sure the device is running and your firewall allows truST on this network, then try again. If you know its address, use Advanced.";
    case ADS_DISCOVERY_UNAVAILABLE_ERROR:
      return "ADS discovery is not available in this runtime build. Update or reinstall truST, then try again.";
  }
}

export function discoveryRuntimeFailureMessage(
  protocol: string,
  error: unknown,
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
