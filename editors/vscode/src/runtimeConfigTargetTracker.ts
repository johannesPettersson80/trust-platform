import * as vscode from "vscode";

/** Keeps runtime settings scoped to the workspace that owns the active run. */
export class RuntimeConfigTargetTracker {
  private lastTarget: vscode.Uri | undefined;

  capture(editor: vscode.TextEditor | undefined): void {
    if (!editor) {
      return;
    }
    const folder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
    if (folder) {
      this.lastTarget = folder.uri;
    }
  }

  target(activeSession: vscode.DebugSession | undefined): vscode.Uri | undefined {
    if (activeSession?.workspaceFolder) {
      this.lastTarget = activeSession.workspaceFolder.uri;
      return activeSession.workspaceFolder.uri;
    }
    const editor = vscode.window.activeTextEditor;
    if (editor) {
      const folder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
      if (folder) {
        this.lastTarget = folder.uri;
        return folder.uri;
      }
    }
    if (
      this.lastTarget &&
      vscode.workspace.workspaceFolders?.some(
        (folder) => folder.uri.toString() === this.lastTarget?.toString(),
      )
    ) {
      return this.lastTarget;
    }
    return vscode.workspace.workspaceFolders?.[0]?.uri;
  }

  scope(target: vscode.Uri | undefined): vscode.ConfigurationTarget {
    return target
      ? vscode.ConfigurationTarget.WorkspaceFolder
      : vscode.ConfigurationTarget.Workspace;
  }
}
