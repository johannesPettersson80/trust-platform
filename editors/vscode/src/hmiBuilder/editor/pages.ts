// The Pages panel's actions on the project's page files (specification 39, §3.3 item 8): add,
// rename, duplicate, reorder (written as `order`), hide from the navigation and delete after a
// confirmation. The language server decides the order, names and texts (`trust-lsp.hmiPages`);
// this module gives it every page's text, writes the files it answers, each as one edit of the
// file's text that an open editor of the page follows, and asks before a delete.

import * as vscode from "vscode";

import type { PageEntry } from "./messages";

const folder = (project: vscode.Uri): vscode.Uri => vscode.Uri.joinPath(project, "hmi", "pages");
const fileOf = (project: vscode.Uri, name: string): vscode.Uri => vscode.Uri.joinPath(folder(project), `${name}.toml`);

/** The names of the project's page files. */
async function names(project: vscode.Uri): Promise<string[]> {
  try {
    const entries = await vscode.workspace.fs.readDirectory(folder(project));
    return entries
      .filter(([name, type]) => type === vscode.FileType.File && name.endsWith(".toml"))
      .map(([name]) => name.slice(0, -".toml".length));
  } catch {
    return [];
  }
}

/** A page's text: an open editor's, saved or not, else the file's, without opening it. */
async function textOf(project: vscode.Uri, name: string): Promise<string> {
  const uri = fileOf(project, name);
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
  return open ? open.getText() : new TextDecoder().decode(await vscode.workspace.fs.readFile(uri));
}

/** Every page's text by its name. */
async function texts(project: vscode.Uri): Promise<Record<string, string>> {
  const pages: Record<string, string> = {};
  for (const name of await names(project)) pages[name] = await textOf(project, name);
  return pages;
}

interface PageFile {
  name: string;
  text: string;
}

interface Answer {
  ok?: boolean;
  error?: string;
  pages?: PageEntry[];
  name?: string;
  write?: PageFile[];
  create?: PageFile[];
}

/** An answer with the page texts it was given. */
type Answered = Answer & { sent: Record<string, string> };

/** The language server's answer to `request` about the project's pages, given their texts. */
async function ask(project: vscode.Uri, request: Record<string, unknown>): Promise<Answered> {
  const sent = await texts(project);
  const answer = await vscode.commands.executeCommand<Answer>("trust-lsp.hmiPages", { ...request, pages: sent });
  if (answer?.ok !== true) throw new Error(answer?.error ?? "The truST language server could not change the pages.");
  return { ...answer, sent };
}

/** Writes the files of an answer: each changed page as one edit of its text, saved, and each new
 * page as a file. Nothing is written when a page changed after its text was sent. */
async function apply(project: vscode.Uri, answer: Answered): Promise<void> {
  const targets: Array<{ document: vscode.TextDocument; text: string }> = [];
  for (const { name, text } of answer.write ?? []) {
    const document = await vscode.workspace.openTextDocument(fileOf(project, name));
    if (document.getText() !== answer.sent[name])
      throw new Error(`The page "${name}" changed meanwhile, so the pages are left as they are.`);
    targets.push({ document, text });
  }
  for (const { document, text } of targets) {
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), text);
    await vscode.workspace.applyEdit(edit);
    await document.save();
  }
  for (const { name, text } of answer.create ?? []) {
    await vscode.workspace.fs.createDirectory(folder(project));
    await vscode.workspace.fs.writeFile(fileOf(project, name), new TextEncoder().encode(text));
  }
}

/** The project's pages in the navigation's order. */
export async function listPages(project: vscode.Uri): Promise<PageEntry[]> {
  return (await ask(project, { action: "list" })).pages ?? [];
}

/** Adds an empty page at the end of the navigation; its name. */
export async function addPage(project: vscode.Uri): Promise<string> {
  const answer = await ask(project, { action: "add" });
  await apply(project, answer);
  return answer.name ?? "";
}

/** Gives a page a new title, which the navigation shows. */
export async function renamePage(project: vscode.Uri, name: string, title: string): Promise<void> {
  await apply(project, await ask(project, { action: "rename", name, title }));
}

/** Copies a page, placed right after it in the navigation. */
export async function duplicatePage(project: vscode.Uri, name: string): Promise<void> {
  await apply(project, await ask(project, { action: "duplicate", name }));
}

/** Puts a page before another in the navigation, or at its end. */
export async function movePage(project: vscode.Uri, name: string, before: string | null): Promise<void> {
  await apply(project, await ask(project, { action: "move", name, before }));
}

/** Hides a page from the navigation, or shows it again. */
export async function hidePage(project: vscode.Uri, name: string, hidden: boolean): Promise<void> {
  await apply(project, await ask(project, { action: "hide", name, hidden }));
}

/** Deletes a page's file once `confirm` says so. */
export async function deletePage(project: vscode.Uri, name: string, confirm: () => Promise<boolean>): Promise<void> {
  if (!(await confirm())) return;
  await vscode.workspace.fs.delete(fileOf(project, name));
}
