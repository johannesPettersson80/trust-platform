import * as vscode from "vscode";

import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { RuntimeTarget } from "../runtimeTarget";
import type {
  DiscoveryRequestToken,
  DiscoveryRequestTracker,
} from "./discoverySession";
import {
  discoveryProtocolName,
  discoveryRuntimeFailureMessage,
} from "./discoveryErrors";
import {
  offlineCommDiscover,
  offlineBrowseSymbols,
  type BrowseSymbolsResponse,
  type DiscoverCandidate,
} from "./offlineComm";
import {
  adsDiscoveryPorts,
  probeAdsCandidatePorts,
  respondingAdsPorts,
} from "./adsDiscoveryPorts";

interface DiscoveryRequestItem {
  readonly protocol: string;
  readonly cidr?: string;
  readonly host?: string;
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

export function parseDiscoveryEnvelope(
  message: Record<string, unknown>
): DiscoveryMessageEnvelope | undefined {
  if (
    typeof message.sessionId !== "string" ||
    message.sessionId.length === 0 ||
    !Number.isSafeInteger(message.requestId) ||
    typeof message.requestId !== "number" ||
    !isRecord(message.request)
  ) {
    return undefined;
  }
  const origin =
    typeof message.request.origin === "string"
      ? message.request.origin
      : "this_host";
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
  const all: DiscoverCandidate[] = [];
  let adsIdentityFound = false;
  const configuredAdsPorts = adsDiscoveryPorts(
    vscode.workspace
      .getConfiguration("trust")
      .get<unknown[]>("ads.discoveryPorts"),
  );

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
    const { protocol, cidr, host } = item;
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
    if (viaRuntime && runtimeTarget?.endpoint) {
      try {
        const response = await sendRuntimeControlRequest<{
          candidates?: DiscoverCandidate[];
        }>(
          runtimeTarget.endpoint,
          runtimeTarget.authToken,
          "comm.discover",
          { protocol, origin: "runtime", scope: { cidr, host } },
          { timeoutMs: 8000 }
        );
        candidates = response?.candidates ?? [];
      } catch (error) {
        postIfCurrent({
          type: "discoverResults",
          candidates: all,
          error: discoveryRuntimeFailureMessage(protocol, error),
        });
        return;
      }
    } else {
      const response = await offlineCommDiscover(
        extensionContext,
        protocol,
        "this-host",
        { cidr, host }
      );
      candidates = response?.candidates ?? [];
    }
    if (!tracker.isCurrent(token, panel)) {
      return;
    }
    const stamped = await Promise.all(
      candidates.map(async (candidate) => {
        const discovered = {
          ...candidate,
          protocol,
          originRuntimeId: runtimeRequested ? request.origin : undefined,
        };
        if (protocol !== "ads") {
          return discovered;
        }
        return probeAdsCandidatePorts(
          discovered,
          configuredAdsPorts,
          async (target) => {
            if (viaRuntime && runtimeTarget?.endpoint) {
              try {
                return await sendRuntimeControlRequest<BrowseSymbolsResponse>(
                  runtimeTarget.endpoint,
                  runtimeTarget.authToken,
                  "comm.browse_symbols",
                  { protocol: "ads", kind: "symbols", target },
                  { timeoutMs: 8_000 },
                );
              } catch {
                return undefined;
              }
            }
            return offlineBrowseSymbols(
              extensionContext,
              "ads",
              target,
              "symbols",
            );
          },
        );
      }),
    );
    const verified = stamped.filter(
      (candidate) =>
        candidate.protocol !== "ads" ||
        respondingAdsPorts(candidate.params).length > 0,
    );
    if (protocol === "ads" && stamped.length > 0) {
      adsIdentityFound = true;
    }
    all.push(...verified);
    if (
      !postIfCurrent({
        type: "discoverProgress",
        protocol,
        label,
        status: "done",
        count: verified.length,
      })
    ) {
      return;
    }
  }

  const adsVerificationError =
    adsIdentityFound && !all.some((candidate) => candidate.protocol === "ads")
      ? `TwinCAT identity was found, but none of the configured ADS ports ` +
        `(${configuredAdsPorts.join(", ")}) responded.`
      : undefined;
  postIfCurrent({
    type: "discoverResults",
    candidates: all,
    error: adsVerificationError,
  });
}

function discoverLabel(protocol: string, host?: string, cidr?: string): string {
  const label = discoveryProtocolName(protocol);
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
