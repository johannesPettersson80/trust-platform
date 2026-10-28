// Generate Process Screen from P&ID (specification 39, §18 phase 4 item 3): the language server
// (`trust-lsp.hmiPidSync`) makes the page linked to the P&ID, with its title, its process objects
// and their pipes, placed as on the P&ID scaled to the screen's design size and bound as on the
// P&ID; this command writes it when the project has no such page, and opens it in the builder.

import * as path from "path";
import * as vscode from "vscode";

import { languageServerScreen } from "./languageServerPageFiles";
import { RUST_RULES } from "./messages";
import { projectPath } from "./pidSync";

export const GENERATE_PROCESS_SCREEN_COMMAND = "trust-lsp.hmiBuilder.generateProcessScreen";

const OPEN_A_PID = "Open a P&ID (hmi/pid/<name>.toml) to generate its process screen.";

/** What the command may be given instead of taking the open P&ID. */
interface GenerateArgs {
  pid?: string;
}

/** What `processScreen` answers: the page's name, file and text, or why there is none. */
interface ProcessScreen {
  ok?: boolean;
  error?: string;
  name?: string;
  file?: string;
  text?: string;
}

export function registerProcessScreen(): vscode.Disposable {
  return vscode.commands.registerCommand(GENERATE_PROCESS_SCREEN_COMMAND, async (args?: GenerateArgs) =>
    generate(args ?? {}),
  );
}

async function generate(args: GenerateArgs): Promise<void> {
  const pid = args.pid ? vscode.Uri.file(args.pid) : activeFile();
  if (!pid) {
    void vscode.window.showErrorMessage(OPEN_A_PID);
    return;
  }
  // A P&ID is `<project>/hmi/pid/<name>.toml`; the language server says whether this file is one.
  const project = vscode.Uri.file(path.dirname(path.dirname(path.dirname(pid.fsPath))));
  let text: string;
  try {
    text = Buffer.from(await vscode.workspace.fs.readFile(pid)).toString("utf8");
  } catch {
    void vscode.window.showErrorMessage(OPEN_A_PID);
    return;
  }
  // The P&ID is drawn at the screen's size too, so its places carry over as they are.
  const { size } = await languageServerScreen(vscode.Uri.joinPath(project, "hmi", "screen.toml"));
  let answer: ProcessScreen | undefined;
  try {
    answer = await vscode.commands.executeCommand<ProcessScreen>(RUST_RULES.pidSync, {
      action: "processScreen",
      file: projectPath(project, pid),
      text,
      size,
    });
  } catch (error) {
    answer = { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
  if (answer?.ok !== true || answer.file === undefined || answer.text === undefined) {
    void vscode.window.showErrorMessage(answer?.error ?? "The truST language server is not running yet.");
    return;
  }
  const page = vscode.Uri.joinPath(project, ...answer.file.split("/"));
  if (await exists(page)) {
    void vscode.window.showErrorMessage(`The project has a page "${answer.name}" already: ${page.fsPath}`);
    return;
  }
  await vscode.workspace.fs.createDirectory(vscode.Uri.file(path.dirname(page.fsPath)));
  await vscode.workspace.fs.writeFile(page, Buffer.from(answer.text, "utf8"));
  await vscode.commands.executeCommand("vscode.openWith", page, "trust-lsp.hmiBuilder");
}

/** The file shown in the active editor, if any. */
function activeFile(): vscode.Uri | undefined {
  const input = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
  return input instanceof vscode.TabInputCustom || input instanceof vscode.TabInputText ? input.uri : undefined;
}

async function exists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}
