export interface AdsConnectionStatus {
  name: string;
  state: string;
  point_count: number;
  degraded_points: number;
  summary: string;
}

export interface AdsStatusReport {
  overall: string;
  summary: string;
  connections: AdsConnectionStatus[];
}

export interface AdsStatusSummary {
  text: string;
  overall: string;
  deviceCount: number;
  degradedCount: number;
}

export function summarizeAdsStatus(
  status: AdsStatusReport | undefined
): AdsStatusSummary {
  if (!status) {
    return {
      text: "ADS status unavailable",
      overall: "unknown",
      deviceCount: 0,
      degradedCount: 0,
    };
  }
  const connections = Array.isArray(status.connections)
    ? status.connections
    : [];
  const deviceCount = connections.length;
  if (deviceCount === 0) {
    return {
      text: status.summary || "ADS is not configured.",
      overall: normalizeOverall(status.overall),
      deviceCount,
      degradedCount: 0,
    };
  }
  const degradedCount = connections.filter(
    (connection) =>
      connection.degraded_points > 0 ||
      !["connected", "disabled"].includes(connection.state)
  ).length;
  return {
    text: `ADS: ${deviceCount} device${deviceCount === 1 ? "" : "s"} · ${degradedCount} degraded`,
    overall: normalizeOverall(status.overall),
    deviceCount,
    degradedCount,
  };
}

function normalizeOverall(value: string | undefined): string {
  const normalized = (value ?? "unknown").toLowerCase();
  return normalized.length > 0 ? normalized : "unknown";
}
