import * as vscode from "vscode";

import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { RuntimeTarget } from "../runtimeTarget";
import type {
  DiscoveryRequestToken,
  DiscoveryRequestTracker,
} from "./discoverySession";
import {
  ADS_DISCOVERY_BLOCKED_ERROR,
  classifyDiscoveryErrorCode,
  classifyAdsWarningFailure,
  discoveryProtocolName,
  discoveryRuntimeFailureMessage,
  discoveryTypedFailureMessage,
} from "./discoveryErrors";
import {
  offlineCommDiscover,
  type DiscoverCandidate,
} from "./offlineComm";

interface DiscoveryRequestItem {
  readonly protocol: string;
  readonly cidr?: string;
  readonly host?: string;
  readonly targetAmsNetId?: string;
  readonly amsPort?: number;
}

export interface DiscoveryMessageEnvelope {
  readonly sessionId: string;
  readonly requestId: number;
  readonly request: {
    readonly origin: string;
    readonly originEndpoint?: string;
    readonly items: readonly DiscoveryRequestItem[];
  };
}

export interface DiscoveryControllerContext {
  readonly panel: vscode.WebviewPanel;
  readonly extensionContext: vscode.ExtensionContext;
  readonly runtimeTarget?: RuntimeTarget;
  readonly tracker: DiscoveryRequestTracker<vscode.WebviewPanel>;
  readonly token: DiscoveryRequestToken;
}

const DEFAULT_DISCOVERY_CONTROL_TIMEOUT_MS = 8_000;
const ADS_DISCOVERY_CONTROL_TIMEOUT_MS = 15_000;

/**
 * Native same-computer ADS discovery has a bounded five-second identity scan
 * before sequential 900 ms LAN broadcast windows. Keep the control request
 * alive long enough for several real/virtual Windows interfaces while retaining
 * a finite cancellation boundary.
 */
export function discoveryControlTimeoutMs(protocol: string): number {
  return protocol === "ads"
    ? ADS_DISCOVERY_CONTROL_TIMEOUT_MS
    : DEFAULT_DISCOVERY_CONTROL_TIMEOUT_MS;
}

export function parseDiscoveryEnvelope(
  message: Record<string, unknown>
): DiscoveryMessageEnvelope | undefined {
  if (
    typeof message.sessionId !== "string" ||
    message.sessionId.length === 0 ||
    !Number.isSafeInteger(message.requestId) ||
    typeof message.requestId !== "number" ||
    !isRecord(message.request) ||
    typeof message.request.origin !== "string" ||
    message.request.origin.length === 0
  ) {
    return undefined;
  }
  const origin = message.request.origin;
  const rawItems = Array.isArray(message.request.items)
    ? message.request.items
    : [];
  const items = rawItems.flatMap((raw): DiscoveryRequestItem[] => {
    if (!isRecord(raw) || typeof raw.protocol !== "string") {
      return [];
    }
    return [
      {
        protocol: raw.protocol,
        cidr: typeof raw.cidr === "string" ? raw.cidr : undefined,
        host: typeof raw.host === "string" ? raw.host : undefined,
        targetAmsNetId:
          typeof raw.targetAmsNetId === "string"
            ? raw.targetAmsNetId
            : undefined,
        amsPort:
          typeof raw.amsPort === "number" && Number.isSafeInteger(raw.amsPort)
            ? raw.amsPort
            : undefined,
      },
    ];
  });
  return {
    sessionId: message.sessionId,
    requestId: message.requestId,
    request: {
      origin,
      originEndpoint:
        typeof message.request.originEndpoint === "string"
          ? message.request.originEndpoint
          : undefined,
      items,
    },
  };
}

