import * as vscode from "vscode";

import { FOCUS_MAIN_KEY } from "../newProject";
import type { ExampleEntry } from "./model";

// After Create project scaffolds a lesson + reloads the window (which reopens the folder),
// this globalState key tells the next activation which README to land the learner on (W-06:
// "the learner lands in an empty Explorer and must know to open README.md ... the lesson text
// is the entire course"). Mirrors newProject.ts's FOCUS_MAIN_KEY reload handoff.
export const OPEN_LESSON_README_KEY = "trust.examples.openLessonReadme";

export async function copyExample(
  context: vscode.ExtensionContext,
  entry: ExampleEntry
): Promise<boolean> {
  const automation = readAcceptanceCopyOverride(entry);
  const base = automation?.base ?? (await promptExampleDestination());
  if (!base) {
    return false;
  }

  const name = automation?.name ?? (await promptExampleName(entry));
  if (!name) {
    return false;
  }

  const source = vscode.Uri.joinPath(
    context.extensionUri,
    "media",
    "examples",
    entry.path
  );
  const dest = vscode.Uri.joinPath(base, name.trim());

  if (await pathExists(dest)) {
    const choice = await vscode.window.showWarningMessage(
      `${dest.fsPath} already exists. Overwrite its contents with the example?`,
      { modal: true },
      "Overwrite"
    );
    if (choice !== "Overwrite") {
      return false;
    }
  }

  try {
    await vscode.workspace.fs.copy(source, dest, { overwrite: true });
  } catch (error) {
    void vscode.window.showErrorMessage(
      `Could not copy the example: ${String(error)}`
    );
    return false;
  }

  // Reuse Create project's reload handoff so copied projects open at Main.st.
  await context.globalState.update(
    FOCUS_MAIN_KEY,
    vscode.Uri.joinPath(dest, "src", "Main.st").fsPath
  );
  if (entry.lesson) {
    // Same reload handoff, for the lesson's README preview (W-06).
    await context.globalState.update(
      OPEN_LESSON_README_KEY,
      vscode.Uri.joinPath(dest, "README.md").fsPath
    );
  }
  if (automation?.openFolder === false) {
    return true;
  }
  await vscode.commands.executeCommand("vscode.openFolder", dest, false);
  return true;
}

/**
 * Consumes the reload handoff written above: after "Create project" reloads the window into
 * the new lesson folder, open its README in preview so the learner lands on the lesson text
 * instead of an empty Explorer (W-06). No-op when there is no pending README, it no longer
 * exists, or it does not belong to the workspace that just opened.
 */
export async function openPendingControlLessonReadme(
  context: vscode.ExtensionContext
): Promise<void> {
  const pending = context.globalState.get<string>(OPEN_LESSON_README_KEY);
  if (!pending) {
    return;
  }
  const uri = vscode.Uri.file(pending);
  try {
    await vscode.workspace.fs.stat(uri);
  } catch {
    await context.globalState.update(OPEN_LESSON_README_KEY, undefined);
    return;
  }
  if (!vscode.workspace.getWorkspaceFolder(uri)) {
    return;
  }
  await context.globalState.update(OPEN_LESSON_README_KEY, undefined);
  await vscode.commands.executeCommand("markdown.showPreview", uri);
}

async function promptExampleDestination(): Promise<vscode.Uri | undefined> {
  const selection = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Select destination folder",
  });
  return selection?.[0];
}

async function promptExampleName(entry: ExampleEntry): Promise<string | undefined> {
  return vscode.window.showInputBox({
    prompt: "New project folder name",
    value: entry.id,
    validateInput: validateExampleFolderName,
  });
}

function validateExampleFolderName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "A folder name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "The name must not contain path separators.";
  }
  return undefined;
}

function readAcceptanceCopyOverride(
  entry: ExampleEntry
): { base: vscode.Uri; name: string; openFolder: boolean } | undefined {
  const base = process.env.TRUST_UX_EXAMPLE_DESTINATION;
  if (!base) {
    return undefined;
  }
  const name = (process.env.TRUST_UX_EXAMPLE_NAME || entry.id).trim();
  const validation = validateExampleFolderName(name);
  if (validation) {
    throw new Error(`Invalid TRUST_UX_EXAMPLE_NAME: ${validation}`);
  }
  return {
    base: vscode.Uri.file(base),
    name,
    openFolder: process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER !== "0",
  };
}

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}
