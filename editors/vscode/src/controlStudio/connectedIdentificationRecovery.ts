import { bindRuntimeConnection } from "./runtimeConnection";
import { RuntimeRecordingSession } from "./runtimeRecording";
import { ConnectedIdentificationMode } from "./connectedIdentificationMode";
import type { ConnectedIdentificationPorts } from "./connectedIdentification";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationAttempt } from "./verificationClient";

/** Reconcile original recording/mode owners; recovery cannot start another experiment. */
export function identificationRecovery(loop: ControlLoopDefinition, ports: ConnectedIdentificationPorts | undefined,
  completed: () => void) {
  const checkpoint = ports?.mode.read(), recording = ports?.recordingOwner();
  if (!ports || !checkpoint && !recording) return undefined;
  let restored = false;
  const cancel = async () => {
    const connection = await bindRuntimeConnection(loop.runtimeId);
    if (connection.source !== "simulation") throw new Error("Reconnect the original simulator for identification recovery.");
    if (recording) { await ports.retain(await (await RuntimeRecordingSession.recover(loop, recording)).stop()); await ports.clear(); }
    if (checkpoint) await new ConnectedIdentificationMode(loop, connection.request, ports.mode).restore();
    restored = true; completed();
  };
  return { cancel, snapshot: (): RuntimeVerificationAttempt | undefined => restored ? undefined : {
    status: "cancellation-unconfirmed", jobId: checkpoint?.transfer?.owner.id ?? recording?.recordingId ?? `mode:${checkpoint!.original.program_revision}`,
    projectRevision: checkpoint?.original.program_revision ?? recording!.programRevision ?? loop.revision, request: { execution: "connected-runtime" },
    reason: "Restore the original identification mode and commands before starting another operation." } };
}
