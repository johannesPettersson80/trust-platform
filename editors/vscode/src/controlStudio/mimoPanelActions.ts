/** V7.17 debt: a parked loop's retained-recording MIMO analysis on the TypeScript route. A served loop's measured paths are the engine's (C7). */
import * as vscode from "vscode";
import { decodeMimoAnalysisCommand } from "./messages";
import { storeMimoAnalysis } from "./panelPersistence";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import { StudioAnalysisSessions } from "./studioAnalysisSessions";

export type MimoAnalysisCommand = NonNullable<ReturnType<typeof decodeMimoAnalysisCommand>>;

export interface MimoPanelActionDependencies {
  context: vscode.ExtensionContext;
  loop: ControlLoopDefinition;
  recording: LoopRecording | undefined;
  sessions: StudioAnalysisSessions;
  command: MimoAnalysisCommand;
  retainWorkflowDraft: () => Promise<void>;
  post: (payload: Record<string, unknown>) => Promise<void> | void;
}

/** Runs the retained-recording MIMO owner and publishes its typed result envelope. */
export async function handleMimoAnalysisCommand(dependencies: MimoPanelActionDependencies): Promise<void> {
  const { loop, recording, sessions, command, retainWorkflowDraft, post } = dependencies;
  try {
    if (!recording) throw new Error("Select a retained recording before calculating MIMO paths.");
    const snapshot = sessions.analyzeMimo(recording, command.recordingId, command.trials,
      command.maximumUnexcitedInputDrift === undefined ? undefined : { maximumUnexcitedInputDrift: command.maximumUnexcitedInputDrift });
    await storeMimoAnalysis(loop, snapshot);
    await retainWorkflowDraft();
    await post({ status: "complete", result: snapshot.result, recordingId: snapshot.recordingId, recordingHash: snapshot.recordingHash,
      trials: snapshot.trials, ...(snapshot.options ? { options: snapshot.options } : {}) });
  } catch (error) {
    await post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
  }
}
