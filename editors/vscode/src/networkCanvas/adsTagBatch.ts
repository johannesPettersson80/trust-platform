import {
  adsConnectionNameForTarget,
  adsDiscoveryPorts,
} from "./adsDiscoveryPorts";

export interface AdsTagPortSelection {
  readonly port: number;
  readonly paths: readonly string[];
}

export interface AdsTagPortPath {
  readonly port: number;
  readonly path: string;
}

export interface AdsTagAddPlan {
  readonly connectionName: string;
  readonly paths: readonly string[];
}

export function planAdsTagAdd(
  connections: unknown,
  target: Record<string, unknown>,
  port: number,
  paths: readonly string[],
): AdsTagAddPlan {
  const matching = adsConnectionsForTarget(connections, target).filter(
    (connection) => connection.ams_port === port,
  );
  const connectionName = stringField(matching[0] ?? {}, "name") ??
    adsConnectionNameForTarget({ ...target, ams_port: port }, "ads_import");
  return {
    connectionName,
    paths: [...new Set(paths.map((path) => path.trim()).filter(Boolean))]
      .sort(),
  };
}

export interface AdsTagPortImportResult {
  readonly port: number;
  readonly paths: readonly string[];
  readonly applied: boolean;
  readonly addedCount: number;
  readonly message: string;
}

export interface AdsTagBatchImportResult {
  readonly operation?: "add" | "remove";
  readonly applied: boolean;
  readonly addedCount: number;
  readonly removedCount?: number;
  readonly restartRequired: boolean;
  readonly ports: readonly AdsTagPortImportResult[];
}

export function adsConnectionsForTarget(
  value: unknown,
  target: Record<string, unknown>,
): Record<string, unknown>[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter(
    (connection): connection is Record<string, unknown> =>
      isRecord(connection) && sameAdsDevice(connection, target),
  );
}

export function adsTagSelectionsFromConnections(
  value: unknown,
  target: Record<string, unknown>,
): AdsTagPortSelection[] {
  return normalizeAdsTagSelections(
    adsConnectionsForTarget(value, target).map((connection) => ({
      port: connection.ams_port,
      paths: Array.isArray(connection.points)
        ? connection.points.flatMap((point): string[] => {
            if (!isRecord(point)) {
              return [];
            }
            const path = point.symbol ?? point.path;
            return typeof path === "string" ? [path] : [];
          })
        : [],
    })),
  );
}

export function normalizeAdsTagSelections(
  value: unknown,
): AdsTagPortSelection[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const selections = new Map<number, Set<string>>();
  for (const item of value) {
    if (!isRecord(item)) {
      continue;
    }
    const port = adsDiscoveryPorts([item.port])[0];
    if (!port || !Array.isArray(item.paths)) {
      continue;
    }
    const paths = selections.get(port) ?? new Set<string>();
    for (const path of item.paths) {
      if (typeof path === "string" && path.trim().length > 0) {
        paths.add(path.trim());
      }
    }
    if (paths.size > 0) {
      selections.set(port, paths);
    }
  }
  return [...selections]
    .sort(([left], [right]) => left - right)
    .map(([port, paths]) => ({ port, paths: [...paths].sort() }));
}

export function adsTagBatchSummary(result: AdsTagBatchImportResult): string {
  const successfulPorts = result.ports
    .filter((port) => port.applied)
    .map((port) => port.port);
  if (result.operation === "remove") {
    if (successfulPorts.length === 0) {
      return "No ADS tags were removed.";
    }
    const removedCount = result.removedCount ?? 0;
    const tagLabel = removedCount === 1 ? "tag" : "tags";
    const portLabel = successfulPorts.length === 1 ? "port" : "ports";
    return `Removed ${removedCount} ${tagLabel} from ADS ${portLabel} ${successfulPorts.join(", ")}.`;
  }
  if (successfulPorts.length === 0) {
    return "No ADS tags were added.";
  }
  const tagLabel = result.addedCount === 1 ? "tag" : "tags";
  const portLabel = successfulPorts.length === 1 ? "port" : "ports";
  return `Added ${result.addedCount} ${tagLabel} from ADS ${portLabel} ${successfulPorts.join(", ")}.`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function sameAdsDevice(
  connection: Record<string, unknown>,
  target: Record<string, unknown>,
): boolean {
  const targetNetId = stringField(target, "target_net_id", "ams_net_id");
  const connectionNetId = stringField(
    connection,
    "target_net_id",
    "ams_net_id",
  );
  if (targetNetId && connectionNetId && targetNetId !== connectionNetId) {
    return false;
  }
  const targetHost = stringField(target, "host", "ip");
  const connectionHost = stringField(connection, "host", "ip");
  if (targetHost && connectionHost && targetHost !== connectionHost) {
    return false;
  }
  return Boolean(
    (targetNetId && connectionNetId) ||
    (targetHost && connectionHost) ||
    (!targetNetId && !targetHost),
  );
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
