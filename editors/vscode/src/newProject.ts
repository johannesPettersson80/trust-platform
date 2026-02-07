import * as vscode from "vscode";

type SimulatedCancelAt = "folder" | "name" | "overwrite";

type NewProjectArgs = {
  targetUri?: vscode.Uri | string;
  baseUri?: vscode.Uri | string;
  projectName?: string;
  overwrite?: boolean;
  openWorkspace?: boolean;
  simulateCancelAt?: SimulatedCancelAt;
};

export const NEW_PROJECT_COMMAND = "trust-lsp.newProject";

const MAIN_ST_SOURCE = `PROGRAM Main
END_PROGRAM
`;

const PROJECT_TOML_SOURCE = `include_paths = ["src"]
`;

function asUri(value?: vscode.Uri | string): vscode.Uri | undefined {
  if (!value) {
    return undefined;
  }
  if (value instanceof vscode.Uri) {
    return value;
  }
  try {
    if (value.includes("://")) {
      return vscode.Uri.parse(value);
    }
    return vscode.Uri.file(value);
  } catch {
    return undefined;
  }
}

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

async function isDirectory(uri: vscode.Uri): Promise<boolean> {
  try {
    const stat = await vscode.workspace.fs.stat(uri);
    return (stat.type & vscode.FileType.Directory) !== 0;
  } catch {
    return false;
  }
}

function validateProjectName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "Project name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "Project name must not contain path separators.";
  }
  if (trimmed === "." || trimmed === "..") {
    return "Project name is invalid.";
  }
  return undefined;
}

async function promptForBaseFolder(): Promise<vscode.Uri | undefined> {
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Select Parent Folder",
  });
  return selected?.[0];
}

async function promptForProjectName(): Promise<string | undefined> {
  return vscode.window.showInputBox({
    prompt: "Enter a name for the new Structured Text project",
    placeHolder: "my-st-project",
    validateInput: validateProjectName,
  });
}

async function confirmOverwrite(targetUri: vscode.Uri): Promise<boolean> {
  const selection = await vscode.window.showWarningMessage(
    `The target path already exists: ${targetUri.fsPath}\nContinue and overwrite project scaffold files if present?`,
    { modal: true },
    "Continue",
    "Cancel"
  );
  return selection === "Continue";
}

async function writeScaffold(targetUri: vscode.Uri): Promise<void> {
  const srcUri = vscode.Uri.joinPath(targetUri, "src");
  const sourcesUri = vscode.Uri.joinPath(targetUri, "sources");
  await vscode.workspace.fs.createDirectory(srcUri);
  await vscode.workspace.fs.createDirectory(sourcesUri);
  const mainBuffer = Buffer.from(MAIN_ST_SOURCE);
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(srcUri, "Main.st"),
    mainBuffer
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(sourcesUri, "Main.st"),
    mainBuffer
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(targetUri, "trust-lsp.toml"),
    Buffer.from(PROJECT_TOML_SOURCE)
  );
}

async function resolveTargetUri(
  args?: NewProjectArgs
): Promise<vscode.Uri | undefined> {
  const directTarget = asUri(args?.targetUri);
  if (directTarget) {
    return directTarget;
  }

  if (args?.simulateCancelAt === "folder") {
    return undefined;
  }
  const baseUri = asUri(args?.baseUri) ?? (await promptForBaseFolder());
  if (!baseUri) {
    return undefined;
  }

  if (args?.simulateCancelAt === "name") {
    return undefined;
  }
  const rawName = args?.projectName ?? (await promptForProjectName());
  if (!rawName) {
    return undefined;
  }
  const trimmedName = rawName.trim();
  const validation = validateProjectName(trimmedName);
  if (validation) {
    vscode.window.showErrorMessage(validation);
    return undefined;
  }
  return vscode.Uri.joinPath(baseUri, trimmedName);
}

export function registerNewProjectCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      NEW_PROJECT_COMMAND,
      async (args?: NewProjectArgs) => {
        const targetUri = await resolveTargetUri(args);
        if (!targetUri) {
          return false;
        }

        const exists = await pathExists(targetUri);
        if (exists) {
          if (!(await isDirectory(targetUri))) {
            vscode.window.showErrorMessage(
              `Target path exists and is not a directory: ${targetUri.fsPath}`
            );
            return false;
          }
          if (args?.simulateCancelAt === "overwrite") {
            return false;
          }
          const overwrite = args?.overwrite ?? (await confirmOverwrite(targetUri));
          if (!overwrite) {
            return false;
          }
        }

        await writeScaffold(targetUri);

        const openWorkspace = args?.openWorkspace ?? true;
        if (openWorkspace) {
          await vscode.commands.executeCommand(
            "vscode.openFolder",
            targetUri,
            false
          );
        }

        vscode.window.showInformationMessage(
          `Structured Text project created at ${targetUri.fsPath}`
        );
        return true;
      }
    )
  );
}
