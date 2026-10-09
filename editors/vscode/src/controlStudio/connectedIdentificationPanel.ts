import type * as vscode from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample, LoopRecording } from "./loopMonitor";
import type { ConnectedIdentificationPorts } from "./connectedIdentification";
import type { IdentificationCampaignJournal } from "./identificationCampaign";
import { clearExperimentRecovery, storeExperimentRecovery } from "./panelPersistence";

function recordingKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.identificationRecording.${loop.projectId}.${loop.id}`;
}
export async function clearIdentificationRecording(context: vscode.ExtensionContext, loop: ControlLoopDefinition): Promise<void> {
  await context.workspaceState.update(recordingKey(loop), undefined);
}
function campaignKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.identificationCampaign.${loop.projectId}.${loop.id}`;
}

/** The experiment an interrupted identification campaign journalled, if any: it belongs to that campaign, not to the Test page. */
export function journalledIdentificationExperimentId(context: vscode.ExtensionContext, loop: ControlLoopDefinition): string | undefined {
  return context.workspaceState.get<IdentificationCampaignJournal>(campaignKey(loop))?.plan?.id;
}

/** Bind recovery to this project, loop and workspace; no other loop's identification recording can be reattached. */
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
    clear: async () => {
      await clearExperimentRecovery(context, loop);
      await clearIdentificationRecording(context, loop);
      await context.workspaceState.update(campaignKey(loop), undefined);
    },
    mode: { read: () => context.workspaceState.get(modeKey),
      write: async value => { await context.workspaceState.update(modeKey, value); } },
    campaign: { read: () => context.workspaceState.get<IdentificationCampaignJournal>(campaignKey(loop)),
      write: async value => { await context.workspaceState.update(campaignKey(loop), value); } },
  };
}
