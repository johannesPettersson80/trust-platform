import type * as vscode from "vscode";
import { restoreRecordingRecovery } from "./panelPersistence";
import { sameRecordingScope, type RecordingCoordinator } from "./recordingCoordinator";
import { RuntimeRecordingSession } from "./runtimeRecording";
import type { ControlLoopDefinition } from "./loopDefinition";

/** Reattaches a persisted owner without granting any new Start authority. */
export async function restoreRecordingRecoveryOwner(context: vscode.ExtensionContext, loop: ControlLoopDefinition,
  coordinator: RecordingCoordinator, recordingChanged: (active: boolean) => void): Promise<void> {
  if (coordinator.recording) return;
  const owner = restoreRecordingRecovery(context, loop);
  if (!owner) return;
  try {
    coordinator.adoptRecoveredOwner(await RuntimeRecordingSession.recover(loop, owner),
      "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.");
  } catch (error) {
    coordinator.adoptUnavailableRecovery(loop,
      `Recording recovery unconfirmed — ${error instanceof Error ? error.message : String(error)}`);
  }
  recordingChanged(true);
}

/** Retries only a persisted unavailable owner after its exact runtime scope reconnects. */
export async function retryRecordingRecoveryOwner(context: vscode.ExtensionContext, loop: ControlLoopDefinition,
  coordinator: RecordingCoordinator, recordingChanged: (active: boolean) => void): Promise<boolean> {
  if (coordinator.session || coordinator.currentUpdate?.state !== "unconfirmed" || !sameRecordingScope(coordinator.loop, loop)) return false;
  const owner = restoreRecordingRecovery(context, loop);
  if (!owner) return false;
  try {
    coordinator.adoptRecoveredOwner(await RuntimeRecordingSession.recover(loop, owner),
      "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.");
  } catch (error) {
    coordinator.adoptUnavailableRecovery(loop,
      `Recording recovery unconfirmed — ${error instanceof Error ? error.message : String(error)}`);
  }
  recordingChanged(true);
  return true;
}
