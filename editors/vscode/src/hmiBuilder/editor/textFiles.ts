// The project's texts on the editor's host (specification 39, §14.5 items 7 and 8): read through the
// language server with the HMI files the editor holds unsaved, and a translation written to its
// language's text file in the canonical form.

import * as path from "path";
import * as vscode from "vscode";

import type { ProjectTexts } from "./messages";
import type { FileError } from "./pageDocument";

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
 * missing; an empty translation removes the text. The language server (`trust-lsp.hmiTranslation`)
 * checks the language and gives the file's new text; the file is saved unless it had unsaved
 * changes of its own. Gives the reasons when the file cannot be written, such as a file that does
 * not load.
 */
export async function writeTranslation(
  project: vscode.Uri,
  language: string,
  text: string,
  translation: string,
): Promise<FileError[]> {
  const translate = (current?: string) =>
    vscode.commands.executeCommand<{ ok?: boolean; text?: string; errors?: FileError[] } | undefined>(
      "trust-lsp.hmiTranslation",
      { language, text, translation, ...(current !== undefined ? { current } : {}) },
    );
  // Asked first without the file, so a language that is no tag never names a file to read.
  let answer = await translate();
  if (answer?.ok !== true || typeof answer.text !== "string") return answer?.errors ?? [NO_ANSWER];
  const file = vscode.Uri.joinPath(project, "hmi", "texts", `${language}.toml`);
  let document: vscode.TextDocument | undefined;
  try {
    document = await vscode.workspace.openTextDocument(file);
  } catch {
    document = undefined;
  }
  if (document) {
    answer = await translate(document.getText());
    if (answer?.ok !== true || typeof answer.text !== "string") return answer?.errors ?? [NO_ANSWER];
  }
  if (!document) {
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(project, "hmi", "texts"));
    await vscode.workspace.fs.writeFile(file, new TextEncoder().encode(answer.text));
    // Opened, so the checks of the Problems view see the new file.
    await vscode.workspace.openTextDocument(file);
    return [];
  }
  if (document.getText() === answer.text) return [];
  const wasDirty = document.isDirty;
  const edit = new vscode.WorkspaceEdit();
  edit.replace(file, new vscode.Range(0, 0, document.lineCount, 0), answer.text);
  if (!(await vscode.workspace.applyEdit(edit)))
    return [{ message: `hmi/texts/${language}.toml could not be changed.` }];
  if (!wasDirty) await document.save();
  return [];
}
