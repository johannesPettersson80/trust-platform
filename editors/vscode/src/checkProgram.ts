import { execFile } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { getBinaryPath } from "./binary";
import {
  summarizeCheck,
  type CheckProblemCounts,
  type CheckProgramResponse,
} from "./checkProgramModel";

// §0.5.17 / Phase 8 — user-facing Compile via `trust-runtime check --json`. The backend verb remains
// `check`; the IDE verb is Compile. Results go to Problems plus a one-line summary, and the command
// returns the parsed report so the sidebar can show an honest badge only after a real compile result.

export const CHECK_PROGRAM_COMMAND = "trust-lsp.checkProgram";
const TRUST_DIAGNOSTIC_SOURCE = "truST";
const EXPECTED_CHECK_REPORT_VERSION = 1;
const RUNTIME_PATH_SETTING = "trust.runtime.executablePath";

type CheckLaunchFailureKind =
  | "missing_binary"
  | "version_mismatch"
  | "unavailable";

interface CheckLaunchFailure {
  readonly kind: CheckLaunchFailureKind;
  readonly message: string;
}

interface CheckRunResult {
  readonly response?: CheckProgramResponse;
  readonly failure?: CheckLaunchFailure;
}

let collection: vscode.DiagnosticCollection | undefined;
const didCheckProgramEmitter = new vscode.EventEmitter<CheckProgramResponse>();
let lastCompileStatusMessage: vscode.Disposable | undefined;

export const onDidCheckProgram = didCheckProgramEmitter.event;

export function registerCheckProgram(context: vscode.ExtensionContext): void {
  collection = vscode.languages.createDiagnosticCollection(TRUST_DIAGNOSTIC_SOURCE);
  context.subscriptions.push(collection);
  context.subscriptions.push(didCheckProgramEmitter);
  context.subscriptions.push({
    dispose: () => clearCompileStatusMessage(),
  });
  context.subscriptions.push(
    vscode.commands.registerCommand(CHECK_PROGRAM_COMMAND, () =>
      runAndReport(context)
    )
  );
}

function hasProjectMarker(root: string): boolean {
  return fs.existsSync(path.join(root, "trust-lsp.toml"));
}

function nearestProjectRoot(fsPath: string): string | undefined {
  let current = fs.existsSync(fsPath) && fs.statSync(fsPath).isDirectory()
    ? fsPath
    : path.dirname(fsPath);
  while (true) {
    if (hasProjectMarker(current)) {
      return current;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      return undefined;
    }
    current = parent;
  }
}

function projectRoot(): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor?.document.uri;
  if (active?.scheme === "file") {
    const nearest = nearestProjectRoot(active.fsPath);
    if (nearest) {
      return vscode.Uri.file(nearest);
    }
  }
  const folder = active ? vscode.workspace.getWorkspaceFolder(active) : undefined;
  return folder?.uri ?? vscode.workspace.workspaceFolders?.[0]?.uri;
}

function runCheck(
  context: vscode.ExtensionContext,
  root: string
): Promise<CheckRunResult> {
  const binary = getBinaryPath(context, "trust-runtime", "runtime.cli.path");
  return new Promise((resolve) => {
    execFile(
      binary,
      ["check", "--project", root, "--json"],
      { cwd: root, timeout: 60_000, maxBuffer: 32 * 1024 * 1024 },
      (error, stdout, stderr) => {
        // `check` exits non-zero when the project fails — but still prints the JSON report to stdout,
        // so parse stdout regardless of exit code; only a missing/old binary yields no JSON.
        try {
          const response = JSON.parse(stdout) as CheckProgramResponse;
          const version = response.version;
          if (version !== EXPECTED_CHECK_REPORT_VERSION) {
            resolve({ failure: versionMismatchFailure(version) });
            return;
          }
          resolve({ response });
        } catch {
          resolve({ failure: launchFailure(error, stderr) });
        }
      }
    );
  });
}

function versionMismatchFailure(version: number | undefined): CheckLaunchFailure {
  const found = typeof version === "number" ? String(version) : "missing";
  return {
    kind: "version_mismatch",
    message: `Runtime mismatch v${found} != v${EXPECTED_CHECK_REPORT_VERSION}. Update truST.`,
  };
}

