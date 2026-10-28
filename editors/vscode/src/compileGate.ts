import * as vscode from "vscode";

import type { CheckProgramResponse } from "./checkProgramModel";
import { projectRequest } from "./projectRequest";

// The sidebar's gates (specification 25 §6.6) are the language server's answer (`trust-lsp.compileGate`): whether
// Start, Update and Debug may run, from the errors VS Code shows in Problems and the last Compile. This module only
// collects the error counts VS Code holds and forwards them.

/** What Problems shows: whether it holds project errors, and how many of which kind. */
export interface ValidityLine {
  readonly ok: boolean;
  readonly label: string;
  readonly errors: number;
  readonly sourceErrors: number;
  readonly configErrors: number;
}

/** The last Compile as the sidebar knows it: not run, stale after a save, passed, or failed with its report. */
export type CompileGateState =
  | { readonly kind: "unknown" }
  | { readonly kind: "dirty" }
  | { readonly kind: "clean" }
  | { readonly kind: "failed"; readonly report: CheckProgramResponse };

/** The gates for the current Problems, the last Compile and the selected target. */
export interface CompileGate {
  readonly validity: ValidityLine;
  /** A failed Compile's error count, split by kind. */
  readonly compile: { readonly errors: number; readonly sourceErrors: number; readonly configErrors: number } | null;
  /** Why each verb may not run, or null. */
  readonly reasons: { readonly start: string | null; readonly update: string | null; readonly debug: string | null };
  readonly debug: { readonly enabled: boolean; readonly title: string };
}

/** No gate: used while the language server is not running; Start still compiles before it launches (§6.5). */
const OPEN_GATE: CompileGate = {
  validity: { ok: true, label: "", errors: 0, sourceErrors: 0, configErrors: 0 },
  compile: null,
  reasons: { start: null, update: null, debug: null },
  debug: { enabled: true, title: "Start debugging" },
};

/** The error count of every file VS Code shows diagnostics for. */
function problemCounts(): Array<{ path: string; errors: number }> {
  const counts: Array<{ path: string; errors: number }> = [];
  for (const [uri, diagnostics] of vscode.languages.getDiagnostics()) {
    const errors = diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error).length;
    if (errors > 0) {
      counts.push({ path: uri.fsPath, errors });
    }
  }
  return counts;
}

/** The gates; without `compile` only Problems decides (the reload command's gate). */
export async function compileGate(compile?: CompileGateState, targetStatus?: string): Promise<CompileGate> {
  const answer = await projectRequest<CompileGate>(
    "trust-lsp.compileGate",
    {
      diagnostics: problemCounts(),
      ...(compile
        ? { compile: { state: compile.kind, ...(compile.kind === "failed" ? { report: compile.report } : {}) } }
        : {}),
      ...(targetStatus ? { target: { status: targetStatus } } : {}),
    },
    { wait: false }
  );
  return answer.ok ? answer : OPEN_GATE;
}
