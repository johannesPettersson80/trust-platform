// The project's runtime.toml as the editor changes it (specification 39, §12.8): the language
// server answers the new text (the runtime display's users and write policy added or removed, the
// rest of the file and its comments as they are), and this applies it as one edit of the file.

import * as vscode from "vscode";

/** Changes the project's runtime.toml to the text `change` answers for its text, and saves it
 * unless it had unsaved changes of the user's own; answers whether it changed. */
export async function editRuntimeToml(
  root: vscode.Uri,
  change: (text: string) => string | Promise<string>,
): Promise<boolean> {
  const file = vscode.Uri.joinPath(root, "runtime.toml");
  let document: vscode.TextDocument;
  try {
    document = await vscode.workspace.openTextDocument(file);
  } catch {
    throw new Error("The project has no runtime.toml to hold the runtime display's settings.");
  }
  const dirty = document.isDirty;
  const text = document.getText();
  const next = await change(text);
  if (next === text) return false;
  const edit = new vscode.WorkspaceEdit();
  edit.replace(file, new vscode.Range(document.positionAt(0), document.positionAt(text.length)), next);
  if (!(await vscode.workspace.applyEdit(edit))) throw new Error("VS Code could not change runtime.toml.");
  // runtime.toml usually has no editor open, so an unsaved change would be lost (as Create PLC
  // code saves); one with unsaved changes of the user's own stays unsaved for the user to review.
  if (!dirty) await document.save();
  return true;
}
