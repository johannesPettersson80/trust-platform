// The Pages panel's actions on the project's page files (specification 39, §3.3 item 8): add,
// rename, duplicate, reorder (written as `order`), hide from the navigation and delete after a
// confirmation. Each change goes through the language server's page format, so the files stay
// canonical, and is one edit of the file's text, which an open editor of that page follows.

import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
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

async function parsed(text: string, name: string): Promise<HmiPageModel | undefined> {
  const answer = (await vscode.commands.executeCommand("trust-lsp.hmiFileParse", {
    kind: "page",
    text,
    file: `hmi/pages/${name}.toml`,
  })) as { ok?: boolean; model?: HmiPageModel } | undefined;
  return answer?.ok === true ? answer.model : undefined;
}

async function formatted(model: HmiPageModel): Promise<string | undefined> {
  const answer = (await vscode.commands.executeCommand("trust-lsp.hmiFileFormat", { kind: "page", model })) as
    { ok?: boolean; text?: string } | undefined;
  return answer?.ok === true ? answer.text : undefined;
}

/** A page's text: an open editor's, saved or not, else the file's, without opening it. */
async function textOf(project: vscode.Uri, name: string): Promise<string> {
  const uri = fileOf(project, name);
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
  return open ? open.getText() : new TextDecoder().decode(await vscode.workspace.fs.readFile(uri));
}

async function modelOf(project: vscode.Uri, name: string): Promise<HmiPageModel | undefined> {
  return parsed(await textOf(project, name), name);
}

/** Changes a page file through its model, as one edit of its text, and saves it; a page that does
 * not load is left as it is. */
async function rewrite(
  project: vscode.Uri,
  name: string,
  change: (model: HmiPageModel) => HmiPageModel,
): Promise<void> {
  const uri = fileOf(project, name);
  const document = await vscode.workspace.openTextDocument(uri);
  const model = await parsed(document.getText(), name);
  if (!model) return;
  const text = await formatted(change(model));
  if (text === undefined || text === document.getText()) return;
  const edit = new vscode.WorkspaceEdit();
  edit.replace(uri, new vscode.Range(0, 0, document.lineCount, 0), text);
  await vscode.workspace.applyEdit(edit);
  await document.save();
}

interface Listed extends PageEntry {
  order?: number;
}

/** The pages with their order, sorted as the navigation sorts them: by order, those without one
 * last, then by name. */
async function listed(project: vscode.Uri): Promise<Listed[]> {
  const pages: Listed[] = [];
  for (const name of await names(project)) {
    const model = await modelOf(project, name);
    pages.push({
      name,
      title: model?.title ?? name,
      hidden: model?.hidden === true,
      ...(model?.order !== undefined ? { order: model.order } : {}),
    });
  }
  return pages.sort(
    (a, b) =>
      Number(a.order === undefined) - Number(b.order === undefined) ||
      (a.order ?? 0) - (b.order ?? 0) ||
      (a.name < b.name ? -1 : a.name > b.name ? 1 : 0),
  );
}

/** The project's pages in the navigation's order. */
export async function listPages(project: vscode.Uri): Promise<PageEntry[]> {
  return (await listed(project)).map(({ name, title, hidden }) => ({ name, title, hidden }));
}

/** Writes each page's place in `sequence` as its order, where it differs. */
async function renumber(project: vscode.Uri, sequence: Listed[]): Promise<void> {
  for (const [order, page] of sequence.entries()) {
    if (page.order !== order) await rewrite(project, page.name, (model) => ({ ...model, order }));
  }
}

/** The first free page name starting with `stem`: `stem`, then `stem-2`, `stem-3`. */
async function freeName(project: vscode.Uri, stem: string): Promise<string> {
  const used = new Set(await names(project));
  if (!used.has(stem)) return stem;
  for (let number = 2; ; number += 1) {
    if (!used.has(`${stem}-${number}`)) return `${stem}-${number}`;
  }
}

/** Writes a new page file. */
async function create(project: vscode.Uri, name: string, model: HmiPageModel): Promise<void> {
  const text = await formatted(model);
  if (text === undefined)
    throw new Error("The truST language server is not running, so the page could not be written.");
  await vscode.workspace.fs.createDirectory(folder(project));
  await vscode.workspace.fs.writeFile(fileOf(project, name), new TextEncoder().encode(text));
}

/** Adds an empty page at the end of the navigation; its name. */
export async function addPage(project: vscode.Uri): Promise<string> {
  const sequence = await listed(project);
  const used = new Set(sequence.map((page) => page.name));
  let number = 1;
  while (used.has(`page-${number}`)) number += 1;
  const name = `page-${number}`;
  await renumber(project, sequence);
  await create(project, name, { title: `Page ${number}`, order: sequence.length, object: [], pipe: [] });
  return name;
}

/** Gives a page a new title, which the navigation shows. */
export async function renamePage(project: vscode.Uri, name: string, title: string): Promise<void> {
  if (title.trim() === "") return;
  await rewrite(project, name, (model) => ({ ...model, title: title.trim() }));
}

/** Copies a page, placed right after it in the navigation. */
export async function duplicatePage(project: vscode.Uri, name: string): Promise<void> {
  const model = await modelOf(project, name);
  if (!model) return;
  const copy = await freeName(project, `${name}-copy`);
  const sequence = await listed(project);
  const at = sequence.findIndex((page) => page.name === name);
  await renumber(project, sequence);
  await create(project, copy, { ...model, title: `${model.title} (copy)`, order: at + 1 });
  // The pages after the copy move one place on.
  for (const [index, page] of sequence.entries()) {
    if (index > at) await rewrite(project, page.name, (current) => ({ ...current, order: index + 1 }));
  }
}

/** Puts a page before another in the navigation, or at its end. */
export async function movePage(project: vscode.Uri, name: string, before: string | null): Promise<void> {
  const sequence = await listed(project);
  const moved = sequence.find((page) => page.name === name);
  if (!moved || name === before) return;
  const rest = sequence.filter((page) => page.name !== name);
  const at = before === null ? rest.length : rest.findIndex((page) => page.name === before);
  rest.splice(at < 0 ? rest.length : at, 0, moved);
  await renumber(project, rest);
}

/** Hides a page from the navigation, or shows it again. */
export async function hidePage(project: vscode.Uri, name: string, hidden: boolean): Promise<void> {
  await rewrite(project, name, (model) => {
    if (hidden) return { ...model, hidden: true };
    const { hidden: _hidden, ...rest } = model;
    return rest;
  });
}

/** Deletes a page's file once `confirm` says so. */
export async function deletePage(project: vscode.Uri, name: string, confirm: () => Promise<boolean>): Promise<void> {
  if (!(await confirm())) return;
  await vscode.workspace.fs.delete(fileOf(project, name));
}
