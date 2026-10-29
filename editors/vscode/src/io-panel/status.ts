import { getTrustConfiguration } from "../configuration";
import * as vscode from "vscode";

import { probeRuntimeControlEndpoint } from "../runtimeControlClient";
import { getControlAuthToken } from "../runtimeAuth";
import { resolveProjectEndpoint, resolveRuntimeConnection } from "../runtimeConnection";
import { projectRequest } from "../projectRequest";
import { summarizeAdsStatus, type AdsStatusReport } from "../adsStatusSummary";
import type { RuntimeStatusPayload } from "./types";

// What Live Values shows of its runtime is the language server's answer (`trust-lsp.runTargets`, action `liveValues`,
// specification 25 §4): the runtime's state, its endpoint and whether it answers, whether it is simulated, the operations
// its session may use and whether a Force must be armed first, and whether the project's own runtime answers, which the
// Target picker offers (§5.1). This module gathers the settings, the debug session's configuration and the project's
// endpoint, and forwards them.

/** Polls a control endpoint until it accepts connections or `totalMs` runs out (the language server tries every 250 ms),
 * as right after a managed Start, whose runtime binds its control socket a moment after its process is up (F-11). */
export async function waitForEndpointReachable(endpoint: string, totalMs = 6000): Promise<boolean> {
  return await probeRuntimeControlEndpoint(endpoint, { waitMs: totalMs });
}

type RuntimeStatusDeps = {
  runtimeConfigTarget: () => vscode.Uri | undefined;
  getStructuredTextSession: () => vscode.DebugSession | undefined;
};

export async function runtimeStatusPayload(
  deps: RuntimeStatusDeps
): Promise<RuntimeStatusPayload> {
  const target = deps.runtimeConfigTarget();
  const config = getTrustConfiguration(target);
  const runtimeMode = config.get<"simulate" | "online">("runtime.mode", "simulate");
  let endpoint: string;
  let authToken: string | undefined;
  if (runtimeMode === "online") {
    // Spec 11 §6.8.0a — the workspace runtime's endpoint and token are asked of trust-runtime at
    // each refresh, so Live Values follows a runtime that restarted on another port.
    const connection = await resolveRuntimeConnection(target);
    endpoint = connection.endpoint ?? "";
    authToken = connection.authToken;
  } else {
    endpoint = (config.get<string>("runtime.controlEndpoint") ?? "").trim();
    authToken = await getControlAuthToken(endpoint);
  }
  // The project's own endpoint is asked of trust-runtime at each refresh too, whatever the mode.
  const projectEndpoint = (await resolveProjectEndpoint(target)) ?? "";
  const endpointEnabled = config.get<boolean>("runtime.controlEndpointEnabled", true);
  const inlineValuesEnabled = config.get<boolean>("runtime.inlineValuesEnabled", true);
  const configuration = deps.getStructuredTextSession()?.configuration;
  const session = configuration
    ? {
        request: configuration.request,
        endpoint: configuration.endpoint,
        authToken: configuration.authToken,
        controlEndpoint: configuration.controlEndpoint,
        controlAuthToken: configuration.controlAuthToken,
        targetLabel: configuration.targetLabel,
      }
    : null;
  const answer = await projectRequest<{ status: RuntimeStatusPayload; adsReport: AdsStatusReport | null }>(
    "trust-lsp.runTargets",
    {
      action: "liveValues",
      runtimeMode,
      endpoint,
      authToken,
      endpointEnabled,
      inlineValuesEnabled,
      session,
      projectEndpoint,
    },
    { wait: false }
  );
  if (!answer.ok) {
    return {
      running: !!session,
      inlineValuesEnabled,
      runtimeMode,
      runtimeState: session ? (session.request === "attach" ? "connected" : "running") : "stopped",
      endpoint,
      endpointConfigured: endpoint.trim().length > 0,
      endpointEnabled,
      endpointReachable: false,
    };
  }
  const status = withoutNulls(answer.status);
  return answer.adsReport ? { ...status, ads: summarizeAdsStatus(answer.adsReport) } : status;
}

/** `value` without the members the language server sent as `null`, which the payload leaves out. */
function withoutNulls<T extends object>(value: T): T {
  return Object.fromEntries(Object.entries(value).filter(([, member]) => member !== null)) as T;
}
