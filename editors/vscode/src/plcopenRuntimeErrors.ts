type PlcopenRuntimeAction = "import" | "export";

const RUNTIME_PATH_SETTING = "trust.runtime.executablePath";

function normalizeWhitespace(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

// Node names a binary it cannot start by the code ENOENT. The words of an error never decide it: a failure that only
// says "not found" is no missing trust-runtime (specification 25, section 11.3).
export function formatPlcopenLaunchError(
  action: PlcopenRuntimeAction,
  error: unknown
): string {
  if ((error as NodeJS.ErrnoException | undefined)?.code === "ENOENT") {
    return `PLCopen ${action} could not start because the trust-runtime binary was not found. Build trust-runtime or set '${RUNTIME_PATH_SETTING}'.`;
  }
  const message = error instanceof Error ? error.message : String(error ?? "");
  return `Failed to run trust-runtime plcopen ${action}: ${normalizeWhitespace(message) || "unknown error"}`;
}

export function formatPlcopenCommandFailure(
  action: PlcopenRuntimeAction,
  exitCode: number,
  detail: string
): string {
  const normalized = normalizeWhitespace(detail);
  if (!normalized) {
    return `PLCopen ${action} failed (exit ${exitCode}). No diagnostics returned.`;
  }
  return `PLCopen ${action} failed (exit ${exitCode}). ${normalized}`;
}
