import * as vscode from "vscode";
import {
  openCompanionForVisualSource,
  openCompanionOnCreateEnabled,
  syncVisualCompanionFromUri,
} from "../visual/companionSt";

type NewLadderArgs = {
  targetUri?: vscode.Uri | string;
  ladderName?: string;
  overwrite?: boolean;
};

export const NEW_LADDER_COMMAND = "trust-lsp.ladder.new";
const LADDER_EDITOR_VIEW_TYPE = "trust-lsp.ladder.editor";

const LADDER_TEMPLATE = `{
  "schemaVersion": 2,
  "networks": [
    {
      "id": "rung_1",
      "order": 0,
      "nodes": [
        {
          "id": "contact_1",
          "type": "contact",
          "contactType": "NO",
          "variable": "Input1",
          "position": {
            "x": 100,
            "y": 100
          }
        },
        {
          "id": "coil_1",
          "type": "coil",
          "coilType": "NORMAL",
          "variable": "Output1",
          "position": {
            "x": 300,
            "y": 100
          }
        }
      ],
      "edges": [
        {
          "id": "edge_1",
          "fromNodeId": "contact_1",
          "toNodeId": "coil_1"
        }
      ],
      "layout": {
        "y": 100
      }
    }
  ],
  "variables": [
    {
      "name": "Input1",
      "type": "BOOL",
      "initialValue": "FALSE"
    },
    {
      "name": "Output1",
      "type": "BOOL",
      "initialValue": "FALSE"
    }
  ],
  "metadata": {
    "name": "{{name}}",
    "description": "Ladder Logic program",
    "version": "1.0.0",
    "created": "{{created}}"
  }
}
`;

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

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

function validateLadderName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "Ladder program name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "Ladder program name must not contain path separators.";
  }
  if (trimmed === "." || trimmed === "..") {
    return "Ladder program name is invalid.";
  }
  if (!/^[a-zA-Z0-9_-]+$/.test(trimmed)) {
    return "Ladder program name must only contain letters, numbers, hyphens, and underscores.";
  }
  return undefined;
}

export async function newLadderCommand(
  args?: NewLadderArgs
): Promise<vscode.Uri | undefined> {
  const outputChannel = vscode.window.createOutputChannel("Ladder Editor");

  try {
    // Determine target directory
    let targetUri: vscode.Uri;
    if (args?.targetUri) {
      const uri = asUri(args.targetUri);
      if (!uri) {
        vscode.window.showErrorMessage("Invalid target URI provided.");
        return undefined;
      }
      targetUri = uri;
    } else {
      const workspaceFolders = vscode.workspace.workspaceFolders;
      if (!workspaceFolders || workspaceFolders.length === 0) {
        vscode.window.showErrorMessage(
          "No workspace folder open. Please open a folder first."
        );
        return undefined;
      }
      targetUri = workspaceFolders[0].uri;
    }

    // Ensure target is a directory
    const stat = await vscode.workspace.fs.stat(targetUri);
    if (stat.type !== vscode.FileType.Directory) {
      targetUri = vscode.Uri.joinPath(targetUri, "..");
    }

    // Get ladder program name
    let ladderName: string;
    if (args?.ladderName) {
      ladderName = args.ladderName;
      const validation = validateLadderName(ladderName);
      if (validation) {
        vscode.window.showErrorMessage(validation);
        return undefined;
      }
    } else {
      const input = await vscode.window.showInputBox({
        prompt: "Enter the name for the new Ladder Logic program",
        placeHolder: "my-ladder-program",
        validateInput: validateLadderName,
      });

      if (!input) {
        return undefined;
      }
      ladderName = input.trim();
    }

    // Create file path
    const fileName = `${ladderName}.ladder.json`;
    const fileUri = vscode.Uri.joinPath(targetUri, fileName);

    // Check if file exists
    let exists = false;
    try {
      await vscode.workspace.fs.stat(fileUri);
      exists = true;
    } catch {
      // File doesn't exist, which is fine
    }

    if (exists && !args?.overwrite) {
      const overwrite = await vscode.window.showWarningMessage(
        `File ${fileName} already exists. Do you want to overwrite it?`,
        "Overwrite",
        "Cancel"
      );
      if (overwrite !== "Overwrite") {
        return undefined;
      }
    }

    // Create file content
    const content = LADDER_TEMPLATE.replace("{{name}}", ladderName).replace(
      "{{created}}",
      new Date().toISOString()
    );

    // Write file
    await vscode.workspace.fs.writeFile(
      fileUri,
      Buffer.from(content, "utf-8")
    );

    outputChannel.appendLine(`Created Ladder file: ${fileUri.fsPath}`);

    // Open the program in its visual editor
    await vscode.commands.executeCommand(
      "vscode.openWith",
      fileUri,
      LADDER_EDITOR_VIEW_TYPE,
      { preview: false }
    );

    // Sync companion ST file if enabled and open it beside the visual editor
    if (openCompanionOnCreateEnabled(fileUri)) {
      try {
        const companionUri = await syncVisualCompanionFromUri(fileUri);
        if (companionUri) {
          await openCompanionForVisualSource(fileUri, { beside: true });
        }
      } catch (error) {
        outputChannel.appendLine(
          `Warning: Failed to create companion ST file: ${error}`
        );
      }
    }

    vscode.window.showInformationMessage(
      `Ladder Logic program "${ladderName}" created successfully.`
    );

    return fileUri;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    outputChannel.appendLine(`Error creating Ladder program: ${message}`);
    vscode.window.showErrorMessage(`Failed to create Ladder program: ${message}`);
    return undefined;
  }
}

export function registerNewLadderCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      NEW_LADDER_COMMAND,
      newLadderCommand
    )
  );
}
