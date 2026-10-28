import * as vscode from "vscode";

import { runtimeLifecycleService } from "./runtimeLifecycle";
import { setSelectedRuntimeId } from "./selectedRuntime";
import { SIMULATOR_RUNTIME_ID } from "./trustHomeModel";

/**
 * Selects run target `id` from the side panel's Target or a Devices & Connections node's `Set as run target`, so that
 * Start, the status bar, Live Values and HMI Live all act on the target the side panel shows (specification 25,
 * section 5.1). `endpoint` is the target's control endpoint when it has one: a remote's, or a managed runtime's; the
 * Simulator has none. A session with another target ends first, as Disconnect or Stop ends it, and a failure reported
 * for another target is forgotten. The runtime settings then name the selected endpoint in online mode, or leave online
 * mode for a target without one. Selecting opens no session: only Start or Connect does. A session that does not end
 * keeps the previous target selected.
 */
export async function selectRunTarget(id: string, endpoint?: string): Promise<boolean> {
  if (!id) {
    return false;
  }
  const selected = endpoint?.trim() || undefined;
  const { status } = await runtimeLifecycleService.snapshot();
  const online = status.runtimeMode === "online" ? status.endpoint.trim() : undefined;
  const sameTarget = selected ? online === selected : id === SIMULATOR_RUNTIME_ID && online === undefined;
  if (!sameTarget) {
    if (runtimeLifecycleService.getStructuredTextSession()) {
      const ended = await runtimeLifecycleService.stopRuntime();
      if (!ended.ok) {
        void vscode.window.showWarningMessage(`The target stays as it was: ${ended.failure.message}`);
        return false;
      }
    }
    runtimeLifecycleService.clearFailure();
  }
  if (selected) {
    await runtimeLifecycleService.useOnlineEndpoint(selected);
  } else if (online !== undefined) {
    await runtimeLifecycleService.setRuntimeMode("simulate");
  }
  await setSelectedRuntimeId(id);
  return true;
}
