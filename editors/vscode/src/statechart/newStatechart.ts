import * as vscode from "vscode";
import { asUri, pathExists } from "./uriUtils";

type NewStatechartArgs = {
  targetUri?: vscode.Uri | string;
  statechartName?: string;
  overwrite?: boolean;
};

export const NEW_STATECHART_COMMAND = "trust-lsp.statechart.new";

const STATECHART_TEMPLATE = `{
  "id": "{{name}}",
  "initial": "Initial",
  "states": {
    "Initial": {
      "entry": [],
      "exit": [],
      "on": {
        "START": {
          "target": "Running",
          "actions": []
        }
      }
    },
    "Running": {
      "entry": [],
      "exit": [],
      "on": {
        "STOP": {
          "target": "Initial",
          "actions": []
        }
      }
    }
  }
}
`;

function validateStatechartName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "Statechart name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "Statechart name must not contain path separators.";
  }
  if (trimmed === "." || trimmed === "..") {
    return "Statechart name is invalid.";
  }
  return undefined;
}

async function promptForStatechartName(): Promise<string | undefined> {
  return vscode.window.showInputBox({
    prompt: "Enter a name for the new UML Statechart",
    placeHolder: "my-statechart",
    validateInput: validateStatechartName,
  });
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

async function writeStatechart(
  targetUri: vscode.Uri,
  statechartName: string
): Promise<void> {
  const content = STATECHART_TEMPLATE.replace("{{name}}", statechartName);
  const buffer = Buffer.from(content);
  await vscode.workspace.fs.writeFile(targetUri, buffer);
}

async function resolveTargetUri(
  args?: NewStatechartArgs
): Promise<{ uri: vscode.Uri; name: string } | undefined> {
  const directTarget = asUri(args?.targetUri);
  if (directTarget) {
    const name =
      args?.statechartName ??
      directTarget.path.split("/").pop()?.replace(".statechart.json", "") ??
      "statechart";
    return { uri: directTarget, name };
  }

  // First, ask for the name
  const rawName = args?.statechartName ?? (await promptForStatechartName());
  if (!rawName) {
    return undefined;
  }

  const trimmedName = rawName.trim();
  const validation = validateStatechartName(trimmedName);
  if (validation) {
    vscode.window.showErrorMessage(validation);
    return undefined;
  }

  // Get the current workspace folder or ask the user to select one
  let baseUri: vscode.Uri | undefined;
  const folders = vscode.workspace.workspaceFolders;
  if (folders && folders.length > 0) {
    baseUri = folders[0].uri;
  } else {
    const selected = await vscode.window.showOpenDialog({
      canSelectFiles: false,
      canSelectFolders: true,
      canSelectMany: false,
      openLabel: "Select Folder for New Statechart",
      title: `Select folder to save "${trimmedName}"`,
    });
    baseUri = selected?.[0];
  }

  if (!baseUri) {
    return undefined;
  }

  const fileName = trimmedName.endsWith(".statechart.json")
    ? trimmedName
    : `${trimmedName}.statechart.json`;

  return {
    uri: vscode.Uri.joinPath(baseUri, fileName),
    name: trimmedName.replace(".statechart.json", ""),
  };
}

export function registerNewStatechartCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      NEW_STATECHART_COMMAND,
      async (args?: NewStatechartArgs) => {
        const resolved = await resolveTargetUri(args);
        if (!resolved) {
          return;
        }

        const { uri: targetUri, name } = resolved;

        const exists = await pathExists(targetUri);
        if (exists) {
          const shouldOverwrite =
            args?.overwrite ?? (await confirmOverwrite(targetUri));
          if (!shouldOverwrite) {
            return;
          }
        }

        try {
          await writeStatechart(targetUri, name);

          // Open with the custom StateChart editor
          await vscode.commands.executeCommand(
            "vscode.openWith",
            targetUri,
            "trust-lsp.statechartEditor"
          );

          vscode.window.showInformationMessage(
            `UML Statechart created: ${targetUri.fsPath}`
          );
        } catch (error) {
          console.error("[New Statechart] Error:", error);
          vscode.window.showErrorMessage(
            `Failed to create statechart: ${error instanceof Error ? error.message : String(error)}`
          );
        }
      }
    )
  );
}
