// Keeps a page and its P&ID one (specification 39, §18 phase 4 item 2): after each change of one,
// the other gets the shared process objects and pipes. An open file is edited as a document and
// left unsaved, as edits are; a closed one is written.

import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
import type { PageFiles } from "./pageDocument";
import { linkedUpdate } from "./pidLink";

/** Whether a file is a P&ID, `hmi/pid/<name>.toml`. */
export function isPidFile(uri: vscode.Uri): boolean {
  return (
    path.basename(path.dirname(uri.fsPath)) === "pid" && path.basename(path.dirname(path.dirname(uri.fsPath))) === "hmi"
  );
}

/** The files that share process objects with this one: a page's P&ID, or the pages naming a P&ID. */
async function linkedFiles(
  project: vscode.Uri,
  uri: vscode.Uri,
  model: HmiPageModel,
  files: PageFiles,
): Promise<vscode.Uri[]> {
  if (!isPidFile(uri)) return model.pid ? [vscode.Uri.joinPath(project, "hmi", "pid", `${model.pid}.toml`)] : [];
  const name = path.basename(uri.fsPath, ".toml");
  const found: vscode.Uri[] = [];
  for (const page of await vscode.workspace.findFiles(new vscode.RelativePattern(project, "hmi/pages/*.toml"))) {
    const read = await files.read(await textOf(page), `hmi/pages/${path.basename(page.fsPath)}`);
    if (read.ok && read.model.pid === name) found.push(page);
  }
  return found;
}

/** Brings the files linked to this one in line with its model; both views have the screen's size. */
export async function syncLinked(
  project: vscode.Uri,
  uri: vscode.Uri,
  model: HmiPageModel,
  size: [number, number],
  files: PageFiles,
): Promise<void> {
  for (const linked of await linkedFiles(project, uri, model, files)) {
    if (linked.toString() === uri.toString()) continue;
    let text: string;
    try {
      text = await textOf(linked);
    } catch {
      continue;
    }
    const name = `hmi/${path.basename(path.dirname(linked.fsPath))}/${path.basename(linked.fsPath)}`;
    const read = await files.read(text, name);
    if (!read.ok) continue;
    const next = linkedUpdate(model, size, read.model, size);
    if (!next) continue;
    const written = await files.write(next);
    if (!written.ok) continue;
    const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === linked.toString());
    if (open) {
      const edit = new vscode.WorkspaceEdit();
      edit.replace(linked, new vscode.Range(0, 0, open.lineCount, 0), written.text);
      await vscode.workspace.applyEdit(edit);
    } else {
      await vscode.workspace.fs.writeFile(linked, Buffer.from(written.text, "utf8"));
    }
  }
}

/** A file's text: the open document's, unsaved changes included, or the file's. */
async function textOf(uri: vscode.Uri): Promise<string> {
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
  return open ? open.getText() : Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8");
}
