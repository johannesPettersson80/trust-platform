// The project's runtime.toml as the editor changes it (specification 39, §12.8): the runtime
// display's users and write policy are appended or removed as text, so the rest of the file and
// its comments stay as they are.

import * as vscode from "vscode";

/** The sections of runtime.toml's text, each from its header line to the next. */
export interface TomlSection {
  /** The header as written, `[[runtime.web.hmi.user]]`; empty for the lines before the first. */
  header: string;
  /** Its first line and the line after its last. */
  start: number;
  end: number;
}

/** The sections of a TOML text by their header lines. */
export function sectionsOf(lines: string[]): TomlSection[] {
  const sections: TomlSection[] = [{ header: "", start: 0, end: lines.length }];
  lines.forEach((line, index) => {
    const header = /^\s*(\[\[?\s*[^\]]+?\s*\]\]?)\s*(#.*)?$/.exec(line);
    if (!header) return;
    sections[sections.length - 1].end = index;
    sections.push({ header: header[1].replace(/\s+/g, ""), start: index, end: lines.length });
  });
  return sections;
}

/** Changes the project's runtime.toml through `change` and saves it unless it had unsaved
 * changes of the user's own; answers whether it changed. */
export async function editRuntimeToml(root: vscode.Uri, change: (text: string) => string): Promise<boolean> {
  const file = vscode.Uri.joinPath(root, "runtime.toml");
  let document: vscode.TextDocument;
  try {
    document = await vscode.workspace.openTextDocument(file);
  } catch {
    throw new Error("The project has no runtime.toml to hold the runtime display's settings.");
  }
  const dirty = document.isDirty;
  const text = document.getText();
  const next = change(text);
  if (next === text) return false;
  const edit = new vscode.WorkspaceEdit();
  edit.replace(file, new vscode.Range(document.positionAt(0), document.positionAt(text.length)), next);
  if (!(await vscode.workspace.applyEdit(edit))) throw new Error("VS Code could not change runtime.toml.");
  // runtime.toml usually has no editor open, so an unsaved change would be lost (as Create PLC
  // code saves); one with unsaved changes of the user's own stays unsaved for the user to review.
  if (!dirty) await document.save();
  return true;
}

/** `text` with `addition` appended on a line of its own. */
export function appended(text: string, addition: string): string {
  return `${text}${/\n$/.test(text) || text === "" ? "" : "\n"}${addition}`;
}
