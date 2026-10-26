import * as vscode from "vscode";
import {
  openCompanionForVisualSource,
  openCompanionOnCreateEnabled,
  syncVisualCompanionFromUri,
} from "../visual/companionSt";

type NewSfcArgs = {
  targetUri?: vscode.Uri | string;
  sfcName?: string;
  overwrite?: boolean;
};

export const NEW_SFC_COMMAND = "trust-lsp.sfc.new";
const SFC_EDITOR_VIEW_TYPE = "trust-lsp.sfc.editor";

const SFC_TEMPLATE = `{
  "name": "{{name}}",
  "steps": [
    {
      "id": "step_init",
      "name": "Init",
      "initial": true,
      "x": 200,
      "y": 50,
      "actions": []
    },
    {
      "id": "step_1",
      "name": "Step1",
      "x": 200,
      "y": 150,
      "actions": []
    }
  ],
  "transitions": [
    {
      "id": "trans_1",
      "name": "T1",
      "condition": "TRUE",
      "sourceStepId": "step_init",
      "targetStepId": "step_1"
    }
  ],
  "variables": [],
  "metadata": {
    "author": "",
    "version": "1.0.0",
    "description": "Sequential Function Chart program",
    "created": "{{created}}"
  }
}
`;

function asUri(target: vscode.Uri | string): vscode.Uri | undefined {
  if (typeof target === "string") {
    try {
      return vscode.Uri.parse(target);
    } catch {
      return undefined;
    }
  }
  return target;
}

function validateSfcName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "SFC program name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "SFC program name must not contain path separators.";
  }
  if (trimmed === "." || trimmed === "..") {
    return "SFC program name is invalid.";
  }
  if (!/^[a-zA-Z0-9_-]+$/.test(trimmed)) {
    return "SFC program name must only contain letters, numbers, hyphens, and underscores.";
  }
  return undefined;
}

export async function newSfcCommand(
  args?: NewSfcArgs
): Promise<vscode.Uri | undefined> {
  const outputChannel = vscode.window.createOutputChannel("SFC Editor");

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

    // Get SFC program name
    let sfcName: string;
    if (args?.sfcName) {
      sfcName = args.sfcName;
      const validation = validateSfcName(sfcName);
      if (validation) {
        vscode.window.showErrorMessage(validation);
        return undefined;
      }
    } else {
      const input = await vscode.window.showInputBox({
        prompt: "Enter the name for the new SFC program",
        placeHolder: "my-sfc-program",
        validateInput: validateSfcName,
      });

      if (!input) {
        return undefined;
      }
      sfcName = input.trim();
    }

    // Create file path
    const fileName = `${sfcName}.sfc.json`;
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
    const content = SFC_TEMPLATE.replace("{{name}}", sfcName).replace(
      "{{created}}",
      new Date().toISOString()
    );

    // Write file
    await vscode.workspace.fs.writeFile(
      fileUri,
      Buffer.from(content, "utf-8")
    );

    outputChannel.appendLine(`Created SFC file: ${fileUri.fsPath}`);

    // Open the program in its visual editor
    await vscode.commands.executeCommand(
      "vscode.openWith",
      fileUri,
      SFC_EDITOR_VIEW_TYPE,
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
      `SFC program "${sfcName}" created successfully.`
    );

    return fileUri;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    outputChannel.appendLine(`Error creating SFC: ${message}`);
    vscode.window.showErrorMessage(`Failed to create SFC: ${message}`);
    return undefined;
  }
}
