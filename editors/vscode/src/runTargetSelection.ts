import * as vscode from "vscode";

import { projectRequest } from "./projectRequest";
import { runtimeLifecycleService } from "./runtimeLifecycle";
import { setSelectedRuntimeId } from "./selectedRuntime";
import { toModelSnapshot } from "./trustHomeModel";

/** What selecting a target does, as the language server answers it (`trust-lsp.runTargets`, action `select`). */
interface Selecting {
  /** It is another target than the one the runtime settings are on. */
  readonly changesTarget: boolean;
  /** `online` at `endpoint` (empty for the project's own runtime), `simulate`, or `keep`. */
  readonly settings: "online" | "simulate" | "keep";
  readonly endpoint: string;
}

/**
 * Selects run target `id` from the side panel's Target or a Devices & Connections node's `Set as run target`, so that
 * Start, the status bar, Live Values and HMI Live all act on the target the side panel shows (specification 25,
 * section 5.1). `endpoint` is the target's control endpoint when it has one: a remote's, or a managed runtime's; the
 * Simulator and the project's own runtime have none. The language server answers what selecting does: a session with
 * another target ends first, as Disconnect or Stop ends it, and a failure reported for another target is forgotten;
 * the runtime settings then name the selected endpoint in online mode, follow the project's own runtime online, or
 * leave online mode for a target without an endpoint. Selecting opens no session: only Start or Connect does. A
 * session that does not end keeps the previous target selected.
 */
export async function selectRunTarget(id: string, endpoint?: string): Promise<boolean> {
  const answer = await projectRequest<Selecting>("trust-lsp.runTargets", {
    action: "select",
    snapshot: toModelSnapshot(await runtimeLifecycleService.snapshot()),
    id,
    endpoint,
  });
  if (!answer.ok) {
    void vscode.window.showWarningMessage(`The target stays as it was: ${answer.error}`);
    return false;
  }
  if (answer.changesTarget) {
    if (runtimeLifecycleService.getStructuredTextSession()) {
      const ended = await runtimeLifecycleService.stopRuntime();
      if (!ended.ok) {
        void vscode.window.showWarningMessage(`The target stays as it was: ${ended.failure.message}`);
        return false;
      }
    }
    runtimeLifecycleService.clearFailure();
  }
  if (answer.settings === "online") {
    await runtimeLifecycleService.useOnlineEndpoint(answer.endpoint);
  } else if (answer.settings === "simulate") {
    await runtimeLifecycleService.setRuntimeMode("simulate");
  }
  await setSelectedRuntimeId(id);
  return true;
}
