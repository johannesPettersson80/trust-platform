import * as vscode from "vscode";

import { FOCUS_MAIN_KEY } from "./newProject";
import {
  exampleQuickPickItems,
  parseManifest,
  type ExampleEntry,
} from "./examples/model";

// §0.5.12 — "Start from example": pick a bundled starter (with a hardware-requirement badge) → copy an
// editable working copy to a user-chosen folder → open it (focus Main.st). The user never hand-edits TOML
// to start; a "No hardware" starter is immediately runnable in the Simulator.

export const START_FROM_EXAMPLE_COMMAND = "trust.examples.start";

export function registerExamples(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(START_FROM_EXAMPLE_COMMAND, () =>
      startFromExample(context)
    )
  );
}

async function loadManifest(
  context: vscode.ExtensionContext
): Promise<ExampleEntry[]> {
  const uri = vscode.Uri.joinPath(
    context.extensionUri,
    "media",
    "examples",
    "manifest.json"
  );
  const data = await vscode.workspace.fs.readFile(uri);
  return parseManifest(JSON.parse(Buffer.from(data).toString("utf8")));
}

async function startFromExample(context: vscode.ExtensionContext): Promise<void> {
  let entries: ExampleEntry[];
  try {
    entries = await loadManifest(context);
  } catch (error) {
    void vscode.window.showErrorMessage(
      `Could not load the examples list: ${String(error)}`
    );
    return;
  }

  const items = exampleQuickPickItems(entries);
  const pick = await vscode.window.showQuickPick(items, {
    title: "truST — Start from example",
    placeHolder: "Choose a starter project",
    matchOnDescription: true,
    matchOnDetail: true,
  });
  if (!pick) {
    return;
  }

  const baseSelection = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Select destination folder",
  });
  const base = baseSelection?.[0];
  if (!base) {
    return;
  }

  const name = await vscode.window.showInputBox({
    prompt: "New project folder name",
    value: pick.id,
    validateInput: (value) => {
      const trimmed = value.trim();
      if (!trimmed) {
        return "A folder name is required.";
      }
      if (trimmed.includes("/") || trimmed.includes("\\")) {
        return "The name must not contain path separators.";
      }
      return undefined;
    },
  });
  if (!name) {
    return;
  }

  const source = vscode.Uri.joinPath(
    context.extensionUri,
    "media",
    "examples",
    pick.path
  );
  const dest = vscode.Uri.joinPath(base, name.trim());

  if (await pathExists(dest)) {
    const choice = await vscode.window.showWarningMessage(
      `${dest.fsPath} already exists. Overwrite its contents with the example?`,
      { modal: true },
      "Overwrite"
    );
    if (choice !== "Overwrite") {
      return;
    }
  }

  try {
    await vscode.workspace.fs.copy(source, dest, { overwrite: true });
  } catch (error) {
    void vscode.window.showErrorMessage(
      `Could not copy the example: ${String(error)}`
    );
    return;
  }

  // Focus Main.st after the window reloads (same mechanism as Create project).
  await context.globalState.update(
    FOCUS_MAIN_KEY,
    vscode.Uri.joinPath(dest, "src", "Main.st").fsPath
  );
  await vscode.commands.executeCommand("vscode.openFolder", dest, false);
}

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}
