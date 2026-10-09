// Opens a symbol's faceplate from the canvas's context menu (specification 39, §3.3 item 8).

import * as vscode from "vscode";

import { languageServerFaceplateUri } from "./languageServerPageFiles";

/** Opens the faceplate `name` of the project or its libraries in the HMI Builder; false when there is none. */
export async function openFaceplate(project: vscode.Uri, name: string): Promise<boolean> {
  const uri = await languageServerFaceplateUri(project, name);
  if (!uri) return false;
  await vscode.commands.executeCommand("vscode.openWith", uri, "trust-lsp.hmiBuilder");
  return true;
}
