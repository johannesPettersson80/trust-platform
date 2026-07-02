import { execFile } from "child_process";
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

let collection: vscode.DiagnosticCollection | undefined;
const didCheckProgramEmitter = new vscode.EventEmitter<CheckProgramResponse>();
let lastCompileStatusMessage: vscode.Disposable | undefined;

export const onDidCheckProgram = didCheckProgramEmitter.event;

export function registerCheckProgram(context: vscode.ExtensionContext): void {
  collection = vscode.languages.createDiagnosticCollection("trust check");
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

function projectRoot(): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor?.document.uri;
  const folder = active
    ? vscode.workspace.getWorkspaceFolder(active)
    : undefined;
  return folder?.uri ?? vscode.workspace.workspaceFolders?.[0]?.uri;
}

function runCheck(
  context: vscode.ExtensionContext,
  root: string
): Promise<CheckProgramResponse | undefined> {
  const binary = getBinaryPath(context, "trust-runtime", "runtime.cli.path");
  return new Promise((resolve) => {
    execFile(
      binary,
      ["check", "--project", root, "--json"],
      { cwd: root, timeout: 60_000, maxBuffer: 32 * 1024 * 1024 },
      (_error, stdout) => {
        // `check` exits non-zero when the project fails — but still prints the JSON report to stdout,
        // so parse stdout regardless of exit code; only a missing/old binary yields no JSON.
        try {
          resolve(JSON.parse(stdout) as CheckProgramResponse);
        } catch {
          resolve(undefined);
        }
      }
    );
  });
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
  if (!response) {
    clearCompileStatusMessage();
    void vscode.window.showWarningMessage(
      "Could not compile the project (the trust-runtime tools aren't available)."
    );
    return undefined;
  }

  applyDiagnostics(root, response);
  didCheckProgramEmitter.fire(response);
  const summary = summarizeCheck(
    response,
    response.ok ? undefined : visibleProblemCounts(root)
  );
  clearCompileStatusMessage();
  lastCompileStatusMessage = vscode.window.setStatusBarMessage(summary, 8_000);
  if (response.ok) {
    return response;
  } else {
    void vscode.window
      .showWarningMessage(summary, "Show Problems")
      .then((choice) => {
        if (choice === "Show Problems") {
          void vscode.commands.executeCommand("workbench.actions.view.problems");
        }
      });
  }
  return response;
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

// File-anchored issues → the Problems panel (labeled `trust check`). File-less issues (config/sources)
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
    diagnostic.source = "trust check";
    const key = fileUri.toString();
    const list = byFile.get(key) ?? [];
    list.push(diagnostic);
    byFile.set(key, list);
  }
  for (const [key, diags] of byFile) {
    collection?.set(vscode.Uri.parse(key), diags);
  }
}
