import * as vscode from "vscode";

export interface ValidityLine {
  readonly ok: boolean;
  readonly label: string;
  readonly errors: number;
  readonly sourceErrors: number;
  readonly configErrors: number;
}

export type CompileGateVerb = "start" | "update" | "debug";

export interface CompileGateState {
  readonly kind: string;
  readonly errors?: number;
  readonly configErrors?: number;
  readonly summary?: string;
}

// Passive validity (§0.5.6): diagnostics are only a pre-compile warning source.
// They are never enough to show a green Compile badge.
export function validityLine(): ValidityLine {
  let errors = 0;
  let sourceErrors = 0;
  let configErrors = 0;
  for (const [uri, diagnostics] of vscode.languages.getDiagnostics()) {
    const filePath = uri.fsPath;
    const relevant = isSourceDiagnosticPath(filePath) || isConfigDiagnosticPath(filePath);
    if (!relevant) {
      continue;
    }
    const count = diagnostics.filter(
      (d) => d.severity === vscode.DiagnosticSeverity.Error
    ).length;
    errors += count;
    if (isConfigDiagnosticPath(filePath)) {
      configErrors += count;
    } else {
      sourceErrors += count;
    }
  }
  return errors === 0
    ? { ok: true, label: "No known errors", errors, sourceErrors, configErrors }
    : {
        ok: false,
        label: `${errors} error${errors === 1 ? "" : "s"} — see Problems`,
        errors,
        sourceErrors,
        configErrors,
      };
}

export function isSourceDiagnosticPath(filePath: string): boolean {
  return /\.(st|pou)$/i.test(filePath);
}

export function isConfigDiagnosticPath(filePath: string): boolean {
  return /(^|[/\\])(runtime|trust-lsp)\.toml$/i.test(filePath) ||
    /[/\\]hmi[/\\].+\.toml$/i.test(filePath);
}

export function diagnosticsGateReason(
  diagnostics: ValidityLine,
  verb: CompileGateVerb
): string | undefined {
  if (diagnostics.ok) {
    return undefined;
  }
  if (diagnostics.configErrors > 0) {
    return configGateReason(verb);
  }
  return sourceGateReason(diagnostics.errors, verb);
}

export function compileGateReason(
  compileState: CompileGateState,
  diagnostics: ValidityLine,
  verb: CompileGateVerb
): string | undefined {
  const diagnosticsReason = diagnosticsGateReason(diagnostics, verb);
  if (diagnosticsReason) {
    return diagnosticsReason;
  }
  if (compileState.kind === "failed") {
    if ((compileState.configErrors ?? 0) > 0 || looksLikeConfigFailure(compileState.summary ?? "")) {
      return configGateReason(verb);
    }
    return sourceGateReason(compileState.errors ?? 1, verb);
  }
  return undefined;
}

function configGateReason(verb: CompileGateVerb): string {
  return `Fix runtime.toml to ${verb}.`;
}

function sourceGateReason(errors: number, verb: CompileGateVerb): string {
  const count = Math.max(1, errors);
  return `Fix ${count} error${count === 1 ? "" : "s"} to ${verb}.`;
}

function looksLikeConfigFailure(summary: string): boolean {
  return /\b(runtime|trust-lsp)\.toml\b|configuration|config/i.test(summary);
}
