import type { DiscoverCandidate } from "../offlineComm";

export const DEFAULT_ADS_PORT = 851;
export const MIN_ADS_PORT = 1;
export const MAX_ADS_PORT = 65_535;

export type AdsPortParseResult = { port: number; error?: undefined } | { error: string; port?: undefined };

export function parseAdsPortInput(raw: string): AdsPortParseResult {
  const value = raw.trim();
  if (!value) {
    return { port: DEFAULT_ADS_PORT };
  }
  if (!/^\d+$/.test(value)) {
    return { error: "Enter a whole-number ADS port from 1 to 65535." };
  }
  const port = Number(value);
  if (!Number.isSafeInteger(port) || port < MIN_ADS_PORT || port > MAX_ADS_PORT) {
    return { error: "ADS port must be between 1 and 65535." };
  }
  return { port };
}

export function candidateAdsPort(candidate: DiscoverCandidate): number {
  return adsTargetPort(candidate.params);
}

export function adsTargetPort(target: Record<string, unknown>): number {
  const port = target.ams_port;
  return typeof port === "number" && Number.isInteger(port) && port >= MIN_ADS_PORT && port <= MAX_ADS_PORT
    ? port
    : DEFAULT_ADS_PORT;
}

export function adsPortDraftIsStale(
  raw: string,
  target: Record<string, unknown>
): boolean {
  const parsed = parseAdsPortInput(raw);
  return parsed.port === undefined || parsed.port !== adsTargetPort(target);
}

export function adsTargetNetId(target: Record<string, unknown>): string {
  const netId = target.ams_net_id ?? target.target_net_id;
  return typeof netId === "string" && netId.trim().length > 0
    ? netId.trim()
    : "Unknown AMS Net ID";
}

export function withAdsTargetPort(
  target: Record<string, unknown>,
  port: number
): Record<string, unknown> {
  return { ...target, ams_port: port };
}

/**
 * Recovery for a service already selected and verified in Discover.
 * Happy-path handoff stays one click; only a failed browse exposes Retry, and
 * it must retry the exact selected target instead of reopening a port editor.
 */
export function confirmedAdsBrowseRetryTarget(
  target: Record<string, unknown>,
  loading: boolean,
  browseFailed: boolean
): Record<string, unknown> | undefined {
  return target.ads_port_confirmed === true && !loading && browseFailed
    ? target
    : undefined;
}

export function withCandidateAdsPort(
  candidate: DiscoverCandidate,
  port: number
): DiscoverCandidate {
  return {
    ...candidate,
    params: withAdsTargetPort(candidate.params, port),
  };
}
