import * as vscode from "vscode";

import {
  isLocalControlEndpoint,
  parseControlEndpoint,
} from "./runtimeControl";
import {
  isRuntimeControlAuthError,
  probeRuntimeControlEndpoint,
  requestRuntimeStatus,
} from "./runtimeControlClient";
import { getControlAuthToken } from "./runtimeAuth";

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
}

export interface RuntimeTargetDeps {
  probeEndpoint?: (endpoint: string) => Promise<boolean>;
  requestStatus?: (
    endpoint: string,
    authToken: string | undefined
  ) => Promise<unknown>;
}

export async function resolveRuntimeTarget(
  resource?: vscode.Uri,
  deps: RuntimeTargetDeps = {}
): Promise<RuntimeTarget> {
  const config = vscode.workspace.getConfiguration("trust-lsp", resource);
  const endpoint = config.get<string>("runtime.controlEndpoint", "");
  return await resolveRuntimeTargetFromSettings(
    {
      mode: config.get<RuntimeTargetMode>("runtime.mode", "simulate"),
      endpoint,
      // §0.6.8 — token from SecretStorage first, legacy plaintext setting only as fallback.
      authToken: await getControlAuthToken(endpoint),
      endpointEnabled: config.get<boolean>(
        "runtime.controlEndpointEnabled",
        true
      ),
      setupUrl: config.get<string>("runtime.setupUrl", ""),
    },
    deps
  );
}

export async function resolveRuntimeTargetFromSettings(
  settings: RuntimeTargetSettings,
  deps: RuntimeTargetDeps = {}
): Promise<RuntimeTarget> {
  const mode = settings.mode === "online" ? "online" : "simulate";
  const endpoint = (settings.endpoint ?? "").trim();
  const endpointEnabled = settings.endpointEnabled ?? true;
  const authToken = normalizedOptional(settings.authToken);
  const activeEndpoint = endpointEnabled ? endpoint : "";
  const credentialChannel = classifyRuntimeCredentialChannel(activeEndpoint);

  if (mode === "simulate") {
    return {
      mode,
      endpoint: normalizedOptional(endpoint),
      authToken,
      endpointEnabled,
      reachable: false,
      status: "simulate",
      label: settings.label ?? "Simulated runtime",
      setupUrl: normalizedOptional(settings.setupUrl),
      credentialChannel,
    };
  }

  if (activeEndpoint.length === 0) {
    return {
      mode,
      endpoint: normalizedOptional(endpoint),
      authToken,
      endpointEnabled,
      reachable: false,
      status: "missing_endpoint",
      label: settings.label ?? "Online runtime",
      setupUrl: normalizedOptional(settings.setupUrl),
      credentialChannel,
    };
  }

  if (!parseControlEndpoint(activeEndpoint)) {
    return onlineTarget(
      settings,
      authToken,
      credentialChannel,
      false,
      "online_unreachable"
    );
  }

  const probeEndpoint = deps.probeEndpoint ?? probeRuntimeControlEndpoint;
  const reachable = await probeEndpoint(activeEndpoint);
  if (!reachable) {
    return onlineTarget(
      settings,
      authToken,
      credentialChannel,
      false,
      "online_unreachable"
    );
  }

  const requestStatus =
    deps.requestStatus ??
    ((targetEndpoint, targetAuthToken) =>
      requestRuntimeStatus(targetEndpoint, targetAuthToken, { timeoutMs: 750 }));
  try {
    await requestStatus(activeEndpoint, authToken);
  } catch (error) {
    if (isRuntimeControlAuthError(error)) {
      return onlineTarget(
        settings,
        authToken,
        credentialChannel,
        true,
        "auth_failed"
      );
    }
    return onlineTarget(
      settings,
      authToken,
      credentialChannel,
      false,
      "online_unreachable"
    );
  }

  return onlineTarget(
    settings,
    authToken,
    credentialChannel,
    true,
    "online_reachable"
  );
}

export function classifyRuntimeCredentialChannel(
  endpoint: string | undefined
): RuntimeCredentialChannel {
  const normalized = (endpoint ?? "").trim();
  if (normalized.length === 0) {
    return "unavailable";
  }
  const parsed = parseControlEndpoint(normalized);
  if (!parsed) {
    return "unavailable";
  }
  if (isLocalControlEndpoint(normalized)) {
    return "trusted_same_host";
  }
  if (parsed.kind === "tcp") {
    return "untrusted_remote_plain_tcp";
  }
  return "unavailable";
}

export async function openRuntimePane(): Promise<unknown> {
  return await vscode.commands.executeCommand(RUNTIME_PANEL_COMMAND);
}

function onlineTarget(
  settings: RuntimeTargetSettings,
  authToken: string | undefined,
  credentialChannel: RuntimeCredentialChannel,
  reachable: boolean,
  status: RuntimeTargetStatus
): RuntimeTarget {
  const endpoint = (settings.endpoint ?? "").trim();
  return {
    mode: "online",
    endpoint: endpoint.length > 0 ? endpoint : undefined,
    authToken,
    endpointEnabled: settings.endpointEnabled ?? true,
    reachable,
    status,
    label: settings.label ?? (endpoint || "Online runtime"),
    setupUrl: normalizedOptional(settings.setupUrl),
    credentialChannel,
  };
}

function normalizedOptional(value: string | undefined): string | undefined {
  const normalized = (value ?? "").trim();
  return normalized.length > 0 ? normalized : undefined;
}
