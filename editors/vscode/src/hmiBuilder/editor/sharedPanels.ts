import * as vscode from "vscode";
import type { HmiPageModel } from "../drawing/model";
import type { FileError } from "./pageDocument";
import { openHmiTexts } from "./textFiles";

export async function sharedComposition(
  project: vscode.Uri,
  document: vscode.TextDocument,
): Promise<{ model?: HmiPageModel; errors?: FileError[] }> {
  const file = document.uri.path.slice(project.path.length + 1);
  if (!/^hmi\/pages\/[^/]+\.toml$/.test(file)) return {};
  const result = await vscode.commands.executeCommand<{ ok: boolean; model?: HmiPageModel; errors?: FileError[] }>(
    "trust-lsp.hmiFileParse",
    { kind: "composition", root_uri: project.toString(), file, text: document.getText(), open: openHmiTexts(project) },
  );
  return result?.ok
    ? { model: result.model }
    : { errors: result?.errors ?? [{ message: "Shared panels could not be read." }] };
}

export async function openSharedPanel(project: vscode.Uri, name: string): Promise<void> {
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(name)) return;
  await vscode.commands.executeCommand(
    "vscode.openWith",
    vscode.Uri.joinPath(project, "hmi", "panels", `${name}.toml`),
    "trust-lsp.hmiBuilder",
  );
}
