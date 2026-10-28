import * as vscode from "vscode";
import { checkProgramName, createVisualProgram, programFolder } from "../visual/programFiles";

type NewSfcArgs = {
  targetUri?: vscode.Uri | string;
  sfcName?: string;
  overwrite?: boolean;
};

export const NEW_SFC_COMMAND = "trust-lsp.sfc.new";

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

/** Creates an SFC program; the name rule and template are the language server's. */
export async function newSfcCommand(args?: NewSfcArgs): Promise<vscode.Uri | undefined> {
  if (args?.targetUri && !asUri(args.targetUri)) {
    void vscode.window.showErrorMessage("Invalid target URI provided.");
    return undefined;
  }
  const folder = await programFolder(asUri(args?.targetUri));
  if (!folder) {
    return undefined;
  }
  const name =
    args?.sfcName ??
    (await vscode.window.showInputBox({
      prompt: "Enter the name for the new SFC program",
      placeHolder: "my-sfc-program",
      validateInput: (value) => checkProgramName("sfc", value),
    }));
  if (!name) {
    return undefined;
  }
  try {
    return await createVisualProgram("sfc", folder, name, args?.overwrite ?? false);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(`Failed to create SFC: ${message}`);
    return undefined;
  }
}
