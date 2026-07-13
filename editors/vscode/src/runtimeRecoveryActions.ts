import * as vscode from "vscode";

import { debugChannel } from "./debug/configuration";
import { runtimeLifecycleService } from "./runtimeLifecycle";
import { findRuntimeControlToml } from "./windowsRuntimeControlMigration";

export async function openSelectedRuntimeToml(): Promise<void> {
  const projectRoot = runtimeLifecycleService.runtimeConfigTarget()?.fsPath;
  const runtimeToml = projectRoot
    ? findRuntimeControlToml(projectRoot)
    : undefined;
  if (!runtimeToml) {
    void vscode.window.showWarningMessage(
      "runtime.toml was not found in the selected simulator project."
    );
    return;
  }
  try {
    const document = await vscode.workspace.openTextDocument(
      vscode.Uri.file(runtimeToml)
    );
    await vscode.window.showTextDocument(document, { preview: false });
  } catch {
    void vscode.window.showErrorMessage(
      "runtime.toml could not be opened. Check the workspace permissions and logs."
    );
  }
}

export function openStructuredTextDebuggerLogs(): void {
  debugChannel().show(true);
}
