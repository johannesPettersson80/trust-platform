import * as vscode from "vscode";

import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { RuntimeTarget } from "../runtimeTarget";
import type {
  DiscoveryRequestToken,
  DiscoveryRequestTracker,
} from "./discoverySession";
import { requestCanvasAct } from "./canvasCommands";
import {
  offlineCommDiscover,
  type DiscoverCandidate,
} from "./offlineComm";

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
  /**
   * The ADS ports that every ADS find is probed on (`trust.ads.discoveryPorts`); the search checks
   * them and answers the ports that respond (specification 11, "Communication discovery scope and
   * symbol browsing").
   */
  readonly adsPorts?: readonly unknown[];
  /** Receives the search's candidates once it has answered them. */
  readonly onCandidates?: (candidates: readonly DiscoverCandidate[]) => void;
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
  const { panel, extensionContext, runtimeTarget, tracker, token, adsPorts } = context;
  const { sessionId, requestId, request } = envelope;
  const viaRuntime =
    request.origin !== "this_host" &&
    runtimeTarget?.status === "online_reachable" &&
    Boolean(runtimeTarget.endpoint);
  const runtimeRequested = request.origin !== "this_host";
  const all: DiscoverCandidate[] = [];
  // The source row's text of an ADS search whose TwinCAT systems no port answered (server's).
  let adsError: string | undefined;

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
    const label = await discoveryText("discover_label", { protocol, host, cidr }, "label", protocol);
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
              ...(protocol === "ads" && adsPorts ? { ads_ports: adsPorts } : {}),
            },
          },
          { timeoutMs: 8000 }
        );
        candidates = response?.candidates ?? [];
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        postIfCurrent({
          type: "discoverResults",
          candidates: all,
          error: await discoveryText("discover_failure", { protocol, message }, "text", message),
        });
        return;
      }
    } else {
      const response = await offlineCommDiscover(
        extensionContext,
        protocol,
        "this-host",
        { cidr, host, adsPorts: protocol === "ads" ? adsPorts : undefined }
      );
      candidates = response?.candidates ?? [];
    }
    if (!tracker.isCurrent(token, panel)) {
      return;
    }
    let shown: DiscoverCandidate[] = candidates.map((candidate) => ({
      ...candidate,
      protocol,
      originRuntimeId: runtimeRequested ? request.origin : undefined,
    }));
    if (protocol === "ads") {
      // The cards leave out an ADS find that no port answered (specification 25 §2).
      const reply = await requestCanvasAct("ads_finds", {
        candidates: shown,
        ads_ports: adsPorts ?? [],
      });
      const answer = (reply.result ?? {}) as { candidates?: DiscoverCandidate[]; error?: string };
      shown = answer.candidates ?? [];
      adsError = answer.error ?? reply.notice?.text;
      if (!tracker.isCurrent(token, panel)) {
        return;
      }
    }
    all.push(...shown);
    if (
      !postIfCurrent({
        type: "discoverProgress",
        protocol,
        label,
        status: "done",
        count: shown.length,
      })
    ) {
      return;
    }
  }

  if (
    postIfCurrent({
      type: "discoverResults",
      candidates: all,
      error: adsError,
    })
  ) {
    context.onCandidates?.(all);
  }
}

/**
 * A discovery's words, as the server gives them (specification 25 §2): a progress row's label or the
 * text of a failure at the selected runtime. Without the server's answer, `fallback` shows.
 */
async function discoveryText(
  op: "discover_label" | "discover_failure",
  args: Record<string, unknown>,
  key: "label" | "text",
  fallback: string
): Promise<string> {
  const reply = await requestCanvasAct(op, args);
  const text = isRecord(reply.result) ? reply.result[key] : undefined;
  return typeof text === "string" ? text : fallback;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
