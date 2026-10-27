import * as vscode from "vscode";
import { importVisualProgram } from "../visual/programFiles";

type ImportSfcArgs = {
  sourceUri?: vscode.Uri | string;
  targetUri?: vscode.Uri | string;
  overwrite?: boolean;
  openAfterImport?: boolean;
};

export const IMPORT_SFC_COMMAND = "trust-lsp.sfc.import";

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

export function registerImportSfcCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(IMPORT_SFC_COMMAND, async (args?: ImportSfcArgs) => {
      const imported = await importVisualProgram({
        kind: "sfc",
        noun: "SFC",
        examplesFolder: "examples/sfc",
        filters: { "SFC Files": ["sfc", "json"], "All Files": ["*"] },
        source: asUri(args?.sourceUri),
        targetFile: asUri(args?.targetUri),
        openInPlace: true,
        overwrite: args?.overwrite,
        openAfterImport: args?.openAfterImport,
      });
      return imported !== undefined;
    })
  );
}
