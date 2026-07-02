import * as net from "net";
import * as vscode from "vscode";

import {
  isLocalControlEndpoint,
  parseControlEndpoint,
} from "../runtimeControl";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import { getControlAuthToken } from "../runtimeAuth";
import {
  summarizeAdsStatus,
  type AdsStatusReport,
  type AdsStatusSummary,
} from "../adsStatusSummary";
import type { RuntimeAccessPayload, RuntimeStatusPayload } from "./types";

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
  const report = await fetchRuntimeStatusReport(endpoint, authToken);
  const state = typeof report?.state === "string" ? report.state.toLowerCase() : "";
  if (state === "running") {
    return "running";
  }
  if (state === "stopped" || state === "ready") {
    return "stopped";
  }
  return undefined;
}

type RuntimeControlStatusReport = {
  state?: unknown;
  access?: unknown;
};

async function fetchRuntimeStatusReport(
  endpoint: string,
  authToken?: string
): Promise<RuntimeControlStatusReport | undefined> {
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    return undefined;
  }
  try {
    return await sendRuntimeControlRequest<RuntimeControlStatusReport>(
      endpoint,
      authToken,
      "status",
      undefined,
      { timeoutMs: 750 }
    );
  } catch {
    return undefined;
  }
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
  let endpoint = (config.get<string>("runtime.controlEndpoint") ?? "").trim();
  const authToken = await getControlAuthToken(endpoint);
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
  let targetLabel: string | undefined;
  let endpointReachable = false;
  let access: RuntimeAccessPayload | undefined;
  let ads: AdsStatusSummary | undefined;
  let statusEndpoint = endpoint;
  let statusAuthToken = authToken ?? undefined;

  if (running) {
    const request = session?.configuration?.request;
    const configuredLabel = session?.configuration?.targetLabel;
    if (typeof configuredLabel === "string" && configuredLabel.trim()) {
      targetLabel = configuredLabel.trim();
    }
    runtimeState = request === "attach" ? "connected" : "running";
    if (
      request === "launch" &&
      typeof session?.configuration?.controlEndpoint === "string" &&
      session.configuration.controlEndpoint.trim()
    ) {
      statusEndpoint = session.configuration.controlEndpoint.trim();
      endpoint = statusEndpoint;
    }
    if (
      request === "launch" &&
      typeof session?.configuration?.controlAuthToken === "string" &&
      session.configuration.controlAuthToken.trim()
    ) {
      statusAuthToken = session.configuration.controlAuthToken.trim();
    }
    if (
      request === "attach" &&
      typeof session?.configuration?.endpoint === "string" &&
      session.configuration.endpoint.trim()
    ) {
      endpoint = session.configuration.endpoint.trim();
      statusEndpoint = endpoint;
    }
    if (
      request === "attach" &&
      typeof session?.configuration?.authToken === "string" &&
      session.configuration.authToken.trim()
    ) {
      statusAuthToken = session.configuration.authToken.trim();
    }
  }
  if (running && statusEndpoint) {
    const report = await fetchRuntimeStatusReport(statusEndpoint, statusAuthToken);
    access = normalizeRuntimeAccess(report?.access);
  }
  if (!running && runtimeMode === "online" && endpointConfigured && endpointEnabled) {
    endpointReachable = await probeEndpointReachable(endpoint);
    if (endpointReachable) {
      const report = await fetchRuntimeStatusReport(endpoint, authToken);
      const state =
        typeof report?.state === "string" ? report.state.toLowerCase() : "";
      if (state === "running") {
        runtimeState = "running";
      } else if (state === "stopped" || state === "ready") {
        runtimeState = "stopped";
      }
      access = normalizeRuntimeAccess(report?.access);
      ads = await fetchAdsStatusSummary(endpoint, authToken);
    }
  }
  if (!access) {
    access = defaultRuntimeAccess(runtimeMode, runtimeState);
  }

  return {
    running,
    inlineValuesEnabled,
    runtimeMode,
    runtimeState,
    targetLabel,
    endpoint,
    endpointConfigured,
    endpointEnabled,
    endpointReachable,
    access,
    ads,
  };
}

function normalizeRuntimeAccess(value: unknown): RuntimeAccessPayload | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  const io = isRecord(value.io) ? value.io : {};
  return {
    role: typeof value.role === "string" ? value.role : undefined,
    allowWrite: io.write === true,
    allowForce: io.force === true,
    allowRelease: io.release === true,
    reason: typeof value.reason === "string" ? value.reason : undefined,
  };
}

function defaultRuntimeAccess(
  runtimeMode: RuntimeStatusPayload["runtimeMode"],
  runtimeState: RuntimeStatusPayload["runtimeState"]
): RuntimeAccessPayload {
  if (runtimeMode === "simulate") {
    return {
      role: "admin",
      allowWrite: true,
      allowForce: true,
      allowRelease: true,
    };
  }
  const connected = runtimeState === "connected" || runtimeState === "running";
  return {
    allowWrite: false,
    allowForce: false,
    allowRelease: false,
    reason: connected
      ? "Write/force permissions are unknown — reconnect with an engineer token."
      : "Connect with an engineer token to write or force.",
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
