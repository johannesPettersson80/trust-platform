import * as vscode from "vscode";
import { asUri } from "./uriUtils";
import { importVisualProgram, programFolder } from "../visual/programFiles";

type ImportBlocklyArgs = {
  sourceUri?: vscode.Uri | string;
  /** The folder the copy goes into (the folder of a file). */
  targetUri?: vscode.Uri | string;
};

export const IMPORT_BLOCKLY_COMMAND = "trust-lsp.blockly.import";

export async function importBlocklyCommand(args?: ImportBlocklyArgs): Promise<vscode.Uri | undefined> {
  if (args?.sourceUri && !asUri(args.sourceUri)) {
    void vscode.window.showErrorMessage("Invalid source URI provided.");
    return undefined;
  }
  const targetFolder = args?.targetUri ? await programFolder(asUri(args.targetUri)) : undefined;
  return importVisualProgram({
    kind: "blockly",
    noun: "Blockly program",
    examplesFolder: "examples/blockly",
    filters: { "Blockly Files": ["json"], "All Files": ["*"] },
    source: asUri(args?.sourceUri),
    targetFolder,
    openInPlace: false,
  });
}

export function registerImportBlocklyCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(vscode.commands.registerCommand(IMPORT_BLOCKLY_COMMAND, importBlocklyCommand));
}
