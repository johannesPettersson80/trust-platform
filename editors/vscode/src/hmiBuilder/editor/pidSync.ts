// Keeps a page and its P&ID one (specification 39, §18 phase 4 item 2): after each change of one,
// the language server (`trust-lsp.hmiPidSync`) names the files that share its process objects and
// gives each one's new text; this module only reads and writes them. An open file is edited as a
// document and left unsaved, as edits are; a closed one is written.

import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
import { RUST_RULES } from "./messages";

/** What `linked` answers: whether the file is a P&ID, and the files `sync` needs. */
interface Linked {
  ok?: boolean;
  pid?: boolean;
  files?: string[];
  pattern?: string;
}

/** What `sync` answers: the new text of each linked file that changes. */
interface Synced {
  ok?: boolean;
  writes?: Array<{ file: string; text: string }>;
}

/** A file's path in its project, with `/` between the parts, as the language server names files. */
export function projectPath(project: vscode.Uri, uri: vscode.Uri): string {
  return path.relative(project.fsPath, uri.fsPath).split(path.sep).join("/");
}

/** The file a project path names. */
function projectFile(project: vscode.Uri, file: string): vscode.Uri {
  return vscode.Uri.joinPath(project, ...file.split("/"));
}

async function ask<T>(argument: object): Promise<T | undefined> {
  try {
    return await vscode.commands.executeCommand<T>(RUST_RULES.pidSync, argument);
  } catch {
    return undefined;
  }
}

/** Whether a file is a P&ID, which the editor shows in the P&ID view (§18 phase 4 item 1). */
export async function isPid(project: vscode.Uri, uri: vscode.Uri): Promise<boolean> {
  const answer = await ask<Linked>({ action: "linked", file: projectPath(project, uri) });
  return answer?.ok === true && answer.pid === true;
}

/** Brings the files linked to this one in line with its model; both views have the screen's size. */
export async function syncLinked(
  project: vscode.Uri,
  uri: vscode.Uri,
  model: HmiPageModel,
  size: [number, number],
): Promise<void> {
  const file = projectPath(project, uri);
  const linked = await ask<Linked>({ action: "linked", file, model });
  if (linked?.ok !== true) return;
  const candidates = [
    ...(linked.files ?? []).map((name) => projectFile(project, name)),
    ...(linked.pattern ? await vscode.workspace.findFiles(new vscode.RelativePattern(project, linked.pattern)) : []),
  ];
  if (candidates.length === 0) return;
  const texts: Array<{ file: string; text: string }> = [];
  for (const candidate of candidates) {
    try {
      texts.push({ file: projectPath(project, candidate), text: await textOf(candidate) });
    } catch {
      // A file that cannot be read keeps its text.
    }
  }
  const synced = await ask<Synced>({ action: "sync", file, model, size, linked: texts });
  for (const write of synced?.ok === true ? (synced.writes ?? []) : []) {
    const target = projectFile(project, write.file);
    const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === target.toString());
    if (open) {
      const edit = new vscode.WorkspaceEdit();
      edit.replace(target, new vscode.Range(0, 0, open.lineCount, 0), write.text);
      await vscode.workspace.applyEdit(edit);
    } else {
      await vscode.workspace.fs.writeFile(target, Buffer.from(write.text, "utf8"));
    }
  }
}

/** A file's text: the open document's, unsaved changes included, or the file's. */
async function textOf(uri: vscode.Uri): Promise<string> {
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
  return open ? open.getText() : Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8");
}
