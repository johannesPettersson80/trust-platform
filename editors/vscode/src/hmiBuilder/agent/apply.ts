import { NoSourceChangeError } from "./applicationState";
import * as vscode from "vscode";
import { sameContent, WorkspaceDocuments } from "./documents";
import type { Draft } from "./drafts";
export interface ApplyOutcome {
  applied: boolean;
  files: string[];
  saved: string[];
  unsaved: string[];
}
function check(signal: AbortSignal): void {
  if (signal.aborted) throw new Error("CANCELLED: draft application cancelled before commit.");
}

/** Source changes use one native editor WorkspaceEdit. Saving follows commit and is reported
 * separately: a save failure must not be mistaken for an unapplied edit. A conflict means a page
 * holds different text than when the draft was prepared; merely opening, closing or saving a
 * page in an editor changes its version, not its text, and is no conflict. */
export async function applyDraftFiles(
  documents: WorkspaceDocuments,
  draft: Draft,
  save: boolean,
  signal: AbortSignal,
): Promise<ApplyOutcome> {
  let invoked = false;
  try {
    check(signal);
    const files = Object.keys(draft.files);
    if (!files.length || files.length > 32) throw new Error("LIMIT_EXCEEDED: provide 1 to 32 changed files.");
    const base = new Map(draft.base.map((snapshot) => [snapshot.file, snapshot]));
    for (const file of files) {
      if (!/^hmi\/pages\/[A-Za-z0-9_-]+\.toml$/.test(file))
        throw new Error("INVALID_ARGUMENT: this transaction accepts HMI pages only.");
      if (!base.has(file)) throw new Error("CONFLICT: changed file has no base revision.");
      if (Buffer.byteLength(draft.files[file]) > 1024 * 1024) throw new Error("LIMIT_EXCEEDED: page exceeds 1 MiB.");
    }
    for (const expected of draft.base) {
      const current = await documents.read(expected.file, signal);
      if (!sameContent(current, expected))
        throw new Error(`CONFLICT: ${expected.file} changed since the draft was created.`);
    }
    const edit = new vscode.WorkspaceEdit();
    const opened: Array<{ file: string; document: vscode.TextDocument; text: string }> = [];
    for (const file of files) {
      const uri = await documents.uri(file, signal);
      const expected = base.get(file)!;
      if (expected.exists) {
        const document = await vscode.workspace.openTextDocument(uri);
        if (document.getText() !== expected.text) throw new Error(`CONFLICT: ${file} changed before application.`);
        opened.push({ file, document, text: expected.text });
        edit.replace(uri, new vscode.Range(0, 0, document.lineCount, 0), draft.files[file]);
      } else {
        // overwrite:false makes a file created by another actor a conflict, never an overwrite.
        edit.createFile(uri, { overwrite: false, ignoreIfExists: false });
        edit.insert(uri, new vscode.Position(0, 0), draft.files[file]);
      }
    }
    check(signal);
    for (const item of opened)
      if (item.document.getText() !== item.text) throw new Error(`CONFLICT: ${item.file} changed before commit.`);
    invoked = true;
    if (!(await vscode.workspace.applyEdit(edit, { isRefactoring: false })))
      throw new Error("CONFLICT: the editor refused the draft transaction; inspect source state before retrying.");
    // From here cancellation cannot honestly report that nothing happened.
    const saved: string[] = [],
      unsaved: string[] = [];
    for (const file of files) {
      if (save) {
        try {
          const document = await vscode.workspace.openTextDocument(await documents.uri(file));
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
    return { applied: true, files, saved, unsaved };
  } catch (error) {
    if (!invoked) throw new NoSourceChangeError(error);
    throw error;
  }
}