export async function runNetworkCanvasDiscovery(
  envelope: DiscoveryMessageEnvelope,
  context: DiscoveryControllerContext
): Promise<void> {
  const { panel, extensionContext, runtimeTarget, tracker, token } = context;
  const { sessionId, requestId, request } = envelope;
  const viaRuntime =
    request.origin !== "this_host" &&
    runtimeTarget?.status === "online_reachable" &&
    Boolean(runtimeTarget.endpoint);
  const runtimeRequested = request.origin !== "this_host";
  const automaticAdsSearch =
    request.items.length > 0 &&
    request.items.every((item) => item.protocol === "ads");
  const all: DiscoverCandidate[] = [];
  const automaticFailures: Array<{
    readonly error: string;
    readonly errorCode?: string;
    readonly errorDetails?: readonly string[];
  }> = [];
  const automaticWarnings: string[] = [];

  const postIfCurrent = (payload: Record<string, unknown>): boolean => {
    if (!tracker.isCurrent(token, panel) || !panel.visible) {
      return false;
    }
    void panel.webview.postMessage({ ...payload, sessionId, requestId });
    return true;
  };

  if (runtimeRequested && !viaRuntime) {
    postIfCurrent({
      type: "discoverResults",
      candidates: [],
      error:
        "The selected runtime is no longer reachable. Start or reconnect it, then scan again.",
    });
    return;
  }

  for (const item of request.items) {
    const { protocol, cidr, host, targetAmsNetId, amsPort } = item;
    const label = discoverLabel(protocol, host, cidr);
    if (
      !postIfCurrent({
        type: "discoverProgress",
        protocol,
        label,
        status: "scanning",
      })
    ) {
      return;
    }

    let candidates: DiscoverCandidate[] = [];
    let itemWarnings: string[] = [];
    if (viaRuntime && runtimeTarget?.endpoint) {
      try {
        const response = await sendRuntimeControlRequest<{
          candidates?: DiscoverCandidate[];
          warnings?: string[];
        }>(
          runtimeTarget.endpoint,
          runtimeTarget.authToken,
          "comm.discover",
          {
            protocol,
            origin: "runtime",
            scope: {
              cidr,
              host,
              target_ams_net_id: targetAmsNetId,
              ams_port: amsPort,
            },
          },
          { timeoutMs: discoveryControlTimeoutMs(protocol) }
        );
        candidates = response?.candidates ?? [];
        itemWarnings = normalizeDiscoveryWarnings(response?.warnings);
        automaticWarnings.push(...itemWarnings);
      } catch (error) {
        const classifiedErrorCode = classifyDiscoveryErrorCode(protocol, error);
        const errorCode =
          classifiedErrorCode ??
          (automaticAdsSearch ? ADS_DISCOVERY_BLOCKED_ERROR : undefined);
        const rawDetail = error instanceof Error ? error.message : String(error);
        const message = errorCode
          ? discoveryTypedFailureMessage(errorCode)
          : discoveryRuntimeFailureMessage(protocol, error);
        if (
          !postIfCurrent({
            type: "discoverProgress",
            protocol,
            label,
            status: "failed",
          })
        ) {
          return;
        }
        if (automaticAdsSearch) {
          automaticFailures.push({
            error: message,
            ...(errorCode ? { errorCode } : {}),
            ...(rawDetail ? { errorDetails: [rawDetail] } : {}),
          });
          continue;
        }
        postIfCurrent({
          type: "discoverResults",
          candidates: deduplicateDiscoveryCandidates(all),
          error: message,
          ...(errorCode ? { errorCode } : {}),
          ...(rawDetail ? { errorDetails: [rawDetail] } : {}),
        });
        return;
      }
    } else {
      try {
        const response = await offlineCommDiscover(
          extensionContext,
          protocol,
          "this-host",
          { cidr, host, targetAmsNetId, amsPort }
        );
        candidates = response.candidates ?? [];
        itemWarnings = normalizeDiscoveryWarnings(response.warnings);
        automaticWarnings.push(...itemWarnings);
      } catch (error) {
        const detail = error instanceof Error ? error.message : String(error);
        const classifiedErrorCode = classifyDiscoveryErrorCode(protocol, error);
        const errorCode =
          classifiedErrorCode ??
          (automaticAdsSearch ? ADS_DISCOVERY_BLOCKED_ERROR : undefined);
        const message = errorCode
          ? discoveryTypedFailureMessage(errorCode)
          : discoveryCommandFailureMessage(protocol, detail);
        if (
          !postIfCurrent({
            type: "discoverProgress",
            protocol,
            label,
            status: "failed",
          })
        ) {
          return;
        }
        if (automaticAdsSearch) {
          automaticFailures.push({
            error: message,
            ...(errorCode ? { errorCode } : {}),
            ...(detail ? { errorDetails: [detail] } : {}),
          });
          continue;
        }
        postIfCurrent({
          type: "discoverResults",
          candidates: deduplicateDiscoveryCandidates(all),
          error: message,
          ...(errorCode ? { errorCode } : {}),
          ...(detail ? { errorDetails: [detail] } : {}),
        });
        return;
      }
    }
    if (!tracker.isCurrent(token, panel)) {
      return;
    }
    const stamped = candidates.map((candidate) => ({
      ...candidate,
      protocol,
      ...(runtimeRequested ? { originRuntimeId: request.origin } : {}),
    }));
    all.push(...stamped);
    if (
      !postIfCurrent({
        type: "discoverProgress",
        protocol,
        label,
        status:
          automaticAdsSearch && stamped.length === 0 && itemWarnings.length > 0
            ? "failed"
            : "done",
        ...(automaticAdsSearch && stamped.length === 0 && itemWarnings.length > 0
          ? {}
          : { count: stamped.length }),
      })
    ) {
      return;
    }
  }

  const candidates = deduplicateDiscoveryCandidates(all);
  const failure = candidates.length === 0 ? automaticFailures[0] : undefined;
  const automaticFailureDetails = automaticFailures.flatMap(
    (automaticFailure) =>
      automaticFailure.errorDetails?.length
        ? automaticFailure.errorDetails
        : [automaticFailure.error]
  );
  // Visible copy remains generic. Preserve every raw automatic leg failure
  // only in the collapsed Technical details so support can diagnose partial
  // and zero-result scans without exposing transport jargon by default.
  const warningDetails = [
    ...new Set([...automaticWarnings, ...automaticFailureDetails]),
  ];
  if (automaticAdsSearch && candidates.length === 0 && !failure) {
    // A thrown command/runtime failure already has a classified safe message
    // and code. Only synthesize a terminal code from returned warnings when
    // there was no explicit failure to preserve.
    const warningFailureCode = classifyAdsWarningFailure(automaticWarnings);
    if (warningFailureCode) {
      postIfCurrent({
        type: "discoverResults",
        candidates: [],
        error: discoveryTypedFailureMessage(warningFailureCode),
        errorCode: warningFailureCode,
        errorDetails: warningDetails,
      });
      return;
    }
  }
  const warning =
    automaticAdsSearch && candidates.length > 0 && warningDetails.length > 0
      ? "Some ADS checks did not answer. Results from responding devices are shown."
      : automaticAdsSearch
        ? ""
        : warningDetails.join(" ");
  postIfCurrent({
    type: "discoverResults",
    candidates,
    ...(warning ? { warning } : {}),
    ...(automaticAdsSearch && candidates.length > 0 && warningDetails.length > 0
      ? { warningDetails }
      : {}),
    ...(failure
      ? {
          error: failure.error,
          ...(failure.errorCode ? { errorCode: failure.errorCode } : {}),
          ...(warningDetails.length > 0 ? { errorDetails: warningDetails } : {}),
        }
      : {}),
  });
}

