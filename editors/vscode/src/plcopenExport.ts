import * as fs from "fs";
import * as path from "path";
import { spawn } from "child_process";
import * as vscode from "vscode";
import { getBinaryPath } from "./binary";
import {
  formatPlcopenCommandFailure,
  formatPlcopenLaunchError,
} from "./plcopenRuntimeErrors";

type PlcopenExportTarget = "generic" | "ab" | "siemens" | "schneider";
type SimulatedCancelAt = "project" | "output" | "overwrite";

type PlcopenExportArgs = {
  projectUri?: vscode.Uri | string;
  outputUri?: vscode.Uri | string;
  target?: PlcopenExportTarget;
  overwrite?: boolean;
  openOutput?: boolean;
  openReport?: boolean;
  simulateCancelAt?: SimulatedCancelAt;
};

type RuntimeCommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

type PlcopenExportJson = {
  target?: string;
  output_path?: string;
  source_map_path?: string;
  adapter_report_path?: string | null;
  pou_count?: number;
  source_count?: number;
};

export const PLCOPEN_EXPORT_COMMAND = "trust-lsp.plcopen.export";

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

async function promptForProjectFolder(): Promise<vscode.Uri | undefined> {
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri;
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    defaultUri: workspaceRoot,
    openLabel: "Select Export Project Folder",
  });
  return selected?.[0];
}

async function promptForOutputXml(
  projectUri: vscode.Uri
): Promise<vscode.Uri | undefined> {
  const selected = await vscode.window.showSaveDialog({
    defaultUri: vscode.Uri.joinPath(projectUri, "interop", "plcopen.xml"),
    filters: {
      "PLCopen XML": ["xml"],
      "All Files": ["*"],
    },
    saveLabel: "Export PLCopen XML",
  });
  return selected;
}

async function confirmOverwrite(outputUri: vscode.Uri): Promise<boolean> {
  const selected = await vscode.window.showWarningMessage(
    `The output file already exists: ${outputUri.fsPath}\nOverwrite this PLCopen XML export?`,
    { modal: true },
    "Overwrite",
    "Cancel"
  );
  return selected === "Overwrite";
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

function parseExportJson(stdout: string): PlcopenExportJson | undefined {
  const jsonPayload = extractJsonPayload(stdout);
  if (!jsonPayload) {
    return undefined;
  }
  try {
    return JSON.parse(jsonPayload) as PlcopenExportJson;
  } catch {
    return undefined;
  }
}

function toProjectPathUri(filePath: string, projectUri: vscode.Uri): vscode.Uri {
  if (path.isAbsolute(filePath)) {
    return vscode.Uri.file(filePath);
  }
  return vscode.Uri.file(path.join(projectUri.fsPath, filePath));
}

async function openDocument(uri: vscode.Uri): Promise<void> {
  const doc = await vscode.workspace.openTextDocument(uri);
  await vscode.window.showTextDocument(doc);
}

export function registerPlcopenExportCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      PLCOPEN_EXPORT_COMMAND,
      async (args?: PlcopenExportArgs) => {
        if (args?.simulateCancelAt === "project") {
          return false;
        }
        const projectUri = toUri(args?.projectUri) ?? (await promptForProjectFolder());
        if (!projectUri) {
          return false;
        }
        if (!(await pathExists(projectUri))) {
          vscode.window.showErrorMessage(
            `Export project folder does not exist: ${projectUri.fsPath}`
          );
          return false;
        }
        if (!(await isDirectory(projectUri))) {
          vscode.window.showErrorMessage(
            `Export project path is not a directory: ${projectUri.fsPath}`
          );
          return false;
        }

        if (args?.simulateCancelAt === "output") {
          return false;
        }
        const outputUri =
          toUri(args?.outputUri) ?? (await promptForOutputXml(projectUri));
        if (!outputUri) {
          return false;
        }

        const outputExists = await pathExists(outputUri);
        if (outputExists) {
          if (await isDirectory(outputUri)) {
            vscode.window.showErrorMessage(
              `Export output path is a directory, expected a file: ${outputUri.fsPath}`
            );
            return false;
          }
          if (args?.simulateCancelAt === "overwrite") {
            return false;
          }
          const overwrite =
            args?.overwrite ?? (await confirmOverwrite(outputUri));
          if (!overwrite) {
            return false;
          }
        }

        await vscode.workspace.fs.createDirectory(
          vscode.Uri.file(path.dirname(outputUri.fsPath))
        );

        const binary = resolveRuntimeBinary(context);
        const workspaceRoot =
          vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();
        const runtimeArgs = [
          "plcopen",
          "export",
          "--project",
          projectUri.fsPath,
          "--output",
          outputUri.fsPath,
          "--json",
        ];
        if (args?.target) {
          runtimeArgs.push("--target", args.target);
        }

        let result: RuntimeCommandResult;
        try {
          result = await runRuntimeCommand(binary, runtimeArgs, workspaceRoot);
        } catch (error) {
          const message =
            error instanceof Error ? error.message : String(error ?? "unknown");
          vscode.window.showErrorMessage(
            formatPlcopenLaunchError("export", message)
          );
          return false;
        }

        if (result.exitCode !== 0) {
          const detail = (result.stderr || result.stdout).trim();
          vscode.window.showErrorMessage(
            formatPlcopenCommandFailure("export", result.exitCode, detail)
          );
          return false;
        }

        const exportJson = parseExportJson(result.stdout);
        if (!exportJson) {
          vscode.window.showErrorMessage(
            "PLCopen export completed but JSON report could not be parsed."
          );
          return false;
        }

        const outputPath = exportJson.output_path ?? outputUri.fsPath;
        const sourceMapPath = exportJson.source_map_path;
        const adapterReportPath = exportJson.adapter_report_path ?? undefined;
        const exportedOutputUri = toProjectPathUri(outputPath, projectUri);
        const sourceMapUri = sourceMapPath
          ? toProjectPathUri(sourceMapPath, projectUri)
          : undefined;
        const adapterReportUri = adapterReportPath
          ? toProjectPathUri(adapterReportPath, projectUri)
          : undefined;
        const target = exportJson.target ?? args?.target ?? "generic";
        const pouCount = exportJson.pou_count ?? 0;
        const sourceCount = exportJson.source_count ?? 0;

        if (args?.openOutput) {
          await openDocument(exportedOutputUri);
        }
        if (args?.openReport && adapterReportUri) {
          await openDocument(adapterReportUri);
        }

        const actions: string[] = ["Open Export XML"];
        if (sourceMapUri) {
          actions.push("Open Source Map");
        }
        if (adapterReportUri) {
          actions.push("Open Adapter Report");
        }
        void vscode.window
          .showInformationMessage(
            `PLCopen export complete (${target}): ${pouCount} POU(s) from ${sourceCount} source file(s).`,
            ...actions
          )
          .then(async (selection) => {
            if (selection === "Open Export XML") {
              await openDocument(exportedOutputUri);
            }
            if (selection === "Open Source Map" && sourceMapUri) {
              await openDocument(sourceMapUri);
            }
            if (selection === "Open Adapter Report" && adapterReportUri) {
              await openDocument(adapterReportUri);
            }
          });

        return true;
      }
    )
  );
}
