export type NetworkCanvasRuntimeFailureKind =
  | "missing_binary"
  | "configuration"
  | "internal_startup"
  | "port_conflict"
  | "workspace_permission"
  | "failed_spawn"
  | "stale_runtime"
  | "readiness_timeout";

export type NetworkCanvasRuntimeFailure = {
  readonly kind: NetworkCanvasRuntimeFailureKind;
  readonly message: string;
  readonly detail?: string;
};

export function classifyRuntimeStartFailure(
  error: unknown
): NetworkCanvasRuntimeFailure {
  const carriedFailure = carriedRuntimeFailure(error);
  if (carriedFailure) {
    return carriedFailure;
  }
  const detail = error instanceof Error ? error.message : String(error ?? "");
  const lower = detail.toLowerCase();
  if (isMissingRuntimeControlAuth(lower)) {
    return {
      kind: "configuration",
      message:
        "Simulator needs control authentication in runtime.toml, and truST could not add it automatically. Make the file writable or open it to configure the token.",
      detail,
    };
  }
  if (isInvalidRuntimeConfiguration(lower)) {
    return {
      kind: "configuration",
      message:
        "Runtime configuration could not be loaded. Open runtime.toml and fix the reported setting.",
      detail,
    };
  }
  if (isMissingExecutableFailure(lower)) {
    return {
      kind: "missing_binary",
      message:
        "Required runtime/debug binary was not found. Update or reinstall the truST extension, then start the simulator again.",
      detail,
    };
  }
  if (
    lower.includes("eaddrinuse") ||
    lower.includes("address already in use") ||
    (lower.includes("port") && lower.includes("in use"))
  ) {
    return {
      kind: "port_conflict",
      message:
        "The runtime port is already in use. Close the other truST/debug session or process using the port, then start again. Open logs to identify it.",
      detail,
    };
  }
  if (
    lower.includes("project files changed after compile") ||
    lower.includes("project source changed after compile")
  ) {
    return {
      kind: "failed_spawn",
      message:
        "Project files changed after Compile. Start again to compile the latest files.",
      detail,
    };
  }
  if (
    lower.includes("eacces") ||
    lower.includes("eperm") ||
    lower.includes("permission denied") ||
    lower.includes("access denied") ||
    lower.includes("operation not permitted") ||
    lower.includes("read-only file system") ||
    lower.includes("read-only filesystem")
  ) {
    return {
      kind: "workspace_permission",
      message:
        "The workspace or runtime path is not writable. Make it writable, then start the simulator again.",
      detail,
    };
  }
  if (
    lower.includes("stale") ||
    lower.includes("zombie") ||
    lower.includes("already running")
  ) {
    return {
      kind: "stale_runtime",
      message:
        "A stale runtime or debug session blocked startup. Stop the existing session or reload the VS Code window, then start again.",
      detail,
    };
  }
  if (
    lower.includes("timeout") ||
    lower.includes("timed out") ||
    lower.includes("still pending after") ||
    lower.includes("did not become ready")
  ) {
    return {
      kind: "readiness_timeout",
      message:
        "The simulator did not become ready in time. Open the Structured Text Debugger logs to see what blocked startup.",
      detail,
    };
  }
  return {
    kind: "failed_spawn",
    message:
      "Simulator startup failed. Check the Structured Text Debugger output for details.",
    detail,
  };
}

function carriedRuntimeFailure(
  error: unknown
): NetworkCanvasRuntimeFailure | undefined {
  if (typeof error !== "object" || error === null) {
    return undefined;
  }
  const carrier = error as { runtimeFailure?: unknown };
  if (typeof carrier.runtimeFailure !== "object" || carrier.runtimeFailure === null) {
    return undefined;
  }
  const failure = carrier.runtimeFailure as {
    kind?: unknown;
    message?: unknown;
    detail?: unknown;
  };
  if (
    failure.kind !== "configuration" ||
    typeof failure.message !== "string" ||
    !failure.message.trim()
  ) {
    return undefined;
  }
  return {
    kind: "configuration",
    message: failure.message.trim(),
    ...(typeof failure.detail === "string" && failure.detail.trim()
      ? { detail: failure.detail.trim() }
      : {}),
  };
}

export function simulatorStartupIncompleteFailure(): NetworkCanvasRuntimeFailure {
  return {
    kind: "internal_startup",
    message:
      "Simulator startup could not finish. Check the Structured Text Debugger output for details.",
  };
}

export function runtimeStatusCheckFailure(
  error: unknown
): NetworkCanvasRuntimeFailure {
  return {
    kind: "internal_startup",
    message:
      "Runtime status check failed. Check the Structured Text Debugger output for details.",
    detail: error instanceof Error ? error.message : String(error ?? ""),
  };
}

function isMissingRuntimeControlAuth(detail: string): boolean {
  return (
    detail.includes("missing_auth_token") ||
    detail.includes("no auth token provided") ||
    (detail.includes("runtime.control.auth_token") &&
      (detail.includes("required") || detail.includes("missing")))
  );
}

function isInvalidRuntimeConfiguration(detail: string): boolean {
  return (
    detail.includes("runtime.toml") &&
    (detail.includes("invalid config") ||
      detail.includes("failed to load") ||
      detail.includes("failed to parse"))
  );
}

function isMissingExecutableFailure(detail: string): boolean {
  if (/\benoent\b/.test(detail)) {
    return true;
  }
  const missingEvidence =
    detail.includes("not found") ||
    detail.includes("cannot find") ||
    detail.includes("could not find") ||
    detail.includes("no such file or directory") ||
    detail.includes("does not exist") ||
    detail.includes("is not recognized as an internal or external command");
  const executableEvidence =
    detail.includes("spawn") ||
    detail.includes("executable") ||
    detail.includes("binary") ||
    detail.includes("trust-runtime.exe") ||
    detail.includes("trust-debug.exe") ||
    detail.includes("executable path");
  return missingEvidence && executableEvidence;
}
