// Open HMI (specification 39, §21.3 item 8): the truST sidebar's HMI destination. It opens the
// project's start page in the HMI Builder, or starts New HMI Screen for a project without screens.

import * as vscode from "vscode";

import { HMI_BUILDER_VIEW_TYPE } from "./hmiBuilderEditor";
import { exists, NEW_SCREEN_COMMAND, projectFolder } from "./templates";

export const OPEN_HMI_COMMAND = "trust-lsp.hmiBuilder.open";

/** The project's screen layout file. */
function screenFile(folder: vscode.Uri): vscode.Uri {
  return vscode.Uri.joinPath(folder, "hmi", "screen.toml");
}

/** The page the project's screens start at: the screen's `start_page`, else its first page by
 * name, else the screen layout itself; `undefined` for a project without screens. */
export async function startPage(folder: vscode.Uri): Promise<vscode.Uri | undefined> {
  let text: string;
  try {
    text = Buffer.from(await vscode.workspace.fs.readFile(screenFile(folder))).toString("utf8");
  } catch {
    return undefined;
  }
  const pages = vscode.Uri.joinPath(folder, "hmi", "pages");
  // The screen layout as the language server reads it (`trust-lsp.hmiFileParse`, §4).
  const parsed = await vscode.commands
    .executeCommand<{ ok?: boolean; model?: { start_page?: unknown } }>("trust-lsp.hmiFileParse", {
      kind: "screen",
      text,
      file: "hmi/screen.toml",
    })
    .then(
      (answer) => answer,
      () => undefined,
    );
  const named = typeof parsed?.model?.start_page === "string" ? parsed.model.start_page : undefined;
  if (named) {
    const page = vscode.Uri.joinPath(pages, `${named}.toml`);
    if (await exists(page)) return page;
  }
  let entries: [string, vscode.FileType][] = [];
  try {
    entries = await vscode.workspace.fs.readDirectory(pages);
  } catch {
    return screenFile(folder);
  }
  const first = entries
    .filter(([name, type]) => type === vscode.FileType.File && name.endsWith(".toml"))
    .map(([name]) => name)
    .sort()[0];
  return first ? vscode.Uri.joinPath(pages, first) : screenFile(folder);
}

/** Opens the project's start page, or creates the project's screens. */
export async function openHmi(folder: vscode.Uri | undefined): Promise<void> {
  if (!folder) {
    void vscode.window.showErrorMessage("Open a truST project folder first.");
    return;
  }
  const page = await startPage(folder);
  if (page) {
    await vscode.commands.executeCommand("vscode.openWith", page, HMI_BUILDER_VIEW_TYPE);
  } else {
    await vscode.commands.executeCommand(NEW_SCREEN_COMMAND, { folder: folder.toString() });
  }
}

export function registerOpenHmi(): vscode.Disposable {
  return vscode.commands.registerCommand(OPEN_HMI_COMMAND, (args?: { folder?: string }) =>
    openHmi(args?.folder ? vscode.Uri.parse(args.folder) : projectFolder()),
  );
}
