// New and imported visual programs (specification 25 §11.4): the name rule, the template of a new
// program and the target of an import are the language server's (`trust-lsp.visualNew`,
// `trust-lsp.visualImport`); the commands ask, then write, open and prompt.

import * as path from "path";
import * as vscode from "vscode";
import { openCompanion, openCompanionOnCreateEnabled, syncVisualCompanionFromUri } from "./companionSt";
import { problemText, visualRequest, type VisualAnswer } from "./visualCommands";

/** A kind of visual program, as the language server names it. */
export type VisualKindName = "ladder" | "blockly" | "statechart" | "sfc";

/** The custom editor of each kind (package.json `customEditors`). */
export const VISUAL_EDITOR_VIEW_TYPES: Record<VisualKindName, string> = {
  ladder: "trust-lsp.ladder.editor",
  blockly: "trust-lsp.blockly.editor",
  statechart: "trust-lsp.statechartEditor",
  sfc: "trust-lsp.sfc.editor",
};

interface NewAnswer extends VisualAnswer {
  fileName?: string;
  text?: string;
}

interface ImportAnswer extends VisualAnswer {
  targetFile?: string;
}

/** Why `name` cannot name a new program of `kind`, or `undefined` when it can (for an input box). */
export async function checkProgramName(
  kind: VisualKindName,
  name: string
): Promise<string | undefined> {
  const answer = await visualRequest("trust-lsp.visualNew", { kind, name, checkOnly: true });
  return answer.ok ? undefined : problemText(answer);
}

/** The file name and text of a new program of `kind` called `name`, or why there is none. */
export async function newProgramFile(
  kind: VisualKindName,
  name: string
): Promise<{ fileName: string; text: string } | { error: string }> {
  const answer = await visualRequest<NewAnswer>("trust-lsp.visualNew", {
    kind,
    name,
    created: new Date().toISOString(),
  });
  if (answer.ok && typeof answer.fileName === "string" && typeof answer.text === "string") {
    return { fileName: answer.fileName, text: answer.text };
  }
  return { error: problemText(answer) };
}

async function exists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

/**
 * Creates the program `name` of `kind` in `folder` and opens it as specification 25 §11.4 says:
 * in its visual editor, in the active group and with focus; with the companion-on-create setting
 * on, its Structured Text companion is written and opens beside it without focus. An existing
 * file is replaced only after the user confirms (or with `overwrite`). Answers the new file.
 */
export async function createVisualProgram(
  kind: VisualKindName,
  folder: vscode.Uri,
  name: string,
  overwrite = false
): Promise<vscode.Uri | undefined> {
  const created = await newProgramFile(kind, name);
  if ("error" in created) {
    void vscode.window.showErrorMessage(created.error);
    return undefined;
  }
  const fileUri = vscode.Uri.joinPath(folder, created.fileName);
  if (!overwrite && (await exists(fileUri))) {
    const choice = await vscode.window.showWarningMessage(
      `File ${created.fileName} already exists. Do you want to overwrite it?`,
      { modal: true },
      "Overwrite",
      "Cancel"
    );
    if (choice !== "Overwrite") {
      return undefined;
    }
  }
  await vscode.workspace.fs.writeFile(fileUri, Buffer.from(created.text, "utf8"));
  await vscode.commands.executeCommand("vscode.openWith", fileUri, VISUAL_EDITOR_VIEW_TYPES[kind], {
    preview: false,
  });
  const openBeside = openCompanionOnCreateEnabled(fileUri);
  const companion = await syncVisualCompanionFromUri(fileUri, { force: openBeside, showErrors: true });
  if (openBeside && companion) {
    await openCompanion(companion, { beside: true });
  }
  void vscode.window.showInformationMessage(`Created ${created.fileName}.`);
  return fileUri;
}

