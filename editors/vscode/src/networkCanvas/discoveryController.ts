import * as vscode from "vscode";

import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { RuntimeTarget } from "../runtimeTarget";
import type {
  DiscoveryRequestToken,
  DiscoveryRequestTracker,
} from "./discoverySession";
import {
  classifyDiscoveryErrorCode,
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
  const all: DiscoverCandidate[] = [];

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
    if (viaRuntime && runtimeTarget?.endpoint) {
      try {
        const response = await sendRuntimeControlRequest<{
          candidates?: DiscoverCandidate[];
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
          { timeoutMs: 8000 }
        );
        candidates = response?.candidates ?? [];
      } catch (error) {
        const errorCode = classifyDiscoveryErrorCode(protocol, error);
        postIfCurrent({
          type: "discoverResults",
          candidates: all,
          error: errorCode
            ? discoveryTypedFailureMessage(errorCode)
            : discoveryRuntimeFailureMessage(protocol, error),
          ...(errorCode ? { errorCode } : {}),
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
      } catch (error) {
        const detail = error instanceof Error ? error.message : String(error);
        const errorCode = classifyDiscoveryErrorCode(protocol, error);
        postIfCurrent({
          type: "discoverResults",
          candidates: all,
          error: errorCode
            ? discoveryTypedFailureMessage(errorCode)
            : discoveryCommandFailureMessage(protocol, detail),
          ...(errorCode ? { errorCode } : {}),
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
      originRuntimeId: runtimeRequested ? request.origin : undefined,
    }));
    all.push(...stamped);
    if (
      !postIfCurrent({
        type: "discoverProgress",
        protocol,
        label,
        status: "done",
        count: stamped.length,
      })
    ) {
      return;
    }
  }

  postIfCurrent({ type: "discoverResults", candidates: all });
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
