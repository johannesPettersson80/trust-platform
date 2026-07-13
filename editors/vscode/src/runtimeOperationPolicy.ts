import type { LifecyclePhase } from "./lifecycleEntryFailure";
import type { RuntimeLifecycleTarget } from "./runtimeLifecycleModel";
import type { SelectedRuntime } from "./trustHomeModel";

export type RuntimeLockedAction =
  | "compile"
  | "apply_changes"
  | "select_target"
  | "set_run_target"
  | "local_start"
  | "local_stop"
  | "remote_connect"
  | "remote_disconnect"
  | "managed_start"
  | "managed_stop";

export function runtimeOperationAllowed(
  phase: LifecyclePhase,
  action: RuntimeLockedAction,
  operationInProgress = false
): boolean {
  if (operationInProgress) {
    return false;
  }
  switch (phase) {
    case "stopped":
      return action !== "apply_changes";
    case "starting":
      return false;
    case "running":
      return [
        "compile",
        "apply_changes",
        "local_stop",
        "managed_stop",
      ].includes(action);
    case "connected":
      return ["remote_disconnect", "managed_stop"].includes(action);
  }
}

export function runtimeOperationBlockReason(
  phase: LifecyclePhase,
  action: RuntimeLockedAction,
  operationInProgress = false
): string | undefined {
  if (runtimeOperationAllowed(phase, action, operationInProgress)) {
    return undefined;
  }
  if (operationInProgress || phase === "starting") {
    return "A runtime operation is already in progress. Wait for it to finish.";
  }
  if (phase === "running") {
    return "Stop the Simulator before changing the target or starting another runtime operation.";
  }
  if (phase === "stopped") {
    return "Start the Simulator before updating the running simulation.";
  }
  return "Disconnect the remote runtime before changing the target or starting another runtime operation.";
}

/** Maps the sidebar's literal verb to the shared lifecycle operation policy. */
export function lockedActionForSelectedRuntime(
  selected: SelectedRuntime
): RuntimeLockedAction {
  switch (selected.kind) {
    case "simulator":
      return selected.primary.action === "stop" ? "local_stop" : "local_start";
    case "local":
      return selected.primary.action === "stop" ? "managed_stop" : "managed_start";
    case "remote":
      return selected.primary.action === "disconnect"
        ? "remote_disconnect"
        : "remote_connect";
  }
}

/** The operation lease target corresponding to the sidebar's selected target. */
export function lifecycleTargetForSelectedRuntime(
  selected: SelectedRuntime
): RuntimeLifecycleTarget {
  switch (selected.kind) {
    case "simulator":
      return { kind: "simulator" };
    case "local":
      return { kind: "managed", id: selected.id };
    case "remote":
      return {
        kind: "remote",
        endpoint: selected.id,
        ...(selected.label.trim() ? { label: selected.label.trim() } : {}),
      };
  }
}
