// Bundled packages use the ordinary library manifests; source and visuals travel together. The
// language server (`trust-lsp.hmiLibraries`) decides what installing them takes: the packages with
// those they depend on, the files the project lacks, the manifest's new text and the refusals. This
// adds its answer to the caller's edit and, before the edit is applied, checks that nothing it read
// changed meanwhile.

import { createHash } from "crypto";
import * as path from "path";
import * as vscode from "vscode";

interface LibrariesAnswer {
  ok?: boolean;
  error?: string;
  create?: Array<{ path: string; text: string }>;
  manifest?: { path: string; text: string } | null;
  checked?: Array<{ path: string; sha256: string | null }>;
}

/** A file's text as an open document has it, else as the disk has it; undefined when it does not exist. */
async function textOf(uri: vscode.Uri): Promise<string | undefined> {
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
  if (open) return open.getText();
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(await vscode.workspace.fs.readFile(uri));
  } catch (error) {
    if (error instanceof vscode.FileSystemError && error.code === "FileNotFound") return undefined;
    throw error;
  }
}

/** The project's documents open in an editor, by their project-relative path. */
function openTexts(project: vscode.Uri): Record<string, string> {
  const prefix = project.fsPath + path.sep;
  const open: Record<string, string> = {};
  for (const document of vscode.workspace.textDocuments) {
    if (document.uri.scheme === "file" && document.uri.fsPath.startsWith(prefix))
      open[path.relative(project.fsPath, document.uri.fsPath).split(path.sep).join("/")] = document.getText();
  }
  return open;
}

function digest(text: string | undefined): string | null {
  return text === undefined ? null : createHash("sha256").update(text, "utf8").digest("hex");
}

/** Adds the bundled libraries `requested` to `edit` for `project`; answers the check to run right
 * before the edit is applied. */
export async function prepareBundledLibraries(
  project: vscode.Uri,
  extensionPath: string,
  requested: Array<{ name: string; folder: string }>,
  edit: vscode.WorkspaceEdit,
): Promise<() => Promise<void>> {
  const answer = await vscode.commands.executeCommand<LibrariesAnswer | undefined>("trust-lsp.hmiLibraries", {
    root: project.fsPath,
    bundled: path.join(extensionPath, "media", "libraries"),
    libraries: requested,
    open: openTexts(project),
  });
  if (answer?.ok !== true)
    throw new Error(
      answer?.error ?? "The truST language server is not running, so the libraries could not be prepared.",
    );
  const at = (relative: string): vscode.Uri => vscode.Uri.joinPath(project, ...relative.split("/"));
  // Install immutable package contents as resource edits. Text inserts would open every
  // bundled ST file as a dirty editor and trigger hundreds of diagnostic refreshes.
  for (const file of answer.create ?? [])
    edit.createFile(at(file.path), {
      overwrite: false,
      ignoreIfExists: false,
      contents: Buffer.from(file.text, "utf8"),
    });
  if (answer.manifest) {
    const uri = at(answer.manifest.path);
    const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
    // An open trust-lsp.toml is edited in its editor; otherwise the file is written as a whole,
    // existing or not, so the dependencies reach the disk the language server reads.
    if (open) edit.replace(uri, new vscode.Range(0, 0, open.lineCount, 0), answer.manifest.text);
    else edit.createFile(uri, { overwrite: true, contents: Buffer.from(answer.manifest.text, "utf8") });
  }
  const checked = answer.checked ?? [];
  return async () => {
    for (const file of checked) {
      if (digest(await textOf(at(file.path))) !== file.sha256)
        throw new Error("Project files changed while preparing the library. Your edits were preserved.");
    }
  };
}
