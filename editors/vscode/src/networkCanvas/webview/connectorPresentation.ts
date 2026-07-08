export function discoveryConfidenceLabel(value: string): string {
  switch (token(value)) {
    case "confirmed":
      return "Confirmed";
    case "likely":
      return "Likely";
    case "port_reachable":
      return "Port reachable only";
    case "unavailable":
      return "Not verified";
    default:
      return labelFromToken(value);
  }
}

export function discoverySourceLabel(value: string): string {
  switch (token(value)) {
    case "tcp_connect":
      return "Known address";
    case "mdns":
      return "Network discovery";
    case "broadcast":
      return "Network scan";
    case "runtime":
      return "Runtime scan";
    default:
      return labelFromToken(value);
  }
}

export function connectorConnectionLabel(value: string): string {
  switch (token(value)) {
    case "ready":
      return "Ready";
    case "configured":
      return "Configured";
    case "starting":
      return "Starting";
    case "reconnecting":
      return "Reconnecting";
    case "degraded":
    case "stale":
    case "not_ready":
      return "Needs attention";
    case "faulted":
      return "Fault";
    case "disabled":
      return "Disabled";
    default:
      return labelFromToken(value);
  }
}

export function connectorHealthLabel(value: string): string {
  switch (token(value)) {
    case "ok":
      return "OK";
    case "degraded":
      return "Degraded";
    case "faulted":
      return "Fault";
    case "unknown":
      return "Unknown";
    default:
      return labelFromToken(value);
  }
}

export function connectorSignalsSummary(counts: { good?: number; degraded?: number; unavailable?: number } | undefined): string {
  if (!counts) {
    return "";
  }
  const good = Number(counts.good) || 0;
  const needsAttention = (Number(counts.degraded) || 0) + (Number(counts.unavailable) || 0);
  if (needsAttention === 0) {
    return `${good} good`;
  }
  return `${good} good, ${needsAttention} need attention`;
}

function token(value: string): string {
  return String(value || "unknown").trim().toLowerCase();
}

function labelFromToken(value: string): string {
  return token(value)
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ") || "Unknown";
}
