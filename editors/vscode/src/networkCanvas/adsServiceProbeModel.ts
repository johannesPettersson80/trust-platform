import type {
  BrowseSymbolsResponse,
  RoutePlan,
  SymbolNode,
} from "./offlineComm";

export const PLC_RUNTIME_PORTS = [851, 852, 853, 854] as const;
export const COMMON_ADS_SERVICE_PORTS = [301, 501] as const;
export const AUTOMATIC_ADS_SERVICE_PORTS = [
  ...PLC_RUNTIME_PORTS,
  ...COMMON_ADS_SERVICE_PORTS,
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
    (port) => !AUTOMATIC_ADS_SERVICE_PORTS.includes(
      port as (typeof AUTOMATIC_ADS_SERVICE_PORTS)[number]
    )
  ).length;
  const maxAdditional =
    MAX_ADS_SERVICE_PROBES - AUTOMATIC_ADS_SERVICE_PORTS.length;
  if (additionalCount > maxAdditional) {
    return {
      ports,
      error: `Add up to ${maxAdditional} additional ADS service ports (${MAX_ADS_SERVICE_PROBES} total including preset ADS services 851–854, 301, and 501).`,
    };
  }
  return { ports };
}

export function planAdsServicePorts(
  customPorts: readonly number[]
): readonly number[] {
  const planned: number[] = [];
  const seen = new Set<number>();
  for (const port of [...AUTOMATIC_ADS_SERVICE_PORTS, ...customPorts]) {
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
  const contractError = adsBrowseResponseContractError(response);
  if (contractError) {
    return {
      port,
      status: "check_failed",
      symbolCount: 0,
      usable: false,
      error: {
        code: "invalid_browse_response",
        message: contractError,
      },
    };
  }
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
    return {
      port,
      status: "check_failed",
      symbolCount: 0,
      usable: false,
      error: {
        code: "unexplained_empty_browse_response",
        message:
          "ADS browse returned no variables and no explicit empty-service result.",
      },
    };
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

/**
 * A parseable JSON value is not proof that an ADS service answered. Only the
 * versioned browse contract emitted by trust-runtime may contribute a
 * responding logical-service row.
 */
export function adsBrowseResponseContractError(
  response: BrowseSymbolsResponse
): string | undefined {
  const raw = response as unknown as Record<string, unknown>;
  if (raw.schema_version !== 1) {
    return "ADS browse response has an unsupported or missing schema version.";
  }
  if (raw.protocol !== "ads" || raw.kind !== "symbols") {
    return "ADS browse response has the wrong protocol or result kind.";
  }
  if (!Array.isArray(raw.tree) || !validSymbolTree(raw.tree)) {
    return "ADS browse response has an invalid symbol tree.";
  }
  if (
    raw.error !== undefined &&
    (!isRecord(raw.error) ||
      typeof raw.error.code !== "string" ||
      raw.error.code.trim().length === 0 ||
      typeof raw.error.message !== "string" ||
      raw.error.message.trim().length === 0)
  ) {
    return "ADS browse response has invalid structured error evidence.";
  }
  if (
    raw.route !== undefined &&
    (!isRecord(raw.route) ||
      typeof raw.route.status !== "string" ||
      !["missing", "not_required", "ok"].includes(raw.route.status))
  ) {
    return "ADS browse response has invalid route evidence.";
  }
  return undefined;
}

export function autoSelectUsableAdsService(
  results: readonly AdsServiceProbeResult[]
): number | undefined {
  const usable = results.filter((result) => result.usable);
  return usable.length === 1 ? usable[0].port : undefined;
}

export function isRespondingAdsServiceResult(
  result: AdsServiceProbeResult
): boolean {
  return (
    result.status === "available" ||
    result.status === "unsupported" ||
    result.status === "empty"
  );
}

export function groupAdsServiceProbeResults(
  results: readonly AdsServiceProbeResult[]
): {
  readonly responding: readonly AdsServiceProbeResult[];
  readonly diagnostics: readonly AdsServiceProbeResult[];
} {
  const responding: AdsServiceProbeResult[] = [];
  const diagnostics: AdsServiceProbeResult[] = [];
  for (const result of results) {
    (isRespondingAdsServiceResult(result) ? responding : diagnostics).push(
      result
    );
  }
  return { responding, diagnostics };
}

/** True when an ADS service answered, even if it has no browsable symbol table. */
export function didAnyAdsServiceRespond(
  results: readonly AdsServiceProbeResult[]
): boolean {
  return results.some(isRespondingAdsServiceResult);
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
    // A missing reciprocal route is target-wide, so later logical ports cannot
    // succeed until recovery is completed. Other failures can be specific to
    // one ADS service; keep checking the remaining requested ports so a broken
    // or protected service on 851 never hides a responding 852/301/501.
    if (result.status === "route_missing") {
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

function validSymbolTree(nodes: readonly unknown[]): boolean {
  return nodes.every((node) => {
    if (
      !isRecord(node) ||
      typeof node.id !== "string" ||
      node.id.trim().length === 0 ||
      typeof node.name !== "string" ||
      node.name.trim().length === 0 ||
      typeof node.path !== "string" ||
      node.path.trim().length === 0
    ) {
      return false;
    }
    return node.children === undefined
      ? true
      : Array.isArray(node.children) && validSymbolTree(node.children);
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
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
