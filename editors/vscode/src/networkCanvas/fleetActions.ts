import * as vscode from "vscode";

import { getTrustConfiguration } from "../configuration";
import { setControlAuthToken } from "../runtimeAuth";
import { offlineFleetRuntimeAdd } from "./offlineComm";

export interface FleetActionDependencies {
  readonly extensionContext: () => vscode.ExtensionContext | undefined;
  readonly endpointLabels: Map<string, string>;
  readonly focusEndpoint: (nodeId: string) => void;
  readonly refresh: () => Promise<void>;
}

export class NetworkCanvasFleetActions {
  constructor(private readonly dependencies: FleetActionDependencies) {}

  async addHost(message: Record<string, unknown>): Promise<void> {
    const endpoint = normalizeFleetControlEndpoint(
      typeof message.endpoint === "string" ? message.endpoint : ""
    );
    if (!endpoint) {
      return;
    }
    const authToken =
      typeof message.authToken === "string" ? message.authToken.trim() : "";
    const label =
      typeof message.label === "string" ? message.label.trim() : "";
    if (authToken) {
      await setControlAuthToken(endpoint, authToken);
    }
    if (label) {
      this.dependencies.endpointLabels.set(endpoint, label);
    }
    if (!authToken) {
      this.dependencies.focusEndpoint(`fleet:${endpoint}:runtime`);
    }
    const config = trustConfig();
    const current = config.get<string[]>("runtime.fleetEndpoints", []) ?? [];
    if (current.includes(endpoint)) {
      await vscode.window.showInformationMessage(
        `${endpoint} is already in the fleet.`
      );
      await this.dependencies.refresh();
      return;
    }
    await config.update(
      "runtime.fleetEndpoints",
      [...current, endpoint],
      configurationTarget()
    );
    await this.dependencies.refresh();
  }

  async addRuntime(message: Record<string, unknown>): Promise<void> {
    const name = typeof message.name === "string" ? message.name.trim() : "";
    const template = message.template === "empty" ? "empty" : "simulate";
    const context = this.dependencies.extensionContext();
    if (!name || !context) {
      return;
    }
    const fleetRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (!fleetRoot) {
      await vscode.window.showWarningMessage(
        "Open a workspace folder to add a runtime."
      );
      return;
    }
    const result = await offlineFleetRuntimeAdd(
      context,
      fleetRoot,
      name,
      template
    );
    if (!result) {
      await vscode.window.showWarningMessage(
        `Could not create runtime "${name}" (it may already exist, or needs a newer trust-runtime).`
      );
      return;
    }
    const config = trustConfig();
    const current = config.get<string[]>("runtime.fleetEndpoints", []) ?? [];
    if (!current.includes(result.control_endpoint)) {
      await config.update(
        "runtime.fleetEndpoints",
        [...current, result.control_endpoint],
        configurationTarget()
      );
    }
    await vscode.window.showInformationMessage(
      `Created runtime "${result.name}" at ${result.path} (${result.control_endpoint}). Start it to see it on the canvas.`
    );
    await this.dependencies.refresh();
  }
}

export function normalizeFleetControlEndpoint(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (
    trimmed.startsWith("tcp://") ||
    trimmed.startsWith("unix://") ||
    trimmed.length === 0
  ) {
    return trimmed;
  }
  if (/^[^/\s:]+:\d+$/.test(trimmed) || /^\[[^\]]+\]:\d+$/.test(trimmed)) {
    return `tcp://${trimmed}`;
  }
  return trimmed;
}

function trustConfig(): vscode.WorkspaceConfiguration {
  return getTrustConfiguration(vscode.workspace.workspaceFolders?.[0]?.uri);
}

function configurationTarget(): vscode.ConfigurationTarget {
  return vscode.workspace.workspaceFolders?.length
    ? vscode.ConfigurationTarget.Workspace
    : vscode.ConfigurationTarget.Global;
}