function launchFailure(
  error: Error | null,
  stderr: string
): CheckLaunchFailure {
  const detail = `${error?.message ?? ""} ${stderr ?? ""}`.trim();
  const lower = detail.toLowerCase();
  if (
    lower.includes("enoent") ||
    lower.includes("not found") ||
    lower.includes("no such file")
  ) {
    return {
      kind: "missing_binary",
      message: "Missing trust-runtime. Set Runtime path.",
    };
  }
  return {
    kind: "unavailable",
    message: `Could not compile because the trust-runtime tools are unavailable. Set '${RUNTIME_PATH_SETTING}' or reinstall truST.`,
  };
}

async function runAndReport(
  context: vscode.ExtensionContext
): Promise<CheckProgramResponse | undefined> {
  const root = projectRoot();
  if (!root) {
    clearCompileStatusMessage();
    void vscode.window.showWarningMessage(
      "Open a truST project before compiling."
    );
    return undefined;
  }
  const response = await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Window, title: "truST: compiling…" },
    () => runCheck(context, root.fsPath)
  );
  if (!response.response) {
    clearCompileStatusMessage();
    void vscode.window.showWarningMessage(
      response.failure?.message ??
        `Could not compile because the trust-runtime tools are unavailable. Set '${RUNTIME_PATH_SETTING}' or reinstall truST.`
    );
    return undefined;
  }

  applyDiagnostics(root, response.response);
  didCheckProgramEmitter.fire(response.response);
  const summary = summarizeCheck(
    response.response,
    response.response.ok ? undefined : visibleProblemCounts(root)
  );
  clearCompileStatusMessage();
  lastCompileStatusMessage = vscode.window.setStatusBarMessage(summary, 8_000);
  if (response.response.ok) {
    return response.response;
  } else {
    void vscode.window
      .showWarningMessage(summary, "Show Problems")
      .then((choice) => {
        if (choice === "Show Problems") {
          void vscode.commands.executeCommand("workbench.actions.view.problems");
        }
      });
  }
  return response.response;
}

function clearCompileStatusMessage(): void {
  lastCompileStatusMessage?.dispose();
  lastCompileStatusMessage = undefined;
}

function visibleProblemCounts(root: vscode.Uri): CheckProblemCounts {
  let errors = 0;
  let warnings = 0;
  for (const [uri, diagnostics] of vscode.languages.getDiagnostics()) {
    if (!isInWorkspaceRoot(root, uri)) {
      continue;
    }
    for (const diagnostic of diagnostics) {
      if (diagnostic.severity === vscode.DiagnosticSeverity.Error) {
        errors += 1;
      } else if (diagnostic.severity === vscode.DiagnosticSeverity.Warning) {
        warnings += 1;
      }
    }
  }
  return { errors, warnings };
}

function isInWorkspaceRoot(root: vscode.Uri, uri: vscode.Uri): boolean {
  if (uri.scheme !== "file" || root.scheme !== "file") {
    return uri.toString().startsWith(root.toString());
  }
  const relative = path.relative(root.fsPath, uri.fsPath);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

// File-anchored issues → the Problems panel (labeled `truST`). File-less issues (config/sources)
// are covered by the summary message.
function applyDiagnostics(
  root: vscode.Uri,
  response: CheckProgramResponse
): void {
  collection?.clear();
  const byFile = new Map<string, vscode.Diagnostic[]>();
  for (const issue of response.issues ?? []) {
    if (!issue.file) {
      continue;
    }
    const fileUri = path.isAbsolute(issue.file)
      ? vscode.Uri.file(issue.file)
      : vscode.Uri.joinPath(root, issue.file);
    const line = Math.max(0, (issue.line ?? 1) - 1);
    const column = Math.max(0, (issue.column ?? 1) - 1);
    const range = new vscode.Range(line, column, line, column + 1);
    const diagnostic = new vscode.Diagnostic(
      range,
      issue.message,
      issue.severity === "warning"
        ? vscode.DiagnosticSeverity.Warning
        : vscode.DiagnosticSeverity.Error
    );
    diagnostic.source = TRUST_DIAGNOSTIC_SOURCE;
    const key = fileUri.toString();
    const list = byFile.get(key) ?? [];
    list.push(diagnostic);
    byFile.set(key, list);
  }
  for (const [key, diags] of byFile) {
    collection?.set(vscode.Uri.parse(key), diags);
  }
}
