import type {
  DiscoverCandidate,
  RoutePlan,
  SymbolNode,
} from "./offlineComm";

export const DEFAULT_ADS_DISCOVERY_PORTS: readonly number[] = [
  301,
  501,
  851,
  852,
  853,
  854,
];

export interface AdsPortProbeResult {
  readonly tree?: readonly SymbolNode[];
  readonly route?: {
    readonly status?: string;
    readonly route_plan?: RoutePlan;
  };
  readonly error?: { readonly code?: string; readonly message?: string };
}

export interface AdsPortBrowseEvidence {
  readonly port: number;
  readonly tree: readonly SymbolNode[];
  readonly routeMissing: boolean;
  readonly routePlan?: RoutePlan;
  readonly error?: { readonly code?: string; readonly message?: string };
}

export type AdsPortProbe = (
  target: Record<string, unknown>,
  port: number,
) => Promise<AdsPortProbeResult | undefined>;

export function adsDiscoveryPorts(raw: readonly unknown[] | undefined): number[] {
  const values = raw ?? DEFAULT_ADS_DISCOVERY_PORTS;
  return [...new Set(values.filter(isAdsPort))].sort((left, right) => left - right);
}

export function respondingAdsPorts(target: Record<string, unknown>): number[] {
  const raw = Array.isArray(target.responding_ads_ports)
    ? target.responding_ads_ports
    : [];
  return adsDiscoveryPorts(raw);
}

export function adsPortBrowseEvidence(
  target: Record<string, unknown>,
): AdsPortBrowseEvidence[] {
  const raw = Array.isArray(target.ads_port_browse_results)
    ? target.ads_port_browse_results
    : [];
  return raw.flatMap((item): AdsPortBrowseEvidence[] => {
    if (!isRecord(item) || !isAdsPort(item.port) || !Array.isArray(item.tree)) {
      return [];
    }
    const routePlan = isRecord(item.routePlan)
      ? (item.routePlan as RoutePlan)
      : undefined;
    const error = isRecord(item.error)
      ? {
          code: typeof item.error.code === "string" ? item.error.code : undefined,
          message: typeof item.error.message === "string" ? item.error.message : undefined,
        }
      : undefined;
    return [{
      port: item.port,
      tree: item.tree as SymbolNode[],
      routeMissing: item.routeMissing === true,
      routePlan,
      error,
    }];
  }).sort((left, right) => left.port - right.port);
}

export function adsPortResponded(result: AdsPortProbeResult | undefined): boolean {
  if (!result) {
    return false;
  }
  if ((result.tree?.length ?? 0) > 0 || result.route?.status === "ok") {
    return true;
  }
  return result.error?.code === "symbol_upload_unsupported" ||
    result.error?.code === "empty_symbol_table";
}

export async function probeAdsCandidatePorts(
  candidate: DiscoverCandidate,
  ports: readonly number[],
  probe: AdsPortProbe,
): Promise<DiscoverCandidate> {
  const configuredPorts = adsDiscoveryPorts(ports);
  const responding: number[] = [];
  const browseResults: AdsPortBrowseEvidence[] = [];
  for (const port of configuredPorts) {
    try {
      const target = { ...candidate.params, ams_port: port };
      const result = await probe(target, port);
      if (adsPortResponded(result)) {
        responding.push(port);
        browseResults.push({
          port,
          tree: [...(result?.tree ?? [])],
          routeMissing: result?.route?.status === "missing",
          routePlan: result?.route?.route_plan,
          error: result?.error,
        });
      }
    } catch {
      // A failed ADS service probe must not hide services on the remaining ports.
    }
  }
  const currentPort = isAdsPort(candidate.params.ams_port)
    ? candidate.params.ams_port
    : undefined;
  const selectedPort = currentPort && responding.includes(currentPort)
    ? currentPort
    : responding[0] ?? currentPort ?? 851;
  return {
    ...candidate,
    params: {
      ...candidate.params,
      ams_port: selectedPort,
      responding_ads_ports: responding,
      ads_port_browse_results: browseResults,
    },
  };
}

export function adsConnectionNameForTarget(
  target: Record<string, unknown>,
  fallback: string,
): string {
  const base = typeof target.name === "string" && target.name.trim().length > 0
    ? target.name.trim()
    : fallback;
  const port = isAdsPort(target.ams_port) ? target.ams_port : 851;
  const unsuffixedBase = base.replace(/_port_\d+$/, "");
  return respondingAdsPorts(target).length > 1
    ? `${unsuffixedBase}_port_${port}`
    : base;
}

function isAdsPort(value: unknown): value is number {
  return Number.isInteger(value) && typeof value === "number" && value >= 1 && value <= 65535;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
