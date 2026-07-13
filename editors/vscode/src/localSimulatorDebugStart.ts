import * as vscode from "vscode";

export type LocalSimulatorDebugStart = (
  attemptId: string,
  workspaceFolder: vscode.Uri | undefined,
) => Thenable<boolean | undefined>;

/** Starts one lifecycle-owned Simulator debug session. */
export async function startLocalSimulatorDebugSession(
  attemptId: string,
  workspaceFolder: vscode.Uri | undefined,
): Promise<boolean | undefined> {
  return vscode.commands.executeCommand<boolean>("trust-lsp.debug.start", {
    lifecycleAttemptId: attemptId,
    workspaceFolder,
  });
}
