import { getTrustConfiguration } from "../../configuration";
import * as vscode from "vscode";
import type { RuntimeUiMode, RuntimeUiState } from "./runtimeTypes";

export interface RuntimePanelIoEntry {
  name?: string;
  address: string;
  source?: string;
  writeTarget?: string;
  value: string;
  forced?: boolean;
  operationStatus?: "pending" | "error";
  operationError?: string;
}

export interface RuntimePanelIoState {
  inputs: RuntimePanelIoEntry[];
  outputs: RuntimePanelIoEntry[];
  memory: RuntimePanelIoEntry[];
}

export interface RuntimePanelStatusPayload {
  running: boolean;
  inlineValuesEnabled: boolean;
  runtimeMode: "simulate" | "online";
  runtimeState: "running" | "connected" | "stopped";
  endpoint: string;
  endpointConfigured: boolean;
  endpointEnabled: boolean;
  endpointReachable: boolean;
}

export interface RuntimePanelSettingsPayload {
  serverPath?: string;
  traceServer?: string;
  debugAdapterPath?: string;
  debugAdapterArgs?: string[];
  debugAdapterEnv?: Record<string, string>;
  runtimeControlEndpoint?: string;
  runtimeControlAuthToken?: string;
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
  runtimeInlineValuesEnabled?: boolean;
}

export type RuntimePanelWebviewMessage =
  | { type: "runtimeStart" }
  | { type: "runtimeSetMode"; mode: "simulate" | "online" }
  | { type: "requestSettings" }
  | { type: "saveSettings"; payload?: RuntimePanelSettingsPayload }
  | { type: "writeInput"; address?: string; value?: string }
  | { type: "forceInput"; address?: string; value?: string }
  | { type: "releaseInput"; address?: string }
  | { type: "webviewReady" };

function runtimeScope(resource: vscode.Uri): {
  section: vscode.WorkspaceConfiguration;
  target: vscode.ConfigurationTarget;
} {
  const folder = vscode.workspace.getWorkspaceFolder(resource);
  if (folder) {
    return {
      section: getTrustConfiguration(folder.uri),
      target: vscode.ConfigurationTarget.WorkspaceFolder,
    };
  }

  return {
    section: getTrustConfiguration(),
    target: vscode.ConfigurationTarget.Workspace,
  };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function isRuntimePanelWebviewMessage(
  value: unknown
): value is RuntimePanelWebviewMessage {
  if (!isObject(value) || typeof value.type !== "string") {
    return false;
  }

  if (
    value.type === "runtimeStart" ||
    value.type === "requestSettings" ||
    value.type === "webviewReady"
  ) {
    return true;
  }

  if (value.type === "runtimeSetMode") {
    return value.mode === "simulate" || value.mode === "online";
  }

  if (value.type === "saveSettings") {
    return value.payload === undefined || isObject(value.payload);
  }

  if (value.type === "writeInput" || value.type === "forceInput") {
    return typeof value.address === "string" && typeof value.value === "string";
  }

  if (value.type === "releaseInput") {
    return typeof value.address === "string";
  }

  return false;
}

export function runtimePanelModeToUi(mode: "simulate" | "online"): RuntimeUiMode {
  return mode === "online" ? "external" : "local";
}

export function runtimeUiModeToPanel(mode: RuntimeUiMode): "simulate" | "online" {
  return mode === "external" ? "online" : "simulate";
}

export function collectRuntimePanelSettings(
  resource: vscode.Uri
): RuntimePanelSettingsPayload {
  const { section } = runtimeScope(resource);
  return {
    serverPath: section.get<string>("server.path") ?? "",
    traceServer: section.get<string>("trace.server") ?? "off",
    debugAdapterPath: section.get<string>("debug.adapter.path") ?? "",
    debugAdapterArgs: section.get<string[]>("debug.adapter.args") ?? [],
    debugAdapterEnv: section.get<Record<string, string>>("debug.adapter.env") ?? {},
    runtimeControlEndpoint: section.get<string>("runtime.controlEndpoint") ?? "",
    runtimeControlAuthToken: section.get<string>("runtime.controlAuthToken") ?? "",
    runtimeIncludeGlobs: section.get<string[]>("runtime.includeGlobs") ?? [],
    runtimeExcludeGlobs: section.get<string[]>("runtime.excludeGlobs") ?? [],
    runtimeIgnorePragmas: section.get<string[]>("runtime.ignorePragmas") ?? [],
    runtimeInlineValuesEnabled:
      section.get<boolean>("runtime.inlineValuesEnabled") ?? true,
  };
}

export async function applyRuntimePanelSettings(
  resource: vscode.Uri,
  payload: RuntimePanelSettingsPayload
): Promise<void> {
  const { section, target } = runtimeScope(resource);
  const updates: Array<{ key: string; value: unknown }> = [
    { key: "server.path", value: payload.serverPath?.trim() || undefined },
    { key: "trace.server", value: payload.traceServer?.trim() || "off" },
    {
      key: "debug.adapter.path",
      value: payload.debugAdapterPath?.trim() || undefined,
    },
    { key: "debug.adapter.args", value: payload.debugAdapterArgs ?? [] },
    { key: "debug.adapter.env", value: payload.debugAdapterEnv ?? {} },
    {
      key: "runtime.controlEndpoint",
      value: payload.runtimeControlEndpoint?.trim() || undefined,
    },
    {
      key: "runtime.controlAuthToken",
      value: payload.runtimeControlAuthToken?.trim() || undefined,
    },
    { key: "runtime.includeGlobs", value: payload.runtimeIncludeGlobs ?? [] },
    { key: "runtime.excludeGlobs", value: payload.runtimeExcludeGlobs ?? [] },
    { key: "runtime.ignorePragmas", value: payload.runtimeIgnorePragmas ?? [] },
    {
      key: "runtime.inlineValuesEnabled",
      value: payload.runtimeInlineValuesEnabled ?? true,
    },
  ];

  for (const update of updates) {
    await section.update(update.key, update.value, target);
  }
}

export function runtimePanelStatusFromState(
  resource: vscode.Uri,
  state: RuntimeUiState
): RuntimePanelStatusPayload {
  const { section } = runtimeScope(resource);
  const endpoint = (section.get<string>("runtime.controlEndpoint") ?? "").trim();
  const endpointEnabled = section.get<boolean>(
    "runtime.controlEndpointEnabled",
    true
  );
  const inlineValuesEnabled = section.get<boolean>(
    "runtime.inlineValuesEnabled",
    true
  );

  const running = state.isExecuting;
  let runtimeState: RuntimePanelStatusPayload["runtimeState"] = "stopped";
  if (running) {
    runtimeState = state.mode === "external" ? "connected" : "running";
  }

  return {
    running,
    inlineValuesEnabled,
    runtimeMode: runtimeUiModeToPanel(state.mode),
    runtimeState,
    endpoint,
    endpointConfigured: endpoint.length > 0,
    endpointEnabled,
    endpointReachable: false,
  };
}

export function boolToDisplay(value: boolean): string {
  return `BOOL(${value ? "TRUE" : "FALSE"})`;
}
