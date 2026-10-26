import * as vscode from "vscode";
import { asUri, pathExists } from "./uriUtils";
import {
  openCompanionForVisualSource,
  openCompanionOnCreateEnabled,
  syncVisualCompanionFromUri,
} from "../visual/companionSt";

type NewBlocklyArgs = {
  targetUri?: vscode.Uri | string;
  blocklyName?: string;
  overwrite?: boolean;
};

export const NEW_BLOCKLY_COMMAND = "trust-lsp.blockly.new";
const BLOCKLY_EDITOR_VIEW_TYPE = "trust-lsp.blockly.editor";

const BLOCKLY_TEMPLATE = `{
  "blocks": {
    "languageVersion": 0,
    "blocks": [
      {
        "type": "controls_if",
        "id": "initial_block",
        "x": 50,
        "y": 50
      }
    ]
  },
  "variables": [],
  "metadata": {
    "name": "{{name}}",
    "description": "PLC program created with Blockly",
    "version": "1.0.0"
  }
}
`;

function validateBlocklyName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "Blockly program name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "Blockly program name must not contain path separators.";
  }
  if (trimmed === "." || trimmed === "..") {
    return "Blockly program name is invalid.";
  }
  if (!/^[a-zA-Z0-9_-]+$/.test(trimmed)) {
    return "Blockly program name must only contain letters, numbers, hyphens, and underscores.";
  }
  return undefined;
}

export async function newBlocklyCommand(
  args?: NewBlocklyArgs
): Promise<vscode.Uri | undefined> {
  const outputChannel = vscode.window.createOutputChannel("Blockly Editor");

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

    // Get blockly program name
    let blocklyName: string;
    if (args?.blocklyName) {
      blocklyName = args.blocklyName;
      const validation = validateBlocklyName(blocklyName);
      if (validation) {
        vscode.window.showErrorMessage(validation);
        return undefined;
      }
    } else {
      const input = await vscode.window.showInputBox({
        prompt: "Enter the name for the new Blockly program",
        placeHolder: "my-plc-program",
        validateInput: validateBlocklyName,
      });

      if (!input) {
        outputChannel.appendLine("Blockly creation cancelled by user.");
        return undefined;
      }
      blocklyName = input.trim();
    }

    // Construct file path
    const fileName = `${blocklyName}.blockly.json`;
    const fileUri = vscode.Uri.joinPath(targetUri, fileName);

    // Check if file already exists
    if (await pathExists(fileUri)) {
      if (!args?.overwrite) {
        const overwrite = await vscode.window.showWarningMessage(
          `File "${fileName}" already exists. Do you want to overwrite it?`,
          { modal: true },
          "Overwrite",
          "Cancel"
        );
        if (overwrite !== "Overwrite") {
          outputChannel.appendLine("File already exists. Operation cancelled.");
          return undefined;
        }
      }
    }

    // Create blockly file
    const content = BLOCKLY_TEMPLATE.replace(/\{\{name\}\}/g, blocklyName);
    await vscode.workspace.fs.writeFile(
      fileUri,
      Buffer.from(content, "utf8")
    );

    outputChannel.appendLine(`✓ Created Blockly program: ${fileUri.fsPath}`);
    outputChannel.show(true);

    const companionUri = await syncVisualCompanionFromUri(fileUri, {
      force: true,
      showErrors: true,
    });

    await vscode.commands.executeCommand(
      "vscode.openWith",
      fileUri,
      BLOCKLY_EDITOR_VIEW_TYPE,
      { preview: false }
    );
    if (openCompanionOnCreateEnabled(fileUri) && companionUri) {
      await openCompanionForVisualSource(fileUri, { beside: true });
    }

    return fileUri;
  } catch (error) {
    const errorMessage =
      error instanceof Error ? error.message : String(error);
    outputChannel.appendLine(`✗ Error creating Blockly program: ${errorMessage}`);
    outputChannel.show(true);
    vscode.window.showErrorMessage(
      `Failed to create Blockly program: ${errorMessage}`
    );
    return undefined;
  }
}

export function registerNewBlocklyCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      NEW_BLOCKLY_COMMAND,
      newBlocklyCommand
    )
  );
}