/** The file name an import of `source` gets, or why the file cannot be imported as `kind`. */
export async function importTargetName(
  kind: VisualKindName,
  source: vscode.Uri
): Promise<{ fileName: string } | { error: string }> {
  let text: string;
  try {
    text = Buffer.from(await vscode.workspace.fs.readFile(source)).toString("utf8");
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
  const answer = await visualRequest<ImportAnswer>("trust-lsp.visualImport", {
    kind,
    file: source.fsPath,
    text,
  });
  if (answer.ok && typeof answer.targetFile === "string") {
    return { fileName: answer.targetFile };
  }
  return { error: problemText(answer) };
}

/**
 * Opens an imported program: its companion is written and, with the companion-on-create setting
 * on, opened; otherwise the program opens in its visual editor.
 */
export async function openImportedProgram(target: vscode.Uri): Promise<void> {
  const companion = await syncVisualCompanionFromUri(target, { force: true, showErrors: true });
  if (openCompanionOnCreateEnabled(target) && companion) {
    await openCompanion(companion);
  } else {
    await vscode.commands.executeCommand("vscode.open", target);
  }
}

/** What an import command asks and where it puts the copy. */
export interface ImportRequest {
  kind: VisualKindName;
  /** The program kind in messages: "Ladder program", "SFC", "statechart", "Blockly program". */
  noun: string;
  /** The workspace folder the file dialog starts in when it exists, e.g. `examples/ladder`. */
  examplesFolder: string;
  /** The file dialog's filters. */
  filters: Record<string, string[]>;
  source?: vscode.Uri;
  /** The copy itself; without it the copy goes into `targetFolder` (or a chosen folder). */
  targetFile?: vscode.Uri;
  targetFolder?: vscode.Uri;
  /** A source inside the workspace is opened where it is instead of copied. */
  openInPlace: boolean;
  overwrite?: boolean;
  openAfterImport?: boolean;
}

/**
 * Imports a visual program: the language server checks the file's shape and names the copy
 * (specification 25 §11.4); the copy replaces an existing file only after the user confirms, and
 * then opens ({@link openImportedProgram}). Answers the imported file.
 */
export async function importVisualProgram(request: ImportRequest): Promise<vscode.Uri | undefined> {
  const root = vscode.workspace.workspaceFolders?.[0]?.uri;
  let source = request.source;
  if (!source) {
    const examples = root ? vscode.Uri.joinPath(root, request.examplesFolder) : undefined;
    const selected = await vscode.window.showOpenDialog({
      canSelectFiles: true,
      canSelectFolders: false,
      canSelectMany: false,
      defaultUri: examples && (await exists(examples)) ? examples : root,
      filters: request.filters,
      openLabel: `Select ${request.noun} to Import`,
    });
    source = selected?.[0];
  }
  if (!source) {
    return undefined;
  }
  if (!(await exists(source))) {
    void vscode.window.showErrorMessage(`Source file not found: ${source.fsPath}`);
    return undefined;
  }
  const named = await importTargetName(request.kind, source);
  if ("error" in named) {
    void vscode.window.showErrorMessage(named.error);
    return undefined;
  }
  let target = request.targetFile;
  if (!target && request.openInPlace && root && source.toString().startsWith(root.toString())) {
    target = source;
  }
  if (!target) {
    let folder = request.targetFolder ?? root;
    if (!folder) {
      const selected = await vscode.window.showOpenDialog({
        canSelectFiles: false,
        canSelectFolders: true,
        canSelectMany: false,
        openLabel: "Select Destination Folder",
      });
      folder = selected?.[0];
    }
    if (!folder) {
      return undefined;
    }
    target = vscode.Uri.joinPath(folder, named.fileName);
  }
  if (target.toString() !== source.toString()) {
    if (await exists(target)) {
      const overwrite =
        request.overwrite ??
        (await vscode.window.showWarningMessage(
          `The file already exists: ${target.fsPath}\nDo you want to overwrite it?`,
          { modal: true },
          "Overwrite",
          "Cancel"
        )) === "Overwrite";
      if (!overwrite) {
        return undefined;
      }
    }
    try {
      await vscode.workspace.fs.writeFile(target, await vscode.workspace.fs.readFile(source));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(`Failed to import ${request.noun}: ${message}`);
      return undefined;
    }
    void vscode.window.showInformationMessage(`${request.noun} imported successfully: ${target.fsPath}`);
  }
  if (request.openAfterImport ?? true) {
    try {
      await openImportedProgram(target);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(`Failed to open ${request.noun}: ${message}`);
      return undefined;
    }
  }
  return target;
}

/** The folder a new program goes into: the target folder, the folder of a target file, or the first workspace folder. */
export async function programFolder(target: vscode.Uri | undefined): Promise<vscode.Uri | undefined> {
  const base = target ?? vscode.workspace.workspaceFolders?.[0]?.uri;
  if (!base) {
    void vscode.window.showErrorMessage("No workspace folder open. Please open a folder first.");
    return undefined;
  }
  try {
    const stat = await vscode.workspace.fs.stat(base);
    return stat.type === vscode.FileType.Directory ? base : vscode.Uri.file(path.dirname(base.fsPath));
  } catch {
    return vscode.Uri.file(path.dirname(base.fsPath));
  }
}
