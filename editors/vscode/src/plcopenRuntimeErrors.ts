type PlcopenRuntimeAction = "import" | "export";

const RUNTIME_PATH_SETTING = "trust-lsp.runtime.cli.path";

function normalizeWhitespace(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

function isBinaryMissingError(message: string): boolean {
  const lower = message.toLowerCase();
  return lower.includes("enoent") || lower.includes("not found");
}

function isMalformedImportXml(detail: string): boolean {
  const lower = detail.toLowerCase();
  return (
    lower.includes("failed to parse plcopen xml") ||
    lower.includes("invalid plcopen xml")
  );
}

export function formatPlcopenLaunchError(
  action: PlcopenRuntimeAction,
  message: string
): string {
  const detail = normalizeWhitespace(message);
  if (isBinaryMissingError(detail)) {
    return `PLCopen ${action} could not start because the trust-runtime binary was not found. Build trust-runtime or set '${RUNTIME_PATH_SETTING}'.`;
  }
  return `Failed to run trust-runtime plcopen ${action}: ${detail || "unknown error"}`;
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
  if (action === "import" && isMalformedImportXml(normalized)) {
    return `PLCopen import failed because the input XML is malformed. ${normalized}`;
  }
  return `PLCopen ${action} failed (exit ${exitCode}). ${normalized}`;
}
