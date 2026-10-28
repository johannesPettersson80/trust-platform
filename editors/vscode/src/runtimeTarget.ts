import { getTrustConfiguration } from "./configuration";
import * as vscode from "vscode";

import { getControlAuthToken } from "./runtimeAuth";
import { resolveRuntimeConnection } from "./runtimeConnection";
import { projectRequest } from "./projectRequest";

// The configured run target's status is the language server's answer (`trust-lsp.runTargets`, action `target`,
// specification 25 §5.1): it probes the endpoint, asks `status` with the token and names the result and the credential
// channel. This module gathers the settings and forwards them.

export const RUNTIME_PANEL_COMMAND = "trust-lsp.debug.openIoPanel";

export type RuntimeTargetStatus =
  | "simulate"
  | "online_reachable"
  | "online_unreachable"
  | "missing_endpoint"
  | "auth_failed";

export type RuntimeTargetMode = "simulate" | "online";

export type RuntimeCredentialChannel =
  | "trusted_same_host"
  | "untrusted_remote_plain_tcp"
  | "unavailable";

export interface RuntimeTarget {
  mode: RuntimeTargetMode;
  endpoint?: string;
  authToken?: string;
  endpointEnabled: boolean;
  reachable: boolean;
  status: RuntimeTargetStatus;
  authFailureKind?: "missing" | "rejected";
  label: string;
  setupUrl?: string;
  credentialChannel: RuntimeCredentialChannel;
}

export interface RuntimeTargetSettings {
  mode?: RuntimeTargetMode;
  endpoint?: string;
  authToken?: string;
  endpointEnabled?: boolean;
  label?: string;
  setupUrl?: string;
  /** trust-runtime's reason why the workspace runtime has no endpoint now (spec 11 §6.8.0a). */
  unavailableReason?: string;
}

export async function resolveRuntimeTarget(resource?: vscode.Uri): Promise<RuntimeTarget> {
  const config = getTrustConfiguration(resource);
  const mode = config.get<RuntimeTargetMode>("runtime.mode", "simulate");
  let endpoint: string;
  let authToken: string | undefined;
  let unavailableReason: string | undefined;
  if (mode === "online") {
    // Spec 11 §6.8.0a — the workspace runtime's endpoint and token are asked of trust-runtime at
    // each resolution, so a runtime that restarted on another port is followed.
    const connection = await resolveRuntimeConnection(resource);
    endpoint = connection.endpoint ?? "";
    authToken = connection.authToken;
    unavailableReason = connection.problem?.message;
  } else {
    endpoint = config.get<string>("runtime.controlEndpoint", "");
    // §0.6.8 — token from SecretStorage first, legacy plaintext setting only as fallback.
    authToken = await getControlAuthToken(endpoint);
  }
  return await resolveRuntimeTargetFromSettings({
    mode,
    endpoint,
    authToken,
    endpointEnabled: config.get<boolean>("runtime.controlEndpointEnabled", true),
    setupUrl: config.get<string>("runtime.setupUrl", ""),
    unavailableReason,
  });
}

/** The target of `settings`, as the language server finds it. Without the server's answer the target is not reachable. */
export async function resolveRuntimeTargetFromSettings(
  settings: RuntimeTargetSettings
): Promise<RuntimeTarget> {
  const answer = await projectRequest<{ target: Omit<RuntimeTarget, "authToken"> }>("trust-lsp.runTargets", {
    action: "target",
    ...settings,
  });
  const authToken = settings.authToken?.trim() || undefined;
  if (answer.ok) {
    return withoutNulls({ ...answer.target, authToken });
  }
  const online = settings.mode === "online";
  return {
    mode: online ? "online" : "simulate",
    endpoint: settings.endpoint?.trim() || undefined,
    authToken,
    endpointEnabled: settings.endpointEnabled ?? true,
    reachable: false,
    status: online ? "online_unreachable" : "simulate",
    label: settings.label ?? (online ? "Online runtime" : "Simulator"),
    setupUrl: settings.setupUrl?.trim() || undefined,
    credentialChannel: "unavailable",
  };
}

export async function openRuntimePane(): Promise<unknown> {
  return await vscode.commands.executeCommand(RUNTIME_PANEL_COMMAND);
}

/** `value` without the members the language server sent as `null`, which the target leaves out. */
function withoutNulls<T extends object>(value: T): T {
  return Object.fromEntries(Object.entries(value).filter(([, member]) => member !== null)) as T;
}
