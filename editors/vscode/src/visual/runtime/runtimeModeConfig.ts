import { getTrustConfiguration } from "../../configuration";
import * as vscode from "vscode";
import type { RuntimeUiMode } from "./runtimeTypes";

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

export function runtimeUiModeFromConfig(resource: vscode.Uri): RuntimeUiMode {
  const { section } = runtimeScope(resource);
  const mode = section.get<"simulate" | "online">("runtime.mode", "simulate");
  return mode === "online" ? "external" : "local";
}

export async function persistRuntimeUiMode(
  resource: vscode.Uri,
  mode: RuntimeUiMode
): Promise<void> {
  const { section, target } = runtimeScope(resource);
  await section.update(
    "runtime.mode",
    mode === "external" ? "online" : "simulate",
    target
  );
}