function normalizeDiscoveryWarnings(value: unknown): string[] {
  return Array.isArray(value)
    ? value.flatMap((warning) =>
        typeof warning === "string" && warning.trim().length > 0
          ? [warning.trim()]
          : []
      )
    : [];
}

/** Merge observations of one ADS identity into one result card. */
export function deduplicateDiscoveryCandidates(
  candidates: readonly DiscoverCandidate[]
): DiscoverCandidate[] {
  const unique = new Map<string, DiscoverCandidate>();
  for (const candidate of candidates) {
    const key = discoveryCandidateIdentity(candidate);
    const previous = unique.get(key);
    unique.set(key, previous ? mergeDiscoveryCandidates(previous, candidate) : candidate);
  }
  return [...unique.values()];
}

function discoveryCandidateIdentity(candidate: DiscoverCandidate): string {
  if (candidate.protocol === "ads") {
    const netId = stringParam(candidate.params, "ams_net_id", "target_net_id");
    if (netId) {
      return `ads:${netId.toLowerCase()}`;
    }
  }
  return `${candidate.protocol}:${candidate.id}`;
}

function mergeDiscoveryCandidates(
  first: DiscoverCandidate,
  second: DiscoverCandidate
): DiscoverCandidate {
  const preferred = discoverySourceRank(second.source) > discoverySourceRank(first.source)
    ? second
    : first;
  const fallback = preferred === first ? second : first;
  const warnings = [...new Set([...(fallback.warnings ?? []), ...(preferred.warnings ?? [])])];
  const params = { ...fallback.params, ...preferred.params };
  const respondingAdsPorts = mergeRespondingAdsPorts(
    fallback.params,
    preferred.params
  );
  if (respondingAdsPorts.length > 0) {
    params.responding_ads_ports = respondingAdsPorts;
  }
  const preferredHost = stringParam(preferred.params, "host", "ip");
  const fallbackHost = stringParam(fallback.params, "host", "ip");
  if (isLoopbackHost(preferredHost) && fallbackHost && !isLoopbackHost(fallbackHost)) {
    params.host = fallbackHost;
  }
  return {
    ...fallback,
    ...preferred,
    params,
    ...(warnings.length > 0 ? { warnings } : {}),
  };
}

