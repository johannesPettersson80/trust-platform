import * as vscode from "vscode";

import { getTrustConfiguration } from "../configuration";
import { setControlAuthToken } from "../runtimeAuth";
import { presentCanvasReply, requestCanvasAct } from "./canvasCommands";
import { offlineFleetRuntimeAdd } from "./offlineComm";

export interface FleetActionDependencies {
  readonly extensionContext: () => vscode.ExtensionContext | undefined;
  readonly endpointLabels: Map<string, string>;
  readonly focusEndpoint: (nodeId: string) => void;
  readonly refresh: () => Promise<void>;
}

export class NetworkCanvasFleetActions {
  constructor(private readonly dependencies: FleetActionDependencies) {}

  /**
   * `Add runtime` of `Connect existing runtime`: `trust-lsp.devicesCanvasAct` normalizes the typed
   * address and answers the fleet's endpoints with it (specification 25 §5.2); this stores the
   * typed token and label for the endpoint, focuses the runtime the answer names, and writes the
   * fleet setting.
   */
  async addHost(message: Record<string, unknown>): Promise<void> {
    const authToken =
      typeof message.authToken === "string" ? message.authToken.trim() : "";
    const label =
      typeof message.label === "string" ? message.label.trim() : "";
    const config = trustConfig();
    const reply = await requestCanvasAct("add_host", {
      endpoint: typeof message.endpoint === "string" ? message.endpoint : "",
      fleet_endpoints: config.get<string[]>("runtime.fleetEndpoints", []) ?? [],
      token: authToken.length > 0,
    });
    const added = isRecord(reply.result) ? reply.result : undefined;
    const endpoint = typeof added?.endpoint === "string" ? added.endpoint : undefined;
    if (endpoint) {
      if (authToken) {
        await setControlAuthToken(endpoint, authToken);
      }
      if (label) {
        this.dependencies.endpointLabels.set(endpoint, label);
      }
      if (typeof added?.focus === "string") {
        this.dependencies.focusEndpoint(added.focus);
      }
      if (added?.already !== true && Array.isArray(added?.fleet_endpoints)) {
        await config.update(
          "runtime.fleetEndpoints",
          added.fleet_endpoints,
          configurationTarget()
        );
      }
    }
    await presentCanvasReply(reply);
    if (reply.refresh) {
      await this.dependencies.refresh();
    }
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

function trustConfig(): vscode.WorkspaceConfiguration {
  return getTrustConfiguration(vscode.workspace.workspaceFolders?.[0]?.uri);
}

function configurationTarget(): vscode.ConfigurationTarget {
  return vscode.workspace.workspaceFolders?.length
    ? vscode.ConfigurationTarget.Workspace
    : vscode.ConfigurationTarget.Global;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
