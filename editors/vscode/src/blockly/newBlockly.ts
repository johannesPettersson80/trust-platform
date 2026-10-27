import * as vscode from "vscode";
import { asUri } from "./uriUtils";
import { checkProgramName, createVisualProgram, programFolder } from "../visual/programFiles";

type NewBlocklyArgs = {
  targetUri?: vscode.Uri | string;
  blocklyName?: string;
  overwrite?: boolean;
};

export const NEW_BLOCKLY_COMMAND = "trust-lsp.blockly.new";

/** Creates a Blockly program; the name rule and template are the language server's. */
export async function newBlocklyCommand(args?: NewBlocklyArgs): Promise<vscode.Uri | undefined> {
  if (args?.targetUri && !asUri(args.targetUri)) {
    void vscode.window.showErrorMessage("Invalid target URI provided.");
    return undefined;
  }
  const folder = await programFolder(asUri(args?.targetUri));
  if (!folder) {
    return undefined;
  }
  const name =
    args?.blocklyName ??
    (await vscode.window.showInputBox({
      prompt: "Enter the name for the new Blockly program",
      placeHolder: "my-plc-program",
      validateInput: (value) => checkProgramName("blockly", value),
    }));
  if (!name) {
    return undefined;
  }
  try {
    return await createVisualProgram("blockly", folder, name, args?.overwrite ?? false);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(`Failed to create Blockly program: ${message}`);
    return undefined;
  }
}

export function registerNewBlocklyCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(vscode.commands.registerCommand(NEW_BLOCKLY_COMMAND, newBlocklyCommand));
}
