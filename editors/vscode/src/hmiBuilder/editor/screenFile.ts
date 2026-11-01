// Changes to `hmi/screen.toml` made from the HMI Builder (specification 39, §14.1 item 2 for the
// project colours, §14.5 item 2 for the languages): the language server writes the new text with
// its canonical parser and writer (a colour: `trust-lsp.hmiScreenColour`), and the file is saved
// when it had no unsaved changes of its own.

import * as vscode from "vscode";

import { DEFAULT_SCREEN, ScreenModel } from "./messages";

type ScreenFileModel = Record<string, unknown> & { colors?: Record<string, Record<string, string>> };

/** The screen layout's text and model after an edit, or why it was refused. */
type ScreenEdit = { text: string; model: ScreenFileModel } | string;

/** Writes the text that `edit` makes of the screen layout's text; gives the new model, or why it was refused. */
async function editScreen(
  file: vscode.Uri,
  edit: (text: string) => Promise<ScreenEdit>,
): Promise<ScreenFileModel | string> {
  let document: vscode.TextDocument;
  try {
    document = await vscode.workspace.openTextDocument(file);
  } catch {
    return "The project has no hmi/screen.toml.";
  }
  const edited = await edit(document.getText());
  if (typeof edited === "string") return edited;
  const wasDirty = document.isDirty;
  const change = new vscode.WorkspaceEdit();
  change.replace(file, new vscode.Range(0, 0, document.lineCount, 0), edited.text);
  if (!(await vscode.workspace.applyEdit(change))) return "VS Code could not change hmi/screen.toml.";
  if (!wasDirty) await document.save();
  return edited.model;
}

/** The screen layout's `text` with `change` applied to its model, through the canonical parser and writer. */
async function changedModel(text: string, change: (model: ScreenFileModel) => ScreenFileModel): Promise<ScreenEdit> {
  const parsed = await vscode.commands.executeCommand<{
    ok?: boolean;
    model?: ScreenFileModel;
    errors?: Array<{ message: string }>;
  }>("trust-lsp.hmiFileParse", { kind: "screen", text, file: "hmi/screen.toml" });
  if (!parsed?.ok || !parsed.model) return parsed?.errors?.[0]?.message ?? "hmi/screen.toml does not load.";
  const model = change(parsed.model);
  const formatted = await vscode.commands.executeCommand<{
    ok?: boolean;
    text?: string;
    errors?: Array<{ message: string }>;
  }>("trust-lsp.hmiFileFormat", { kind: "screen", model });
  if (!formatted?.ok || typeof formatted.text !== "string")
    return formatted?.errors?.[0]?.message ?? "The screen settings are not valid.";
  return { text: formatted.text, model };
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
  const written = await editScreen(file, async (text) => {
    const answer = await vscode.commands.executeCommand<{
      ok?: boolean;
      text?: string;
      model?: ScreenFileModel;
      error?: string;
    }>("trust-lsp.hmiScreenColour", { text, theme, token, value });
    return answer?.ok === true && typeof answer.text === "string" && answer.model
      ? { text: answer.text, model: answer.model }
      : (answer?.error ?? "The truST language server is not running.");
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
  const written = await editScreen(file, (text) => changedModel(text, (model) => ({ ...model, languages })));
  return typeof written === "string" ? written : undefined;
}
