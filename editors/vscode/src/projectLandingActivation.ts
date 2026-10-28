import * as vscode from "vscode";
import { consumeProjectLanding } from "./projectLanding";

/** Present recovery at the activation boundary; retry requires an explicit user choice. */
export async function openPendingProjectLanding(context: vscode.ExtensionContext): Promise<void> {
  for (;;) {
    const result = await consumeProjectLanding(context);
    if (result.kind === "cleared") {
      await vscode.window.showWarningMessage(result.message);
      return;
    }
    if (result.kind !== "failed") return;
    const choice = await vscode.window.showWarningMessage(result.message, "Retry");
    if (choice !== "Retry") return;
  }
}
