import * as vscode from "vscode";

// Specification 25 §6.10 and specification 48 RU-44: after each Simulator build the debug adapter
// sends the compiler's errors and warnings of the project's Rust crate, each at its file's absolute
// path and its 1-based place; Problems shows them as they are, with their source, in place of the
// last build's. The extension decides nothing about them: it only converts the places to VS Code's
// 0-based positions.

const DEBUG_TYPE = "structured-text";

/** The debug adapter's custom event of a build's diagnostics. */
export const BUILD_DIAGNOSTICS_EVENT = "trustBuildDiagnostics";

interface BuildDiagnostic {
  readonly path: string;
  readonly severity: string;
  readonly code: string | null;
  readonly message: string;
  readonly line: number;
  readonly column: number;
  readonly end_line: number;
  readonly end_column: number;
}

interface BuildDiagnosticsBody {
  readonly source: string;
  readonly diagnostics: readonly BuildDiagnostic[];
}

let collection: vscode.DiagnosticCollection | undefined;

export function registerRustBuildDiagnostics(context: vscode.ExtensionContext): void {
  collection = vscode.languages.createDiagnosticCollection("truST (rustc)");
  context.subscriptions.push(collection);
  context.subscriptions.push(
    vscode.debug.onDidReceiveDebugSessionCustomEvent((event) => {
      if (event.event === BUILD_DIAGNOSTICS_EVENT && event.session.type === DEBUG_TYPE) {
        show(event.body as BuildDiagnosticsBody);
      }
    })
  );
}

/** Test hook: shows `body` as the event of a build would. */
export function __testShowBuildDiagnostics(body: BuildDiagnosticsBody): void {
  show(body);
}

/** Replaces the last build's diagnostics with those of `body`. */
function show(body: BuildDiagnosticsBody): void {
  if (!collection) {
    return;
  }
  collection.clear();
  const byFile = new Map<string, vscode.Diagnostic[]>();
  for (const found of body.diagnostics ?? []) {
    const range = new vscode.Range(found.line - 1, found.column - 1, found.end_line - 1, found.end_column - 1);
    const severity =
      found.severity === "error" ? vscode.DiagnosticSeverity.Error : vscode.DiagnosticSeverity.Warning;
    const diagnostic = new vscode.Diagnostic(range, found.message, severity);
    diagnostic.source = body.source;
    if (found.code) {
      diagnostic.code = found.code;
    }
    const list = byFile.get(found.path) ?? [];
    list.push(diagnostic);
    byFile.set(found.path, list);
  }
  for (const [file, diagnostics] of byFile) {
    collection.set(vscode.Uri.file(file), diagnostics);
  }
}
