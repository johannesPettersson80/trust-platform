import * as vscode from "vscode";
import { checkProgramName, createVisualProgram, programFolder } from "../visual/programFiles";

type NewLadderArgs = {
  targetUri?: vscode.Uri | string;
  ladderName?: string;
  overwrite?: boolean;
};

export const NEW_LADDER_COMMAND = "trust-lsp.ladder.new";

function asUri(target: vscode.Uri | string | undefined): vscode.Uri | undefined {
  if (!target) {
    return undefined;
  }
  if (typeof target === "string") {
    try {
      return vscode.Uri.parse(target);
    } catch {
      return undefined;
    }
  }
  return target;
}

/** Creates a Ladder program; the name rule and template are the language server's. */
export async function newLadderCommand(args?: NewLadderArgs): Promise<vscode.Uri | undefined> {
  if (args?.targetUri && !asUri(args.targetUri)) {
    void vscode.window.showErrorMessage("Invalid target URI provided.");
    return undefined;
  }
  const folder = await programFolder(asUri(args?.targetUri));
  if (!folder) {
    return undefined;
  }
  const name =
    args?.ladderName ??
    (await vscode.window.showInputBox({
      prompt: "Enter the name for the new Ladder Logic program",
      placeHolder: "my-ladder-program",
      validateInput: (value) => checkProgramName("ladder", value),
    }));
  if (!name) {
    return undefined;
  }
  try {
    return await createVisualProgram("ladder", folder, name, args?.overwrite ?? false);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(`Failed to create Ladder program: ${message}`);
    return undefined;
  }
}

export function registerNewLadderCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(vscode.commands.registerCommand(NEW_LADDER_COMMAND, newLadderCommand));
}
