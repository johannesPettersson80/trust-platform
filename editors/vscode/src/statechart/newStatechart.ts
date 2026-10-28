import * as path from "path";
import * as vscode from "vscode";
import { asUri } from "./uriUtils";
import { checkProgramName, createVisualProgram, programFolder } from "../visual/programFiles";

type NewStatechartArgs = {
  /** The new file itself, or the folder it goes into. */
  targetUri?: vscode.Uri | string;
  statechartName?: string;
  overwrite?: boolean;
};

export const NEW_STATECHART_COMMAND = "trust-lsp.statechart.new";

/** Creates a UML Statechart; the name rule and template are the language server's. */
async function newStatechartCommand(args?: NewStatechartArgs): Promise<vscode.Uri | undefined> {
  const target = asUri(args?.targetUri);
  if (args?.targetUri && !target) {
    void vscode.window.showErrorMessage("Invalid target URI provided.");
    return undefined;
  }
  let folder: vscode.Uri | undefined;
  if (target || vscode.workspace.workspaceFolders?.length) {
    folder = await programFolder(target);
  } else {
    const selected = await vscode.window.showOpenDialog({
      canSelectFiles: false,
      canSelectFolders: true,
      canSelectMany: false,
      openLabel: "Select Folder for New Statechart",
    });
    folder = selected?.[0];
  }
  if (!folder) {
    return undefined;
  }
  // A target file names the statechart; its suffix is cut by the language server's name rule.
  const targetIsFile = !!target && folder.fsPath !== target.fsPath;
  const name =
    args?.statechartName ??
    (targetIsFile && target
      ? path.basename(target.fsPath)
      : await vscode.window.showInputBox({
          prompt: "Enter a name for the new UML Statechart",
          placeHolder: "my-statechart",
          validateInput: (value) => checkProgramName("statechart", value),
        }));
  if (!name) {
    return undefined;
  }
  try {
    return await createVisualProgram("statechart", folder, name, args?.overwrite ?? false);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(`Failed to create statechart: ${message}`);
    return undefined;
  }
}

export function registerNewStatechartCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(vscode.commands.registerCommand(NEW_STATECHART_COMMAND, newStatechartCommand));
}
