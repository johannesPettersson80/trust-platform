import * as fs from "fs";
import * as path from "path";
import { spawn } from "child_process";
import * as vscode from "vscode";
import { getBinaryPath } from "./binary";

type SimulatedCancelAt = "input" | "project" | "overwrite";

type PlcopenImportArgs = {
  inputUri?: vscode.Uri | string;
  projectUri?: vscode.Uri | string;
  overwrite?: boolean;
  openProject?: boolean;
  openReport?: boolean;
  simulateCancelAt?: SimulatedCancelAt;
};

type RuntimeCommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

type PlcopenImportJson = {
  detected_ecosystem?: string;
  imported_pous?: number;
  discovered_pous?: number;
  migration_report_path?: string;
};

export const PLCOPEN_IMPORT_COMMAND = "trust-lsp.plcopen.import";

function toUri(value?: vscode.Uri | string): vscode.Uri | undefined {
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

async function isDirectoryEmpty(uri: vscode.Uri): Promise<boolean> {
  const entries = await vscode.workspace.fs.readDirectory(uri);
  return entries.length === 0;
}

async function promptForInputXml(): Promise<vscode.Uri | undefined> {
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri;
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: true,
    canSelectFolders: false,
    canSelectMany: false,
    defaultUri: workspaceRoot,
    filters: {
      "PLCopen XML": ["xml"],
      "All Files": ["*"],
    },
    openLabel: "Select PLCopen XML",
  });
  return selected?.[0];
}

async function promptForProjectFolder(): Promise<vscode.Uri | undefined> {
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri;
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    defaultUri: workspaceRoot,
    openLabel: "Select Import Project Folder",
  });
  return selected?.[0];
}

async function confirmOverwrite(projectUri: vscode.Uri): Promise<boolean> {
  const selected = await vscode.window.showWarningMessage(
    `The target project folder is not empty: ${projectUri.fsPath}\nContinue PLCopen import into this folder?`,
    { modal: true },
    "Continue",
    "Cancel"
  );
  return selected === "Continue";
}

function resolveRuntimeBinary(context: vscode.ExtensionContext): string {
  const envPath = process.env.ST_RUNTIME_TEST_BIN?.trim();
  if (envPath && fs.existsSync(envPath)) {
    return envPath;
  }
  return getBinaryPath(context, "trust-runtime", "runtime.cli.path");
}

function runRuntimeCommand(
  binary: string,
  args: string[],
  cwd: string
): Promise<RuntimeCommandResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      cwd,
      env: process.env,
      windowsHide: true,
    });

    let stdout = "";
    let stderr = "";

    child.stdout?.on("data", (chunk: Buffer | string) => {
      stdout += chunk.toString();
    });
    child.stderr?.on("data", (chunk: Buffer | string) => {
      stderr += chunk.toString();
    });
    child.on("error", reject);
    child.on("close", (code) => {
      resolve({
        exitCode: code ?? -1,
        stdout,
        stderr,
      });
    });
  });
}

function extractJsonPayload(stdout: string): string {
  const start = stdout.indexOf("{");
  const end = stdout.lastIndexOf("}");
  if (start < 0 || end <= start) {
    return stdout.trim();
  }
  return stdout.slice(start, end + 1);
}

function parseImportJson(stdout: string): PlcopenImportJson | undefined {
  const jsonPayload = extractJsonPayload(stdout);
  if (!jsonPayload) {
    return undefined;
  }
  try {
    return JSON.parse(jsonPayload) as PlcopenImportJson;
  } catch {
    return undefined;
  }
}

function toMigrationReportUri(
  reportPath: string,
  projectUri: vscode.Uri
): vscode.Uri {
  if (path.isAbsolute(reportPath)) {
    return vscode.Uri.file(reportPath);
  }
  return vscode.Uri.file(path.join(projectUri.fsPath, reportPath));
}

