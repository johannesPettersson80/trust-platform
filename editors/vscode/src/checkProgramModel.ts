// Pure model for the Phase 8 Compile result (NO vscode import → unit-testable). Mirrors the
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

export interface CheckProblemCounts {
  readonly errors: number;
  readonly warnings: number;
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

// One honest line — authoritative whole-project compile result.
export function summarizeCheck(
  response: CheckProgramResponse,
  visibleProblems?: CheckProblemCounts
): string {
  if (response.ok) {
    const sources = response.source_count ?? 0;
    return `Compile passed — ${plural(sources, "source")}, no errors.`;
  }
  const errors = Math.max(response.errors, visibleProblems?.errors ?? 0);
  const warnings = Math.max(response.warnings, visibleProblems?.warnings ?? 0);
  return `Compile failed — ${plural(errors, "error")}, ${plural(
    warnings,
    "warning"
  )}.`;
}
