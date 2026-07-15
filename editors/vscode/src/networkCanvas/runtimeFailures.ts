export type NetworkCanvasRuntimeFailureKind =
  | "missing_binary"
  | "port_conflict"
  | "workspace_permission"
  | "failed_spawn"
  | "stale_runtime";

export type NetworkCanvasRuntimeFailure = {
  readonly kind: NetworkCanvasRuntimeFailureKind;
  readonly message: string;
  readonly detail?: string;
};

export function classifyRuntimeStartFailure(
  error: unknown
): NetworkCanvasRuntimeFailure {
  const detail = error instanceof Error ? error.message : String(error ?? "");
  const lower = detail.toLowerCase();
  if (
    lower.includes("enoent") ||
    lower.includes("not found") ||
    lower.includes("trust-runtime") ||
    lower.includes("trust-debug") ||
    lower.includes("binary")
  ) {
    return {
      kind: "missing_binary",
      message: "Required runtime/debug binary was not found.",
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
      message: "The runtime port is already in use.",
      detail,
    };
  }
  if (
    lower.includes("eacces") ||
    lower.includes("permission") ||
    lower.includes("read-only") ||
    lower.includes("workspace")
  ) {
    return {
      kind: "workspace_permission",
      message: "The workspace or runtime path is not writable.",
      detail,
    };
  }
  if (
    lower.includes("timeout") ||
    lower.includes("timed out") ||
    lower.includes("stale") ||
    lower.includes("zombie") ||
    lower.includes("already running")
  ) {
    return {
      kind: "stale_runtime",
      message: "A stale runtime or debug session blocked startup.",
      detail,
    };
  }
  return {
    kind: "failed_spawn",
    message: detail || "Runtime failed to start.",
    detail,
  };
}
