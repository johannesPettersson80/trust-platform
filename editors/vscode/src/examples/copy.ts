import * as vscode from "vscode";

import { prepareLessonProjectHandoff, prepareStarterProjectHandoff } from "../projectLanding";
import { openPendingProjectLanding } from "../projectLandingActivation";
import { openCreatedProjectFolder } from "../projectOpening";
export { OPEN_LESSON_README_KEY } from "../projectLanding";
import type { ExampleEntry } from "./model";
import { adaptCopiedLessonReadme } from "./copiedLessonReadme";
import { persistControlLessonProject } from "./projectLocations";

// Copy the selected example, then hand its intended destination to the shared
// activation dispatcher before opening the new folder.
export async function copyExample(
  context: vscode.ExtensionContext,
  entry: ExampleEntry
): Promise<boolean> {
  const automation = readAcceptanceCopyOverride(entry);
  const base = automation?.base ?? (await promptExampleDestination());
  if (!base) {
    return false;
  }

  const source = vscode.Uri.joinPath(
    context.extensionUri,
    "media",
    "examples",
    entry.path
  );
  while (true) {
    const name = automation?.name ?? (await promptExampleName(entry));
    if (!name) {
      return false;
    }
    const nameValidation = validateExampleFolderName(name);
    if (nameValidation) {
      void vscode.window.showErrorMessage(nameValidation);
      return false;
    }
    const dest = vscode.Uri.joinPath(base, name.trim());
    if (await pathExists(dest)) {
      if (automation) {
        return false;
      }
      const choice = await vscode.window.showWarningMessage(
        `${dest.fsPath} already exists. Choose another project name; existing files will not be overwritten.`,
        { modal: true },
        "Choose another name"
      );
      if (choice === "Choose another name") {
        continue;
      }
      return false;
    }

    try {
      await vscode.workspace.fs.copy(source, dest, { overwrite: false });
      if (entry.lesson) {
        await adaptCopiedLessonReadme(context, entry, dest);
      }
    } catch (error) {
      void vscode.window.showErrorMessage(
        `Could not copy the example: ${String(error)}`
      );
      return false;
    }

    if (entry.lesson) {
      await persistControlLessonProject(context.globalState, entry.id, dest.fsPath);
    }

    // Keep one landing handoff per copied project. Control lessons land on their
    // instructions; ordinary starters land on Main.st. Setting both keys caused
    // two activation consumers to compete for the first editor.
    if (entry.lesson) {
      await prepareLessonProjectHandoff(
        context,
        dest.fsPath,
        vscode.Uri.joinPath(dest, "README.md").fsPath,
        entry.id
      );
    } else {
      await prepareStarterProjectHandoff(context, vscode.Uri.joinPath(dest, "src", "Main.st").fsPath);
    }
    if (automation?.openFolder === false) {
      return true;
    }
    return openCreatedProjectFolder(dest);
  }
}

/**
 * Compatibility entrypoint for the shared project landing dispatcher. Missing
 * destinations produce recovery guidance; unrelated workspaces leave them pending.
 */
export async function openPendingControlLessonReadme(
  context: vscode.ExtensionContext
): Promise<void> {
  await openPendingProjectLanding(context);
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

export function validateExampleFolderName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "A folder name is required.";
  }
  if (trimmed === "." || trimmed === "..") {
    return "Choose a project folder name, not a path marker.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "The name must not contain path separators.";
  }
  if (/[<>:\"|?*\u0000-\u001f]/.test(trimmed)) {
    return "The name contains characters that are not valid in a folder name.";
  }
  if (/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(trimmed)) {
    return "Choose a name that is valid on Windows too.";
  }
  if (/[. ]$/.test(trimmed)) {
    return "A folder name cannot end with a space or period.";
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
