import { projectRequest } from "../projectRequest";

export type NetworkCanvasRuntimeFailureKind =
  | "missing_binary"
  | "port_conflict"
  | "workspace_permission"
  | "failed_spawn"
  | "stale_runtime"
  // The runtime answered Connect but keeps debugging disabled: it is the target, without Live Values.
  | "debug_disabled";

export type NetworkCanvasRuntimeFailure = {
  readonly kind: NetworkCanvasRuntimeFailureKind;
  readonly message: string;
  readonly detail?: string;
  // The control endpoint of the runtime a Connect failure belongs to. Devices & Connections shows the
  // failure on that runtime's node (specification 25, section 5.1).
  readonly endpoint?: string;
  // What the sidebar offers after a failed Connect, as the language server names it (§5.1).
  readonly remedy?: "openDevices" | "setAuthToken";
};

/** The kind of the failure `error` names, with the message the extension shows: the language server reads the failure's
 * text (`trust-lsp.runTargets`, action `failure`, specification 25 §5.10). */
export async function classifyRuntimeStartFailure(error: unknown): Promise<NetworkCanvasRuntimeFailure> {
  const detail = error instanceof Error ? error.message : String(error ?? "");
  const answer = await projectRequest<{ failure: NetworkCanvasRuntimeFailure }>("trust-lsp.runTargets", {
    action: "failure",
    detail,
  });
  return answer.ok ? answer.failure : { kind: "failed_spawn", message: detail || answer.error, detail };
}
