import type * as vscode from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample, LoopRecording } from "./loopMonitor";
import type { ConnectedIdentificationPorts } from "./connectedIdentification";
import { clearExperimentRecovery, storeExperimentRecovery } from "./panelPersistence";

function recordingKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.identificationRecording.${loop.projectId}.${loop.id}`;
}
export async function clearIdentificationRecording(context: vscode.ExtensionContext, loop: ControlLoopDefinition): Promise<void> {
  await context.workspaceState.update(recordingKey(loop), undefined);
}
/** Bind recovery to this loop and workspace; never borrow the manual recorder's journal. */
export function connectedIdentificationPanelPorts(context: vscode.ExtensionContext, loop: ControlLoopDefinition,
  sample: () => Promise<LoopMonitorSample>, retain: (recording: LoopRecording) => Promise<void>): ConnectedIdentificationPorts {
  const modeKey = `trust.controlStudio.identificationMode.${loop.projectId}.${loop.id}`;
  return { sample, retain,
    recordingOwner: () => context.workspaceState.get(recordingKey(loop)),
    recordingPersistence: {
      persist: async owner => { await context.workspaceState.update(recordingKey(loop), owner); },
      clear: () => clearIdentificationRecording(context, loop),
    },
    persist: (plan, owner) => storeExperimentRecovery(context, loop, plan, owner),
    clear: async () => { await clearExperimentRecovery(context, loop); await clearIdentificationRecording(context, loop); },
    mode: { read: () => context.workspaceState.get(modeKey),
      write: async value => { await context.workspaceState.update(modeKey, value); } },
  };
}
