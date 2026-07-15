export interface AdsClientPortSummary {
  readonly port: number;
  readonly tagCount: number;
}

export interface AdsClientDeviceSummary {
  readonly address: string;
  readonly amsNetId: string;
  readonly ports: readonly AdsClientPortSummary[];
}

export interface AdsClientSummaryModel {
  readonly devices: readonly AdsClientDeviceSummary[];
  readonly status: string;
  readonly statusKind: "ok" | "neutral" | "error";
  readonly enabled: boolean;
  readonly configPath: string;
  readonly updateIntervalMs: number;
}

export function buildAdsClientSummaryModel(
  params: Record<string, unknown> | undefined,
  health: string,
  detail: string,
  runtimeHealth = "",
): AdsClientSummaryModel {
  const values = params ?? {};
  const devices = new Map<string, MutableDeviceSummary>();
  const connections = Array.isArray(values.connections) ? values.connections : [];

  for (const candidate of connections) {
    if (!isRecord(candidate)) {
      continue;
    }
    const address = stringField(candidate, "host", "ip") ?? "Unknown";
    const amsNetId =
      stringField(candidate, "target_net_id", "ams_net_id") ?? "Unknown";
    const key = `${address}|${amsNetId}`;
    const device = devices.get(key) ?? {
      address,
      amsNetId,
      ports: new Map<number, number>(),
    };
    const port = adsPort(candidate.ams_port);
    if (port !== undefined) {
      const tagCount = Array.isArray(candidate.points) ? candidate.points.length : 0;
      device.ports.set(port, (device.ports.get(port) ?? 0) + tagCount);
    }
    devices.set(key, device);
  }

  return {
    devices: [...devices.values()].map((device) => ({
      address: device.address,
      amsNetId: device.amsNetId,
      ports: [...device.ports]
        .sort(([left], [right]) => left - right)
        .map(([port, tagCount]) => ({ port, tagCount })),
    })),
    ...adsStatus(health, detail, runtimeHealth),
    enabled: booleanValue(values.enabled, true),
    configPath: stringField(values, "config_path") ?? "ads.toml",
    updateIntervalMs: positiveNumber(values.worker_tick_interval_ms, 20),
  };
}

interface MutableDeviceSummary {
  readonly address: string;
  readonly amsNetId: string;
  readonly ports: Map<number, number>;
}

function adsStatus(
  health: string,
  detail: string,
  runtimeHealth: string,
): Pick<AdsClientSummaryModel, "status" | "statusKind"> {
  const normalizedHealth = health.trim().toLowerCase();
  const normalizedDetail = detail.trim().toLowerCase();
  const normalizedRuntimeHealth = runtimeHealth.trim().toLowerCase();
  const runtimeIsLive = ["connected", "simulate", "running"].includes(
    normalizedRuntimeHealth,
  );
  if (runtimeIsLive && normalizedHealth === "configured_policy") {
    return {
      status: "Runtime running — ADS is configured.",
      statusKind: "neutral",
    };
  }
  if (
    normalizedHealth === "configured_policy" ||
    normalizedHealth === "stopped" ||
    normalizedDetail.includes("runtime is not running")
  ) {
    return {
      status: "Runtime stopped — start it to read tags.",
      statusKind: "neutral",
    };
  }
  if (normalizedHealth === "connected") {
    return {
      status: "Connected — reading configured ADS tags.",
      statusKind: "ok",
    };
  }
  if (normalizedHealth === "disabled") {
    return {
      status: "ADS is disabled — enable it in Advanced settings.",
      statusKind: "neutral",
    };
  }
  if (["error", "degraded", "auth_failed", "runtime_unreachable"].includes(normalizedHealth)) {
    return {
      status: detail.trim() || "ADS communication needs attention.",
      statusKind: "error",
    };
  }
  return {
    status: detail.trim() || "ADS is configured.",
    statusKind: "neutral",
  };
}

function booleanValue(value: unknown, fallback: boolean): boolean {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    if (value.trim().toLowerCase() === "true") {
      return true;
    }
    if (value.trim().toLowerCase() === "false") {
      return false;
    }
  }
  return fallback;
}

function positiveNumber(value: unknown, fallback: number): number {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : fallback;
}

function adsPort(value: unknown): number | undefined {
  const number = Number(value);
  return Number.isInteger(number) && number >= 1 && number <= 65535
    ? number
    : undefined;
}

function stringField(
  value: Record<string, unknown>,
  ...keys: string[]
): string | undefined {
  for (const key of keys) {
    const item = value[key];
    if (typeof item === "string" && item.trim().length > 0) {
      return item.trim();
    }
  }
  return undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
