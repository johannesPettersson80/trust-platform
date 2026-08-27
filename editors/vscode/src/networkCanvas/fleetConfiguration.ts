import * as vscode from "vscode";

import { getTrustConfiguration } from "../configuration";
import { setControlAuthToken } from "../runtimeAuth";
import { offlineFleetRuntimeAdd } from "./offlineComm";

export interface AddFleetHostResult {
  focusNodeId?: string;
}

/** Persists user-authored fleet membership; it does not probe or render fleet state. */
export async function addFleetHost(
  message: Record<string, unknown>,
  labels: Map<string, string>
): Promise<AddFleetHostResult> {
  const endpoint = normalizeFleetControlEndpoint(
    typeof message.endpoint === "string" ? message.endpoint.trim() : ""
  );
  if (!endpoint) return {};
  const authToken = typeof message.authToken === "string" ? message.authToken.trim() : "";
  const label = typeof message.label === "string" ? message.label.trim() : "";
  if (authToken) await setControlAuthToken(endpoint, authToken);
  if (label) labels.set(endpoint, label);

  const config = trustConfig();
  const current = config.get<string[]>("runtime.fleetEndpoints", []) ?? [];
  if (current.includes(endpoint)) {
    await vscode.window.showInformationMessage(`${endpoint} is already in the fleet.`);
  } else {
    const target = vscode.workspace.workspaceFolders?.length
      ? vscode.ConfigurationTarget.Workspace
      : vscode.ConfigurationTarget.Global;
    await config.update("runtime.fleetEndpoints", [...current, endpoint], target);
  }
  return { focusNodeId: authToken ? undefined : `fleet:${endpoint}:runtime` };
}

/** Scaffolds a sibling runtime project and records its endpoint for later discovery. */
export async function addFleetRuntime(
  context: vscode.ExtensionContext | undefined,
  message: Record<string, unknown>
): Promise<void> {
  const name = typeof message.name === "string" ? message.name.trim() : "";
  const template = message.template === "empty" ? "empty" : "simulate";
  if (!name || !context) return;
  const fleetRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!fleetRoot) {
    await vscode.window.showWarningMessage("Open a workspace folder to add a runtime.");
    return;
  }
  const result = await offlineFleetRuntimeAdd(context, fleetRoot, name, template);
  if (!result) {
    await vscode.window.showWarningMessage(
      `Could not create runtime "${name}" (it may already exist, or needs a newer trust-runtime).`
    );
    return;
  }
  const config = trustConfig();
  const current = config.get<string[]>("runtime.fleetEndpoints", []) ?? [];
  if (!current.includes(result.control_endpoint)) {
    const target = vscode.workspace.workspaceFolders?.length
      ? vscode.ConfigurationTarget.Workspace
      : vscode.ConfigurationTarget.Global;
    await config.update("runtime.fleetEndpoints", [...current, result.control_endpoint], target);
  }
  await vscode.window.showInformationMessage(
    `Created runtime "${result.name}" at ${result.path} (${result.control_endpoint}). Start it to see it on the canvas.`
  );
}

function trustConfig(): vscode.WorkspaceConfiguration {
  return getTrustConfiguration(vscode.workspace.workspaceFolders?.[0]?.uri);
}

function normalizeFleetControlEndpoint(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (trimmed.startsWith("tcp://") || trimmed.startsWith("unix://") || !trimmed) return trimmed;
  if (/^[^/\s:]+:\d+$/.test(trimmed) || /^\[[^\]]+\]:\d+$/.test(trimmed)) {
    return `tcp://${trimmed}`;
  }
  return trimmed;
}
