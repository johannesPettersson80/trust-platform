import type {
  BrowseSymbolsResponse,
  RoutePlan,
  SymbolNode,
} from "./offlineComm";

export const PLC_RUNTIME_PORTS = [851, 852, 853, 854] as const;
export const COMMON_TWINCAT_SERVICE_PORTS = [301, 501] as const;
export const AUTOMATIC_TWINCAT_SERVICE_PORTS = [
  ...PLC_RUNTIME_PORTS,
  ...COMMON_TWINCAT_SERVICE_PORTS,
] as const;
export const MAX_ADS_SERVICE_PROBES = 10;

export type AdsServiceProbeStatus =
  | "available"
  | "unsupported"
  | "empty"
  | "unavailable"
  | "check_failed"
  | "route_missing";

export interface AdsServiceProbeResult {
  readonly port: number;
  readonly status: AdsServiceProbeStatus;
  readonly symbolCount: number;
  readonly usable: boolean;
  readonly routePlan?: RoutePlan;
  readonly error?: { readonly code: string; readonly message: string };
}

export interface AdsServiceProbeViewState {
  readonly probing: boolean;
  readonly results: readonly AdsServiceProbeResult[];
  readonly currentPort?: number;
  readonly completed?: boolean;
  readonly error?: string;
}

export interface AdsServiceProbeSequenceOptions {
  /** Re-checked before every network request so closing or replacing a scan stops fanout. */
  readonly isActive?: () => boolean;
  readonly onBeforeProbe?: (
    port: number,
    index: number,
    total: number
  ) => void | Promise<void>;
}

export function parseCustomAdsPorts(input: string): {
  readonly ports: readonly number[];
  readonly error?: string;
} {
  const trimmed = input.trim();
  if (!trimmed) {
    return { ports: [] };
  }
  const ports: number[] = [];
  const seen = new Set<number>();
  for (const raw of trimmed.split(",")) {
    const value = raw.trim();
    if (!/^\d+$/.test(value)) {
      return {
        ports: [],
        error: "Each logical ADS service port must be a whole number.",
      };
    }
    const port = Number(value);
    if (!Number.isSafeInteger(port) || port < 1 || port > 65535) {
      return {
        ports: [],
        error: "Logical ADS service ports must be between 1 and 65535.",
      };
    }
    if (!seen.has(port)) {
      seen.add(port);
      ports.push(port);
    }
  }
  const additionalCount = ports.filter(
    (port) => !AUTOMATIC_TWINCAT_SERVICE_PORTS.includes(
      port as (typeof AUTOMATIC_TWINCAT_SERVICE_PORTS)[number]
    )
  ).length;
  const maxAdditional =
    MAX_ADS_SERVICE_PROBES - AUTOMATIC_TWINCAT_SERVICE_PORTS.length;
  if (additionalCount > maxAdditional) {
    return {
      ports,
      error: `Add up to ${maxAdditional} additional ADS service ports (${MAX_ADS_SERVICE_PROBES} total including preset TwinCAT services 851–854, 301, and 501).`,
    };
  }
  return { ports };
}

export function planAdsServicePorts(
  customPorts: readonly number[]
): readonly number[] {
  const planned: number[] = [];
  const seen = new Set<number>();
  for (const port of [...AUTOMATIC_TWINCAT_SERVICE_PORTS, ...customPorts]) {
    if (
      planned.length >= MAX_ADS_SERVICE_PROBES ||
      !Number.isSafeInteger(port) ||
      port < 1 ||
      port > 65535 ||
      seen.has(port)
    ) {
      continue;
    }
    seen.add(port);
    planned.push(port);
  }
  return planned;
}

export function classifyAdsServiceProbe(
  port: number,
  response: BrowseSymbolsResponse
): AdsServiceProbeResult {
  if (response.route?.status === "missing") {
    return withOptionalEvidence(
      {
        port,
        status: "route_missing",
        symbolCount: 0,
        usable: false,
      },
      response
    );
  }

  const symbolCount = countLeafSymbols(response.tree);
  if (!response.error && symbolCount > 0) {
    return { port, status: "available", symbolCount, usable: true };
  }
  if (!response.error) {
    return { port, status: "empty", symbolCount: 0, usable: false };
  }

  const status: AdsServiceProbeStatus =
    response.error.code === "symbol_upload_unsupported"
      ? "unsupported"
      : response.error.code === "empty_symbol_table"
        ? "empty"
        : response.error.code === "ads_port_unavailable"
          ? "unavailable"
          : "check_failed";
  return withOptionalEvidence(
    { port, status, symbolCount: 0, usable: false },
    response
  );
}

export function autoSelectUsableAdsService(
  results: readonly AdsServiceProbeResult[]
): number | undefined {
  const usable = results.filter((result) => result.usable);
  return usable.length === 1 ? usable[0].port : undefined;
}

/** Keeps a deliberate choice, but never turns an earlier automatic choice into consent. */
export function resolveSelectedAdsServicePort(
  results: readonly AdsServiceProbeResult[],
  explicitlySelectedPort?: number,
  resultsAreCurrent = true
): number | undefined {
  if (!resultsAreCurrent) {
    return undefined;
  }
  if (
    explicitlySelectedPort !== undefined &&
    results.some(
      (result) =>
        result.usable && result.port === explicitlySelectedPort
    )
  ) {
    return explicitlySelectedPort;
  }
  return autoSelectUsableAdsService(results);
}

export function shouldShowAdsServiceCheckConfirmation(
  probe: AdsServiceProbeViewState | undefined,
  serviceResultsStale: boolean,
  recheckRequested: boolean
): boolean {
  if (!probe || serviceResultsStale || recheckRequested) {
    return true;
  }
  if (probe.probing || !probe.completed) {
    return false;
  }
  return Boolean(
    probe.error ||
      probe.results.length === 0 ||
      probe.results.some((result) => result.status === "check_failed") ||
      !probe.results.some((result) => result.usable)
  );
}

export async function probeAdsServicesSequentially(
  ports: readonly number[],
  probe: (port: number) => Promise<BrowseSymbolsResponse>,
  options: AdsServiceProbeSequenceOptions = {}
): Promise<readonly AdsServiceProbeResult[]> {
  const results: AdsServiceProbeResult[] = [];
  const planned = ports.slice(0, MAX_ADS_SERVICE_PROBES);
  for (const [index, port] of planned.entries()) {
    if (options.isActive && !options.isActive()) {
      break;
    }
    await options.onBeforeProbe?.(port, index, planned.length);
    if (options.isActive && !options.isActive()) {
      break;
    }
    const result = classifyAdsServiceProbe(port, await probe(port));
    results.push(result);
    if (
      result.status === "route_missing" ||
      result.status === "check_failed"
    ) {
      break;
    }
  }
  return results;
}

function countLeafSymbols(nodes: readonly SymbolNode[]): number {
  let count = 0;
  for (const node of nodes) {
    if (node.children?.length) {
      count += countLeafSymbols(node.children);
    } else {
      count += 1;
    }
  }
  return count;
}

function withOptionalEvidence(
  base: AdsServiceProbeResult,
  response: BrowseSymbolsResponse
): AdsServiceProbeResult {
  return {
    ...base,
    ...(response.route?.route_plan
      ? { routePlan: response.route.route_plan }
      : {}),
    ...(response.error ? { error: response.error } : {}),
  };
}
