import * as vscode from "vscode";

/** Open an already-created project. Recovery never repeats creation or changes its files. */
export async function openCreatedProjectFolder(destination: vscode.Uri): Promise<boolean> {
  return openProjectFolder(destination,
    `Project created at ${destination.fsPath}, but VS Code could not open it. Open that folder to continue; your files are preserved.`);
}

/** Open a retained project without implying that it was just created. */
export async function openExistingProjectFolder(destination: vscode.Uri): Promise<boolean> {
  return openProjectFolder(destination,
    `Could not open existing project at ${destination.fsPath}. Your files are preserved; retry opening that folder or cancel.`);
}

async function openProjectFolder(destination: vscode.Uri, failureMessage: string): Promise<boolean> {
  for (;;) {
    try {
      await vscode.commands.executeCommand("vscode.openFolder", destination, false);
      return true;
    } catch {
      const choice = await vscode.window.showErrorMessage(
        failureMessage,
        "Retry opening"
      );
      if (choice !== "Retry opening") return false;
    }
  }
}
