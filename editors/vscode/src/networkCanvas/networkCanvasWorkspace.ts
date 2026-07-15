import * as vscode from "vscode";

import { getTrustConfiguration } from "../configuration";

export function workspaceConfigResource(): vscode.Uri | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri;
}

export function networkCanvasTrustConfig(): vscode.WorkspaceConfiguration {
  return getTrustConfiguration(workspaceConfigResource());
}
