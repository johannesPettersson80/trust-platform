import * as vscode from "vscode";
import { asUri, pathExists } from "./uriUtils";

type ImportStatechartArgs = {
  sourceUri?: vscode.Uri | string;
  targetUri?: vscode.Uri | string;
  overwrite?: boolean;
  openAfterImport?: boolean;
};

export const IMPORT_STATECHART_COMMAND = "trust-lsp.statechart.import";

async function isValidStatechart(uri: vscode.Uri): Promise<boolean> {
  try {
    const content = await vscode.workspace.fs.readFile(uri);
    const text = Buffer.from(content).toString("utf8");
    const json = JSON.parse(text);

    return (
      typeof json === "object" &&
      json !== null &&
      "id" in json &&
      "states" in json
    );
  } catch {
    return false;
  }
}

async function promptForSourceFile(): Promise<vscode.Uri | undefined> {
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri;
  let defaultUri = workspaceRoot;

  if (workspaceRoot) {
    const examplesPath = vscode.Uri.joinPath(workspaceRoot, "examples/statecharts");
    if (await pathExists(examplesPath)) {
      defaultUri = examplesPath;
    }
  }

  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: true,
    canSelectFolders: false,
    canSelectMany: false,
    defaultUri,
    filters: {
      "Statechart Files": ["json"],
      "All Files": ["*"],
    },
    openLabel: "Select Statechart to Import",
  });

  return selected?.[0];
}

async function promptForTargetFolder(): Promise<vscode.Uri | undefined> {
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri;
  if (workspaceRoot) {
    return workspaceRoot;
  }

  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Select Destination Folder",
  });
  return selected?.[0];
}

async function confirmOverwrite(targetUri: vscode.Uri): Promise<boolean> {
  const selection = await vscode.window.showWarningMessage(
    `The file already exists: ${targetUri.fsPath}\nDo you want to overwrite it?`,
    { modal: true },
    "Overwrite",
    "Cancel"
  );
  return selection === "Overwrite";
}

async function copyStatechart(
  sourceUri: vscode.Uri,
  targetUri: vscode.Uri
): Promise<void> {
  const content = await vscode.workspace.fs.readFile(sourceUri);
  await vscode.workspace.fs.writeFile(targetUri, content);
}

function getStatechartFileName(sourceUri: vscode.Uri): string {
  return sourceUri.path.split("/").pop() ?? "imported.statechart.json";
}

async function resolveSourceAndTarget(
  args?: ImportStatechartArgs
): Promise<{ source: vscode.Uri; target: vscode.Uri } | undefined> {
  const sourceUri = asUri(args?.sourceUri) ?? (await promptForSourceFile());
  if (!sourceUri) {
    return undefined;
  }

  if (!(await pathExists(sourceUri))) {
    void vscode.window.showErrorMessage(`Source file not found: ${sourceUri.fsPath}`);
    return undefined;
  }

  if (!(await isValidStatechart(sourceUri))) {
    void vscode.window.showErrorMessage(
      "Invalid statechart file. Must be JSON with 'id' and 'states' properties."
    );
    return undefined;
  }

  const explicitTarget = asUri(args?.targetUri);
  if (explicitTarget) {
    return { source: sourceUri, target: explicitTarget };
  }

  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri;
  if (workspaceRoot && sourceUri.toString().startsWith(workspaceRoot.toString())) {
    // If source is already in workspace and no explicit target is provided,
    // import means "open this file in the statechart editor".
    return { source: sourceUri, target: sourceUri };
  }

  const targetFolder = await promptForTargetFolder();
  if (!targetFolder) {
    return undefined;
  }

  return {
    source: sourceUri,
    target: vscode.Uri.joinPath(targetFolder, getStatechartFileName(sourceUri)),
  };
}

export function registerImportStatechartCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      IMPORT_STATECHART_COMMAND,
      async (args?: ImportStatechartArgs) => {
        const resolved = await resolveSourceAndTarget(args);
        if (!resolved) {
          return false;
        }

        const { source, target } = resolved;
        const shouldCopy = source.toString() !== target.toString();

        if (shouldCopy && (await pathExists(target))) {
          const shouldOverwrite = args?.overwrite ?? (await confirmOverwrite(target));
          if (!shouldOverwrite) {
            return false;
          }
        }

        if (shouldCopy) {
          try {
            await copyStatechart(source, target);
            void vscode.window.showInformationMessage(
              `Statechart imported successfully: ${target.fsPath}`
            );
          } catch (error) {
            void vscode.window.showErrorMessage(
              `Failed to import statechart: ${error instanceof Error ? error.message : String(error)}`
            );
            return false;
          }
        }

        const openAfter = args?.openAfterImport ?? true;
        if (openAfter) {
          try {
            await vscode.commands.executeCommand(
              "vscode.openWith",
              target,
              "trust-lsp.statechartEditor"
            );
          } catch (error) {
            void vscode.window.showErrorMessage(
              `Failed to open statechart: ${error instanceof Error ? error.message : String(error)}`
            );
            return false;
          }
        }

        return true;
      }
    )
  );
}
