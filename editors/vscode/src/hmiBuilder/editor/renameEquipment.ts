// Renaming equipment in the HMI Builder (specification 39, §21.3 item 2): the language server
// renames the instance's declaration as Rename Symbol does, so its program, the pages that show it
// and plant.toml follow in one edit, saved like the builder's other project edits.
import * as vscode from "vscode";

interface NativeEdit {
  range: { start: { line: number; character: number }; end: { line: number; character: number } };
  newText: string;
}
interface Answer {
  ok?: boolean;
  error?: string;
  errors?: Array<{ message: string }>;
  edit?: { changes?: Record<string, NativeEdit[]> };
}

/** Renames `instance` (`Main.P101`) to `name`; answers why not, if not. */
export async function renameEquipment(
  project: vscode.Uri,
  instance: string,
  name: string,
): Promise<string | undefined> {
  const answer = await vscode.commands.executeCommand<Answer>("trust-lsp.hmiRenameInstance", {
    root_uri: project.toString(),
    instance,
    new_name: name,
  });
  if (!answer?.ok) return answer?.errors?.[0]?.message ?? answer?.error ?? "The equipment cannot be renamed.";
  const edit = new vscode.WorkspaceEdit();
  const targets: Array<{ document: vscode.TextDocument; dirty: boolean }> = [];
  for (const [uri, edits] of Object.entries(answer.edit?.changes ?? {})) {
    const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(uri));
    targets.push({ document, dirty: document.isDirty });
    for (const change of edits)
      edit.replace(
        document.uri,
        new vscode.Range(
          change.range.start.line,
          change.range.start.character,
          change.range.end.line,
          change.range.end.character,
        ),
        change.newText,
      );
  }
  if (!(await vscode.workspace.applyEdit(edit))) return "VS Code could not apply the rename.";
  // A file with unsaved changes of the user's own stays unsaved, for the user to review (§10.3).
  for (const target of targets) if (!target.dirty) await target.document.save();
  return undefined;
}
