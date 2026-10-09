// The project's texts on the editor's host (specification 39, §14.5 items 7 and 8): read through the
// language server with the HMI files the editor holds unsaved, and a translation written to its
// language's text file in the canonical form.

import * as path from "path";
import * as vscode from "vscode";

import type { ProjectTexts } from "./messages";
import type { FileError } from "./pageDocument";

/** The shape of a language tag that names a text file (§14.5 item 2). */
const LANGUAGE_TAG = /^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$/;
const NO_ANSWER: FileError = { message: "The truST language server gave no answer." };

/** Unsaved project text shared by native source, library and presentation readers. */
export function openHmiTexts(project: vscode.Uri): Record<string, string> {
  const prefix = project.fsPath + path.sep;
  const open: Record<string, string> = {};
  for (const document of vscode.workspace.textDocuments) {
    if (
      document.isDirty &&
      document.uri.scheme === "file" &&
      document.uri.fsPath.startsWith(prefix) &&
      /\.(toml|st|pou|svg)$/i.test(document.uri.path)
    ) {
      open[path.relative(project.fsPath, document.uri.fsPath).split(path.sep).join("/")] = document.getText();
    }
  }
  return open;
}

/** The project's languages and texts, or undefined while the language server cannot give them. */
export async function languageServerTexts(project: vscode.Uri): Promise<ProjectTexts | undefined> {
  const open = openHmiTexts(project);
  try {
    const answer = await vscode.commands.executeCommand<(ProjectTexts & { ok?: boolean }) | undefined>(
      "trust-lsp.hmiTexts",
      { root_uri: project.toString(), open },
    );
    if (answer?.ok !== true) return undefined;
    return {
      languages: answer.languages,
      texts: answer.texts,
      words: answer.words,
      project: answer.project,
      builtin: answer.builtin,
      shipped: answer.shipped ?? [],
      errors: answer.errors,
    };
  } catch {
    return undefined;
  }
}

/**
 * Writes a text's translation to `hmi/texts/<language>.toml`, creating the file when it is
 * missing; an empty translation removes the text. The file is saved unless it had unsaved
 * changes of its own. Gives the reasons when the file cannot be written, such as a file that does
 * not load.
 */
export async function writeTranslation(
  project: vscode.Uri,
  language: string,
  text: string,
  translation: string,
): Promise<FileError[]> {
  if (!LANGUAGE_TAG.test(language)) return [{ message: `"${language}" is not a language tag.` }];
  const relative = `hmi/texts/${language}.toml`;
  const file = vscode.Uri.joinPath(project, "hmi", "texts", `${language}.toml`);
  let document: vscode.TextDocument | undefined;
  try {
    document = await vscode.workspace.openTextDocument(file);
  } catch {
    document = undefined;
  }
  let texts: Record<string, string> = {};
  if (document) {
    const parsed = await vscode.commands.executeCommand<
      { ok?: boolean; model?: { texts: Record<string, string> }; errors?: FileError[] } | undefined
    >("trust-lsp.hmiFileParse", { kind: "texts", text: document.getText(), file: relative });
    if (parsed?.ok !== true || !parsed.model) return parsed?.errors ?? [NO_ANSWER];
    texts = { ...parsed.model.texts };
  }
  if (translation.trim() === "") delete texts[text];
  else texts[text] = translation;
  const formatted = await vscode.commands.executeCommand<
    { ok?: boolean; text?: string; errors?: FileError[] } | undefined
  >("trust-lsp.hmiFileFormat", {
    kind: "texts",
    model: { language, texts },
  });
  if (formatted?.ok !== true || typeof formatted.text !== "string") return formatted?.errors ?? [NO_ANSWER];
  if (!document) {
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(project, "hmi", "texts"));
    await vscode.workspace.fs.writeFile(file, new TextEncoder().encode(formatted.text));
    // Opened, so the checks of the Problems view see the new file.
    await vscode.workspace.openTextDocument(file);
    return [];
  }
  if (document.getText() === formatted.text) return [];
  const wasDirty = document.isDirty;
  const edit = new vscode.WorkspaceEdit();
  edit.replace(file, new vscode.Range(0, 0, document.lineCount, 0), formatted.text);
  if (!(await vscode.workspace.applyEdit(edit))) return [{ message: `${relative} could not be changed.` }];
  if (!wasDirty) await document.save();
  return [];
}
