import * as vscode from "vscode";
import { asUri } from "./uriUtils";
import { importVisualProgram } from "../visual/programFiles";

type ImportStatechartArgs = {
  sourceUri?: vscode.Uri | string;
  targetUri?: vscode.Uri | string;
  overwrite?: boolean;
  openAfterImport?: boolean;
};

export const IMPORT_STATECHART_COMMAND = "trust-lsp.statechart.import";

export function registerImportStatechartCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(IMPORT_STATECHART_COMMAND, async (args?: ImportStatechartArgs) => {
      const imported = await importVisualProgram({
        kind: "statechart",
        noun: "statechart",
        examplesFolder: "examples/statecharts",
        filters: { "Statechart Files": ["json"], "All Files": ["*"] },
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
