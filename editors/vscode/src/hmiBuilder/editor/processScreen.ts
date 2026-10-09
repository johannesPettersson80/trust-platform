// Generate Process Screen from P&ID (specification 39, §18 phase 4 item 3): a page linked to the
// P&ID, with its title, its process objects and their pipes, placed as on the P&ID scaled to the
// screen's design size and bound as on the P&ID; it opens in the builder.

import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
import { languageServerPageFiles, languageServerScreen } from "./languageServerPageFiles";
import { linkedUpdate } from "./pidLink";
import { isPidFile } from "./pidSync";

export const GENERATE_PROCESS_SCREEN_COMMAND = "trust-lsp.hmiBuilder.generateProcessScreen";

/** What the command may be given instead of taking the open P&ID. */
interface GenerateArgs {
  pid?: string;
}

export function registerProcessScreen(): vscode.Disposable {
  return vscode.commands.registerCommand(GENERATE_PROCESS_SCREEN_COMMAND, async (args?: GenerateArgs) =>
    generate(args ?? {}),
  );
}

async function generate(args: GenerateArgs): Promise<void> {
  const pid = args.pid ? vscode.Uri.file(args.pid) : openPid();
  if (!pid || !isPidFile(pid)) {
    void vscode.window.showErrorMessage("Open a P&ID (hmi/pid/<name>.toml) to generate its process screen.");
    return;
  }
  const name = path.basename(pid.fsPath, ".toml");
  const project = vscode.Uri.file(path.dirname(path.dirname(path.dirname(pid.fsPath))));
  const page = vscode.Uri.joinPath(project, "hmi", "pages", `${name}.toml`);
  if (await exists(page)) {
    void vscode.window.showErrorMessage(`The project has a page "${name}" already: ${page.fsPath}`);
    return;
  }
  const read = await languageServerPageFiles.read(
    Buffer.from(await vscode.workspace.fs.readFile(pid)).toString("utf8"),
    `hmi/pid/${name}.toml`,
  );
  if (!read.ok) {
    void vscode.window.showErrorMessage(
      `The P&ID does not load: ${read.errors.map((error) => `line ${error.line}: ${error.message}`).join("; ")}`,
    );
    return;
  }
  // The P&ID is drawn at the screen's size too, so its places carry over as they are.
  const { size } = await languageServerScreen(vscode.Uri.joinPath(project, "hmi", "screen.toml"));
  const empty: HmiPageModel = { title: read.model.title, pid: name, object: [], pipe: [] };
  const written = await languageServerPageFiles.write(linkedUpdate(read.model, size, empty, size) ?? empty);
  if (!written.ok) {
    void vscode.window.showErrorMessage(
      `The process screen cannot be written: ${written.errors.map((error) => error.message).join("; ")}`,
    );
    return;
  }
  await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(project, "hmi", "pages"));
  await vscode.workspace.fs.writeFile(page, Buffer.from(written.text, "utf8"));
  await vscode.commands.executeCommand("vscode.openWith", page, "trust-lsp.hmiBuilder");
}

/** The P&ID shown in the active editor, if any. */
function openPid(): vscode.Uri | undefined {
  const input = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
  const uri = input instanceof vscode.TabInputCustom || input instanceof vscode.TabInputText ? input.uri : undefined;
  return uri && isPidFile(uri) ? uri : undefined;
}

async function exists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}
