import * as vscode from "vscode";

import { projectRequest } from "../projectRequest";
import type { ExampleEntry } from "./model";

/**
 * Adapts the README of a copied lesson (specification 25 §3.1): the language server (`trust-lsp.copyExample`) gives
 * each link to a sibling lesson of the gallery the words that open it from the gallery, because a copied project has
 * no sibling folders. External, anchor and unresolved links stay as authored.
 */
export async function adaptCopiedLessonReadme(
  context: Pick<vscode.ExtensionContext, "extensionUri">,
  entry: ExampleEntry,
  destination: vscode.Uri
): Promise<void> {
  const answer = await projectRequest<{ readonly changed: boolean }>("trust-lsp.copyExample", {
    action: "readme",
    root: vscode.Uri.joinPath(context.extensionUri, "media", "examples").fsPath,
    entryPath: entry.path,
    destination: destination.fsPath,
  });
  if (!answer.ok) {
    throw new Error(answer.error);
  }
}
