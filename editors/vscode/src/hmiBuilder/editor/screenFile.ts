// Changes to `hmi/screen.toml` made from the HMI Builder (specification 39, §14.1 item 2 for the
// project colours, §14.5 item 2 for the languages): through the language server's canonical
// parser and writer, saved when the file had no unsaved changes of its own.

import * as vscode from "vscode";

import { DEFAULT_SCREEN, ScreenModel } from "./messages";

type ScreenFileModel = Record<string, unknown> & { colors?: Record<string, Record<string, string>> };

/** Applies `change` to the screen layout's model; gives the new model, or why it was refused. */
async function editScreen(
  file: vscode.Uri,
  change: (model: ScreenFileModel) => ScreenFileModel,
): Promise<ScreenFileModel | string> {
  let document: vscode.TextDocument;
  try {
    document = await vscode.workspace.openTextDocument(file);
  } catch {
    return "The project has no hmi/screen.toml.";
  }
  const parsed = await vscode.commands.executeCommand<{
    ok?: boolean;
    model?: ScreenFileModel;
    errors?: Array<{ message: string }>;
  }>("trust-lsp.hmiFileParse", { kind: "screen", text: document.getText(), file: "hmi/screen.toml" });
  if (!parsed?.ok || !parsed.model) return parsed?.errors?.[0]?.message ?? "hmi/screen.toml does not load.";
  const model = change(parsed.model);
  const formatted = await vscode.commands.executeCommand<{
    ok?: boolean;
    text?: string;
    errors?: Array<{ message: string }>;
  }>("trust-lsp.hmiFileFormat", { kind: "screen", model });
  if (!formatted?.ok || typeof formatted.text !== "string")
    return formatted?.errors?.[0]?.message ?? "The screen settings are not valid.";
  const wasDirty = document.isDirty;
  const edit = new vscode.WorkspaceEdit();
  edit.replace(file, new vscode.Range(0, 0, document.lineCount, 0), formatted.text);
  if (!(await vscode.workspace.applyEdit(edit))) return "VS Code could not change hmi/screen.toml.";
  if (!wasDirty) await document.save();
  return model;
}

/** The screen layout the editor draws with, from a screen model. */
function screenOf(model: ScreenFileModel): ScreenModel {
  const size = model.size as [number, number] | undefined;
  return {
    size: size ?? DEFAULT_SCREEN.size,
    theme: (model.theme as "dark" | "light" | undefined) ?? "dark",
    ...(model.colors ? { colors: model.colors } : {}),
  };
}

/** Sets or removes one project colour (§14.1 item 2) and gives the new screen layout. */
export async function writeScreenColour(
  file: vscode.Uri,
  theme: "dark" | "light",
  token: string,
  value: string | undefined,
): Promise<ScreenModel | undefined> {
  const written = await editScreen(file, (model) => {
    const colors = { ...(model.colors ?? {}) };
    const table = { ...(colors[theme] ?? {}) };
    if (value === undefined) delete table[token];
    else table[token] = value;
    if (Object.keys(table).length > 0) colors[theme] = table;
    else delete colors[theme];
    const next: ScreenFileModel = { ...model, colors };
    if (Object.keys(colors).length === 0) delete next.colors;
    return next;
  });
  return typeof written === "string" ? undefined : screenOf(written);
}

/** Sets the project's languages, the first being the one its texts are written in (§14.5 item 2);
 * gives why they were refused, or `undefined`. The language server checks the list
 * (`trust-lsp.hmiChecks`). */
export async function writeScreenLanguages(file: vscode.Uri, languages: string[]): Promise<string | undefined> {
  const checked = await vscode.commands.executeCommand<{ ok?: boolean; error?: string; problem?: string }>(
    "trust-lsp.hmiChecks",
    { action: "languages", languages },
  );
  if (checked?.ok !== true) return checked?.error ?? "The truST language server is not running.";
  if (checked.problem !== undefined) return checked.problem;
  const written = await editScreen(file, (model) => ({ ...model, languages }));
  return typeof written === "string" ? written : undefined;
}
