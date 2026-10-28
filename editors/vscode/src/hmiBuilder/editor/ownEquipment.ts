// My equipment on the host (specification 39, §21.9): the files the language server proposes for
// one of the project's own function blocks, written when it is first placed, and its picture's
// change, written to its definition and to the pages showing it. Each file is saved unless it had
// unsaved changes of its own.

import * as vscode from "vscode";

import { openHmiTexts } from "./textFiles";

/** A file below the project, as the language server gives it. */
export interface ProjectFile {
  path: string;
  text: string;
}

interface Answer {
  ok?: boolean;
  creates?: ProjectFile[];
  edits?: ProjectFile[];
  errors?: Array<{ message: string }>;
}

/** Why an answer was refused. */
function refusal(answer: Answer | undefined, fallback: string): string {
  return answer?.errors?.[0]?.message ?? fallback;
}

/** Writes files below the project: a new one is created, an existing one replaced whole; gives why
 * it could not, or `undefined`. */
export async function writeProjectFiles(
  project: vscode.Uri,
  files: ProjectFile[],
  create: boolean,
): Promise<string | undefined> {
  const uris = files.map((file) => vscode.Uri.joinPath(project, ...file.path.split("/")));
  const edit = new vscode.WorkspaceEdit();
  const dirty = new Set<string>();
  for (const [index, file] of files.entries()) {
    const uri = uris[index];
    if (create) {
      edit.createFile(uri, { overwrite: false, ignoreIfExists: false });
      edit.insert(uri, new vscode.Position(0, 0), file.text);
    } else {
      const document = await vscode.workspace.openTextDocument(uri);
      if (document.isDirty) dirty.add(uri.toString());
      edit.replace(uri, new vscode.Range(0, 0, document.lineCount, 0), file.text);
    }
  }
  if (!(await vscode.workspace.applyEdit(edit))) return "VS Code could not write the equipment's files.";
  for (const uri of uris) {
    if (dirty.has(uri.toString())) continue;
    const document = await vscode.workspace.openTextDocument(uri);
    await document.save();
  }
  return undefined;
}

/** Creates the definition and panel the language server proposes for one of the project's own
 * blocks (§21.9 item 2); gives why it could not, or `undefined`. */
export async function createOwnEquipment(project: vscode.Uri, block: string): Promise<string | undefined> {
  const answer = await vscode.commands.executeCommand<Answer>("trust-lsp.hmiOwnEquipment", {
    root_uri: project.toString(),
    open: openHmiTexts(project),
    block,
  });
  if (!answer?.ok || !answer.creates) return refusal(answer, `The files of ${block} could not be made.`);
  return writeProjectFiles(project, answer.creates, true);
}

/** Gives one of the project's own definitions another picture, its inputs driven by `bind`'s
 * members and otherwise matched by name (§21.9 items 4 and 5); gives why it could not. */
export async function changeEquipmentPicture(
  project: vscode.Uri,
  block: string,
  symbol: string,
  bind: Record<string, string>,
): Promise<string | undefined> {
  const answer = await vscode.commands.executeCommand<Answer>("trust-lsp.hmiOwnEquipment", {
    root_uri: project.toString(),
    open: openHmiTexts(project),
    block,
    symbol,
    bind,
  });
  if (!answer?.ok || !answer.edits) return refusal(answer, `The picture of ${block} could not be changed.`);
  return writeProjectFiles(project, answer.edits, false);
}
