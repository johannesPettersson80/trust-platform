// Pure model for the Phase 8 "Check program" result (NO vscode import → unit-testable). Mirrors the
// `trust-runtime check --json` shape (status:"ok"|"failed" + CheckIssue[]).

export interface CheckIssue {
  readonly severity: string; // "error" | "warning"
  readonly message: string;
  readonly code?: string; // "compile" | "sources" | "config" | …
  readonly file?: string;
  readonly line?: number;
  readonly column?: number;
}

export interface CheckProgramResponse {
  readonly ok: boolean;
  readonly status: string; // "ok" | "failed"
  readonly errors: number;
  readonly warnings: number;
  readonly issues: CheckIssue[];
  readonly source_count?: number;
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

// One honest line — authoritative (whole-project compile), distinct from the diagnostics-derived
// "No known errors" passive line.
export function summarizeCheck(response: CheckProgramResponse): string {
  if (response.ok) {
    const sources = response.source_count ?? 0;
    return `Project check passed — ${plural(sources, "source")}, no errors.`;
  }
  return `Project check failed — ${plural(response.errors, "error")}, ${plural(
    response.warnings,
    "warning"
  )}.`;
}