export function registerPlcopenImportCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      PLCOPEN_IMPORT_COMMAND,
      async (args?: PlcopenImportArgs) => {
        if (args?.simulateCancelAt === "input") {
          return false;
        }
        const inputUri = toUri(args?.inputUri) ?? (await promptForInputXml());
        if (!inputUri) {
          return false;
        }
        if (!(await pathExists(inputUri))) {
          vscode.window.showErrorMessage(
            `PLCopen input file does not exist: ${inputUri.fsPath}`
          );
          return false;
        }
        if (await isDirectory(inputUri)) {
          vscode.window.showErrorMessage(
            `PLCopen input must be a file, not a directory: ${inputUri.fsPath}`
          );
          return false;
        }

        if (args?.simulateCancelAt === "project") {
          return false;
        }
        const projectUri =
          toUri(args?.projectUri) ?? (await promptForProjectFolder());
        if (!projectUri) {
          return false;
        }

        const projectExists = await pathExists(projectUri);
        if (projectExists) {
          if (!(await isDirectory(projectUri))) {
            vscode.window.showErrorMessage(
              `Project path exists and is not a directory: ${projectUri.fsPath}`
            );
            return false;
          }
          const empty = await isDirectoryEmpty(projectUri);
          if (!empty) {
            if (args?.simulateCancelAt === "overwrite") {
              return false;
            }
            const overwrite =
              args?.overwrite ?? (await confirmOverwrite(projectUri));
            if (!overwrite) {
              return false;
            }
          }
        } else {
          await vscode.workspace.fs.createDirectory(projectUri);
        }

        const binary = resolveRuntimeBinary(context);
        const workspaceRoot =
          vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();

        let result: RuntimeCommandResult;
        try {
          result = await runRuntimeCommand(
            binary,
            [
              "plcopen",
              "import",
              "--input",
              inputUri.fsPath,
              "--project",
              projectUri.fsPath,
              "--json",
            ],
            workspaceRoot
          );
        } catch (error) {
          const message =
            error instanceof Error ? error.message : String(error ?? "unknown");
          vscode.window.showErrorMessage(
            `Failed to run trust-runtime plcopen import: ${message}`
          );
          return false;
        }

        if (result.exitCode !== 0) {
          const detail = (result.stderr || result.stdout).trim();
          vscode.window.showErrorMessage(
            `PLCopen import failed (exit ${result.exitCode}). ${detail || "No diagnostics returned."}`
          );
          return false;
        }

        const importJson = parseImportJson(result.stdout);
        if (!importJson) {
          vscode.window.showErrorMessage(
            "PLCopen import completed but JSON report could not be parsed."
          );
          return false;
        }

        const detectedEcosystem = importJson.detected_ecosystem ?? "unknown";
        const importedPous = importJson.imported_pous ?? 0;
        const discoveredPous = importJson.discovered_pous ?? importedPous;
        const migrationReportPath = importJson.migration_report_path;
        const migrationReportUri = migrationReportPath
          ? toMigrationReportUri(migrationReportPath, projectUri)
          : undefined;

        if (args?.openProject) {
          await vscode.commands.executeCommand(
            "vscode.openFolder",
            projectUri,
            false
          );
        }
        if (args?.openReport && migrationReportUri) {
          const doc = await vscode.workspace.openTextDocument(migrationReportUri);
          await vscode.window.showTextDocument(doc);
        }

        const actions = ["Open Imported Project"];
        if (migrationReportUri) {
          actions.unshift("Open Migration Report");
        }
        void vscode.window
          .showInformationMessage(
            `PLCopen import complete (${detectedEcosystem}): imported ${importedPous}/${discoveredPous} POU(s).`,
            ...actions
          )
          .then(async (selection) => {
            if (selection === "Open Migration Report" && migrationReportUri) {
              const doc = await vscode.workspace.openTextDocument(migrationReportUri);
              await vscode.window.showTextDocument(doc);
            }
            if (selection === "Open Imported Project") {
              await vscode.commands.executeCommand(
                "vscode.openFolder",
                projectUri,
                false
              );
            }
          });

        return true;
      }
    )
  );
}