function mergeRespondingAdsPorts(
  first: Record<string, unknown>,
  second: Record<string, unknown>
): number[] {
  const ports = new Set<number>();
  for (const params of [first, second]) {
    const observed = params.responding_ads_ports;
    if (!Array.isArray(observed)) {
      continue;
    }
    for (const port of observed) {
      if (
        typeof port === "number" &&
        Number.isSafeInteger(port) &&
        port >= 1 &&
        port <= 65_535
      ) {
        ports.add(port);
      }
    }
  }
  return [...ports].sort((left, right) => left - right);
}

function isLoopbackHost(host: string): boolean {
  const normalized = host.trim().toLowerCase();
  return normalized === "localhost" || normalized === "127.0.0.1" || normalized === "::1";
}

function discoverySourceRank(source: string): number {
  switch (source) {
    case "ads_local_router":
      return 4;
    case "ads_identify":
      return 3;
    case "ads_broadcast":
      return 2;
    case "manual":
      return 1;
    default:
      return 0;
  }
}

function stringParam(params: Record<string, unknown>, ...keys: string[]): string {
  for (const key of keys) {
    const value = params[key];
    if (typeof value === "string" && value.trim().length > 0) {
      return value.trim();
    }
  }
  return "";
}

function discoveryCommandFailureMessage(
  protocol: string,
  detail: string
): string {
  const prefix = `${discoveryProtocolName(protocol)} discovery failed`;
  return detail.toLowerCase().startsWith(prefix.toLowerCase())
    ? detail
    : `${prefix}: ${detail}`;
}

export function discoverLabel(
  protocol: string,
  host?: string,
  cidr?: string
): string {
  const label = discoveryProtocolName(protocol);
  if (protocol === "ads" && host === "127.0.0.1") {
    return `${label} on the discovery computer`;
  }
  if (host) {
    return `${label} @ ${host}`;
  }
  if (cidr) {
    return `${label} ${cidr}`;
  }
  return label;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
