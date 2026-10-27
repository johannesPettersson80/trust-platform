import * as vscode from "vscode";
import { importVisualProgram } from "../visual/programFiles";

type ImportLadderArgs = {
  sourceUri?: vscode.Uri | string;
  targetUri?: vscode.Uri | string;
  overwrite?: boolean;
  openAfterImport?: boolean;
};

export const IMPORT_LADDER_COMMAND = "trust-lsp.ladder.import";

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

export function registerImportLadderCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(IMPORT_LADDER_COMMAND, async (args?: ImportLadderArgs) => {
      const imported = await importVisualProgram({
        kind: "ladder",
        noun: "Ladder program",
        examplesFolder: "examples/ladder",
        filters: { "Ladder Files": ["json"], "All Files": ["*"] },
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
