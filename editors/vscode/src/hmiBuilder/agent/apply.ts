import * as vscode from "vscode";
import { NoSourceChangeError } from "./applicationState";

/** A page the authoring preflight prepared for this transaction (the `files` of the apply answer). */
export interface PreparedFile {
  file: string;
  text: string;
  /** Whether the page does not exist yet, so the transaction creates it. */
  create: boolean;
}

/** What the transaction did. Saving is reported apart from the edit (specification 43, AT-09). */
export type ApplyOutcome = {
  applied: boolean;
  files: string[];
  saved: string[];
  unsaved: string[];
};

function check(signal: AbortSignal): void {
  if (signal.aborted) throw new Error("CANCELLED: draft application cancelled before commit.");
}

/** Source changes use one native editor WorkspaceEdit. Saving follows commit and is reported separately: a save
 * failure must not be mistaken for an unapplied edit. The preflight has checked every page against its base under
 * the draft lock; that base text is not in the preflight answer, so the transaction does not compare it again. */
export async function applyEditorFiles(
  root: vscode.Uri,
  files: PreparedFile[],
  save: boolean,
  signal: AbortSignal,
): Promise<ApplyOutcome> {
  let invoked = false;
  try {
    check(signal);
    const edit = new vscode.WorkspaceEdit();
    const uris = files.map((prepared) => vscode.Uri.joinPath(root, ...prepared.file.split("/")));
    for (const [index, prepared] of files.entries()) {
      const uri = uris[index];
      if (prepared.create) {
        // overwrite:false makes a file created by another actor a conflict, never an overwrite.
        edit.createFile(uri, { overwrite: false, ignoreIfExists: false });
        edit.insert(uri, new vscode.Position(0, 0), prepared.text);
      } else {
        const document = await vscode.workspace.openTextDocument(uri);
        edit.replace(uri, new vscode.Range(0, 0, document.lineCount, 0), prepared.text);
      }
    }
    check(signal);
    invoked = true;
    if (!(await vscode.workspace.applyEdit(edit, { isRefactoring: false })))
      throw new Error("CONFLICT: the editor refused the draft transaction; inspect source state before retrying.");
    // From here cancellation cannot honestly report that nothing happened.
    const names = files.map((prepared) => prepared.file);
    const saved: string[] = [];
    const unsaved: string[] = [];
    for (const [index, file] of names.entries()) {
      if (save) {
        try {
          const document = await vscode.workspace.openTextDocument(uris[index]);
          if (await document.save()) {
            saved.push(file);
            continue;
          }
        } catch {
          /* Keep the successful edit visible, with the failed save reported separately. */
        }
      }
      unsaved.push(file);
    }
    return { applied: true, files: names, saved, unsaved };
  } catch (error) {
    if (!invoked) throw new NoSourceChangeError(error);
    throw error;
  }
}
