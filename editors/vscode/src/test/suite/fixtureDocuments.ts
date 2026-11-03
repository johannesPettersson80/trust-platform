import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

/** Dispose only a generated test project, including editor models opened without a visible tab.
 * Clear owned source models before deletion: VS Code can cache models after their tabs close,
 * and these external temporary directories have no workspace file watcher. */
export async function disposeFixtureProject(root: string): Promise<void> {
  const directory = path.resolve(root);
  if (!directory.startsWith(path.resolve(os.tmpdir()) + path.sep)) throw new Error("Fixture cleanup must stay inside the test temporary directory.");
  const documents = vscode.workspace.textDocuments.filter(document => document.uri.scheme === "file" && document.uri.fsPath.startsWith(directory + path.sep) && !document.isClosed);
  // VS Code may retain a closed editor's text model in its undo cache. Empty generated ST
  // buffers before deleting their files, so an unclosed model cannot import stale declarations.
  const sources = documents.filter(document => /\.(st|pou)$/i.test(document.uri.path));
  const clear = new vscode.WorkspaceEdit();
  for (const document of sources) clear.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), "");
  if (sources.length && !await vscode.workspace.applyEdit(clear)) throw new Error("Could not clear generated fixture source models.");
  for (const document of sources) if (!await document.save()) throw new Error("Could not save cleared fixture source before cleanup.");
  // Close each editor while its file still exists: opening a file that is gone fails at random
  // ("Could NOT open editor for …", W18, 1 November 2026), as VS Code may already have dropped it.
  for (const document of documents) {
    if (document.isClosed) continue;
    await vscode.window.showTextDocument(document, {preview: false});
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  }
  await fs.rm(directory, {recursive: true, force: true});
}
