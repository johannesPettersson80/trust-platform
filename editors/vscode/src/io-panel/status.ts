import * as net from "net";
import * as vscode from "vscode";

import {
  isLocalControlEndpoint,
  parseControlEndpoint,
} from "../runtimeControl";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  summarizeAdsStatus,
  type AdsStatusReport,
  type AdsStatusSummary,
} from "../adsStatusSummary";
import { RuntimeStatusPayload } from "./types";

const ENDPOINT_PROBE_TTL_MS = 2000;
const ENDPOINT_PROBE_TIMEOUT_MS = 400;

let endpointProbeCache:
  | { endpoint: string; reachable: boolean; checkedAt: number }
  | undefined;

export function isLocalEndpoint(endpoint: string): boolean {
  return isLocalControlEndpoint(endpoint);
}

// One fresh reachability probe (no cache): does the control endpoint accept a connection right now?
async function probeEndpointOnce(endpoint: string): Promise<boolean> {
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    return false;
  }
  return new Promise<boolean>((resolve) => {
    let settled = false;
    const socket =
      parsed.kind === "tcp"
        ? net.createConnection({ host: parsed.host, port: parsed.port })
        : net.createConnection({ path: parsed.path });
    const finish = (value: boolean) => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      resolve(value);
    };
    socket.setTimeout(ENDPOINT_PROBE_TIMEOUT_MS, () => finish(false));
    socket.once("error", () => finish(false));
    socket.once("connect", () => finish(true));
  });
}

export async function probeEndpointReachable(
  endpoint: string
): Promise<boolean> {
  const now = Date.now();
  if (
    endpointProbeCache &&
    endpointProbeCache.endpoint === endpoint &&
    now - endpointProbeCache.checkedAt < ENDPOINT_PROBE_TTL_MS
  ) {
    return endpointProbeCache.reachable;
  }
  const reachable = await probeEndpointOnce(endpoint);
  endpointProbeCache = { endpoint, reachable, checkedAt: Date.now() };
  return reachable;
}

// Poll a control endpoint until it accepts connections (or the budget runs out), bypassing the short
// reachability cache so a freshly-started runtime is not pinned to a stale `false`. Used right after a
// managed Start: the runtime process is up but its control socket may need a beat to bind, and reporting
// "Live Values could not connect" in that window is a false failure on the happy path (F-11). On success
// the cache is primed `true` so the immediately-following attach probe sees it. Returns false honestly if
// the endpoint never becomes reachable within the budget — a genuine failure still surfaces.
export async function waitForEndpointReachable(
  endpoint: string,
  totalMs = 6000,
  intervalMs = 250
): Promise<boolean> {
  const deadline = Date.now() + totalMs;
  let reachable = await probeEndpointOnce(endpoint);
  while (!reachable && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
    reachable = await probeEndpointOnce(endpoint);
  }
  if (reachable) {
    endpointProbeCache = { endpoint, reachable: true, checkedAt: Date.now() };
  }
  return reachable;
}

export async function fetchRuntimeState(
  endpoint: string,
  authToken?: string
): Promise<"running" | "stopped" | undefined> {
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    return undefined;
  }
  return new Promise((resolve) => {
    let settled = false;
    let buffer = "";
    const socket =
      parsed.kind === "tcp"
        ? net.createConnection({ host: parsed.host, port: parsed.port })
        : net.createConnection({ path: parsed.path });
    const finish = (value: "running" | "stopped" | undefined) => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      resolve(value);
    };
    socket.setTimeout(ENDPOINT_PROBE_TIMEOUT_MS, () => finish(undefined));
    socket.once("error", () => finish(undefined));
    socket.once("connect", () => {
      const request = { id: 1, type: "status", auth: authToken || undefined };
      socket.write(JSON.stringify(request) + "\n");
    });
    socket.on("data", (chunk: Buffer | string) => {
      buffer += chunk.toString();
      const idx = buffer.indexOf("\n");
      if (idx == -1) {
        return;
      }
      const line = buffer.slice(0, idx).trim();
      if (!line) {
        finish(undefined);
        return;
      }
      try {
        const response = JSON.parse(line) as {
          ok?: boolean;
          result?: { state?: string };
        };
        if (
          response.ok &&
          response.result &&
          typeof response.result.state === "string"
        ) {
          const state = response.result.state.toLowerCase();
          finish(state === "running" ? "running" : "stopped");
          return;
        }
      } catch {
        // ignore parse errors
      }
      finish(undefined);
    });
  });
}

export async function fetchAdsStatusSummary(
  endpoint: string,
  authToken?: string
): Promise<AdsStatusSummary | undefined> {
  try {
    const report = await sendRuntimeControlRequest<AdsStatusReport>(
      endpoint,
      authToken,
      "ads.status",
      undefined,
      { timeoutMs: 750 }
    );
    return summarizeAdsStatus(report);
  } catch {
    return undefined;
  }
}

type RuntimeStatusDeps = {
  runtimeConfigTarget: () => vscode.Uri | undefined;
  getStructuredTextSession: () => vscode.DebugSession | undefined;
};

export async function runtimeStatusPayload(
  deps: RuntimeStatusDeps
): Promise<RuntimeStatusPayload> {
  const target = deps.runtimeConfigTarget();
  const config = vscode.workspace.getConfiguration("trust-lsp", target);
  const endpoint = (config.get<string>("runtime.controlEndpoint") ?? "").trim();
  const authToken = (config.get<string>("runtime.controlAuthToken") ?? "").trim();
  const endpointConfigured = endpoint.length > 0;
  const endpointEnabled = config.get<boolean>(
    "runtime.controlEndpointEnabled",
    true
  );
  const inlineValuesEnabled = config.get<boolean>(
    "runtime.inlineValuesEnabled",
    true
  );
  const runtimeMode = config.get<"simulate" | "online">(
    "runtime.mode",
    "simulate"
  );
  const session = deps.getStructuredTextSession();
  const running = !!session;
  let runtimeState: RuntimeStatusPayload["runtimeState"] = "stopped";
  let endpointReachable = false;
  let ads: AdsStatusSummary | undefined;

  if (running) {
    const request = session?.configuration?.request;
    runtimeState = request === "attach" ? "connected" : "running";
  }
  if (!running && runtimeMode === "online" && endpointConfigured && endpointEnabled) {
    endpointReachable = await probeEndpointReachable(endpoint);
    if (endpointReachable) {
      const state = await fetchRuntimeState(endpoint, authToken || undefined);
      if (state) {
        runtimeState = state;
      }
      ads = await fetchAdsStatusSummary(endpoint, authToken || undefined);
    }
  }

  return {
    running,
    inlineValuesEnabled,
    runtimeMode,
    runtimeState,
    endpoint,
    endpointConfigured,
    endpointEnabled,
    endpointReachable,
    ads,
  };
}
